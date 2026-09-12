pub mod archive;
pub mod formats;
mod integration;
mod rar;
mod verification;
mod zip_reader;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager};

#[derive(Default)]
struct Jobs {
    busy: AtomicBool,
    cancel: AtomicBool,
}
#[derive(Default)]
struct State {
    jobs: Arc<Jobs>,
    pending: Mutex<Vec<OpenRequest>>,
}
#[derive(serde::Serialize)]
struct OpenRequest {
    paths: Vec<String>,
    compress: bool,
    extract: bool,
}
struct Guard(Arc<Jobs>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}
fn begin(state: &State) -> Result<Guard, String> {
    state
        .jobs
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "다른 작업이 진행 중입니다.")?;
    state.jobs.cancel.store(false, Ordering::SeqCst);
    Ok(Guard(state.jobs.clone()))
}
#[tauri::command]
async fn inspect_archive(
    path: String,
    password: Option<String>,
    limit_gib: u64,
) -> Result<archive::Info, String> {
    tauri::async_runtime::spawn_blocking(move || {
        formats::inspect(
            &PathBuf::from(path),
            password.as_deref(),
            limit_gib.clamp(1, 2048) * 1024 * 1024 * 1024,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn extract_archive(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    path: String,
    destination: String,
    password: Option<String>,
    limit_gib: u64,
) -> Result<archive::Outcome, String> {
    if !(1..=2048).contains(&limit_gib) {
        return Err("해제 한도는 1~2048 GiB여야 합니다.".into());
    }
    let guard = begin(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        formats::extract(
            &PathBuf::from(path),
            &PathBuf::from(destination),
            password.as_deref(),
            limit_gib * 1024 * 1024 * 1024,
            &_guard.0.cancel,
            &|p| {
                let _ = app.emit("progress", p);
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn create_archive(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    sources: Vec<String>,
    output: String,
    level: i64,
    format: String,
    trash_after: bool,
) -> Result<archive::Outcome, String> {
    let guard = begin(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let sources: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();
        let outcome = archive::create_as(
            &sources,
            &PathBuf::from(output),
            level,
            &format,
            &_guard.0.cancel,
            &|p| {
                let _ = app.emit("progress", p);
            },
        )?;
        if trash_after {
            let _ = app.emit("job-phase", "압축 결과 검증 후 원본을 휴지통으로 이동 중");
            let verified = verification::verify_then_trash(
                &sources,
                &PathBuf::from(&outcome.path),
                &_guard.0.cancel,
                &|p| {
                    let _ = app.emit("progress", p);
                },
            );
            Ok(verification::attach_warning(outcome, verified))
        } else {
            Ok(outcome)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn cancel_job(state: tauri::State<'_, State>) {
    state.jobs.cancel.store(true, Ordering::SeqCst);
}
#[tauri::command]
fn pending_files(state: tauri::State<'_, State>) -> Vec<OpenRequest> {
    std::mem::take(&mut *state.pending.lock().unwrap())
}
fn receive(app: &tauri::AppHandle, paths: Vec<String>, compress: bool, extract: bool) {
    if paths.is_empty() {
        return;
    }
    let state = app.state::<State>();
    state.pending.lock().unwrap().push(OpenRequest {
        paths,
        compress,
        extract,
    });
    let _ = app.emit("open-files", ());
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
pub fn run() {
    tauri::Builder::default()
        .manage(State::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            let compress = args.iter().any(|s| s == "--compress");
            let extract = args.iter().any(|s| s == "--extract");
            receive(
                app,
                args.into_iter()
                    .skip(1)
                    .filter(|s| s != "--compress" && s != "--extract")
                    .collect(),
                compress,
                extract,
            );
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            inspect_archive,
            extract_archive,
            create_archive,
            cancel_job,
            pending_files,
            integration::reveal,
            integration::set_default,
            integration::platform,
            integration::open_folder,
            integration::suggest_output,
            integration::shell_settings
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<State>().jobs.busy.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.emit("exit-blocked", ());
                }
            }
        })
        .setup(|app| {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let compress = args.iter().any(|s| s == "--compress");
            let extract = args.iter().any(|s| s == "--extract");
            receive(
                app.handle(),
                args.into_iter()
                    .filter(|s| s != "--compress" && s != "--extract")
                    .collect(),
                compress,
                extract,
            );
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("DalZip 초기화 실패")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                for url in urls {
                    if url.scheme() == "dalzip"
                        && matches!(url.host_str(), Some("compress" | "extract"))
                    {
                        receive(
                            app,
                            url.query_pairs()
                                .filter(|(key, _)| key == "path")
                                .map(|(_, value)| value.into_owned())
                                .collect(),
                            url.host_str() == Some("compress"),
                            url.host_str() == Some("extract"),
                        );
                    } else if let Ok(path) = url.to_file_path() {
                        receive(app, vec![path.to_string_lossy().into()], false, false);
                    }
                }
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if app.state::<State>().jobs.busy.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    let _ = app.emit("exit-blocked", ());
                }
            }
        });
}

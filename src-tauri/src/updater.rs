use crate::{begin, State};
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);
#[derive(Serialize)]
pub struct AvailableUpdate {
    version: String,
    notes: String,
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<AvailableUpdate>, String> {
    let update = app
        .updater_builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|value| AvailableUpdate {
        version: value.version.clone(),
        notes: value.body.clone().unwrap_or_default(),
    });
    *app.state::<PendingUpdate>().0.lock().unwrap() = update;
    Ok(info)
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle, version: String) -> Result<(), String> {
    // 확인 화면에서 동의한 버전만 설치하며, 압축 작업과 설치를 동시에 실행하지 않습니다.
    let guard = begin(&app.state::<State>())?;
    let mut update = app
        .state::<PendingUpdate>()
        .0
        .lock()
        .unwrap()
        .as_ref()
        .filter(|update| update.version == version)
        .cloned()
        .ok_or("업데이트 정보를 다시 확인해주세요.")?;
    update.timeout = Some(Duration::from_secs(300));
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit("update-progress", (downloaded, total));
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("update-installing", ());
    tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    drop(guard);
    app.restart();
}

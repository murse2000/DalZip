use crate::{
    archive::{Entry, Result, MAX_ENTRIES},
    formats::Sink,
};
use std::{ffi::CString, path::Path, ptr};
use unrar_sys as rar;
struct Handle(*const rar::Handle);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            rar::RARCloseArchive(self.0);
        }
    }
}
fn error(code: i32) -> String {
    match code {
        rar::ERAR_MISSING_PASSWORD => "PASSWORD_REQUIRED".into(),
        rar::ERAR_BAD_PASSWORD => "RAR 암호가 틀립니다.".into(),
        rar::ERAR_BAD_DATA => "RAR 데이터 또는 체크섬이 손상되었습니다.".into(),
        _ => format!("RAR 처리 실패 (코드 {code}). 분할 파일과 지원 형식을 확인하세요."),
    }
}
// C 콜백은 파일 경로를 받지 않고 데이터만 전달합니다. 원본 라이브러리에 파일 생성 권한을 맡기지 않습니다.
struct Context<'a, 'b> {
    sink: Option<&'a mut Sink<'b>>,
    error: Option<String>,
    password: &'a str,
}
extern "C" fn callback(
    message: rar::UINT,
    data: rar::LPARAM,
    p1: rar::LPARAM,
    p2: rar::LPARAM,
) -> i32 {
    if data == 0 {
        return -1;
    }
    let context = unsafe { &mut *(data as *mut Context<'_, '_>) };
    match message {
        rar::UCM_PROCESSDATA => {
            if p2 < 0 || p1 == 0 {
                return -1;
            }
            if let Some(sink) = context.sink.as_mut() {
                let bytes = unsafe { std::slice::from_raw_parts(p1 as *const u8, p2 as usize) };
                if let Err(error) = sink.write(bytes) {
                    context.error = Some(error);
                    return -1;
                }
            }
            0
        }
        rar::UCM_NEEDPASSWORD => {
            if context.password.is_empty() || p2 <= 0 {
                context.error = Some("PASSWORD_REQUIRED".into());
                return -1;
            }
            let value = context.password.as_bytes();
            if value.len() >= p2 as usize {
                return -1;
            }
            unsafe {
                ptr::copy_nonoverlapping(value.as_ptr(), p1 as *mut u8, value.len());
                *(p1 as *mut u8).add(value.len()) = 0;
            }
            0
        }
        rar::UCM_NEEDPASSWORDW => {
            if context.password.is_empty() || p2 <= 0 {
                context.error = Some("PASSWORD_REQUIRED".into());
                return -1;
            }
            let value = wide(context.password);
            if value.len() > p2 as usize {
                return -1;
            }
            unsafe {
                ptr::copy_nonoverlapping(value.as_ptr(), p1 as *mut rar::WCHAR, value.len());
            }
            0
        }
        rar::UCM_CHANGEVOLUME | rar::UCM_CHANGEVOLUMEW => {
            context.error = Some("분할 RAR은 이 버전에서 지원하지 않습니다.".into());
            -1
        }
        _ => 0,
    }
}
fn wide(value: &str) -> Vec<rar::WCHAR> {
    #[cfg(windows)]
    let mut data: Vec<rar::WCHAR> = value.encode_utf16().collect();
    #[cfg(not(windows))]
    let mut data: Vec<rar::WCHAR> = value.chars().map(|c| c as rar::WCHAR).collect();
    data.push(0);
    data
}
fn entry(header: &rar::HeaderDataEx) -> Result<Entry> {
    if header.redir_type != 0
        || (header.host_os == 3
            && ![0, 0o100000, 0o040000].contains(&(header.file_attr & 0o170000)))
    {
        return Err("RAR의 링크/특수 파일은 지원하지 않습니다.".into());
    }
    if header.flags & (rar::RHDF_SPLITBEFORE | rar::RHDF_SPLITAFTER) != 0 {
        return Err("분할 RAR은 이 버전에서 지원하지 않습니다.".into());
    }
    if header.dict_size > 512 * 1024 {
        return Err("RAR 사전 크기가 메모리 안전 한도(512 MiB)를 초과했습니다.".into());
    }
    let data: Vec<_> = header
        .filename_w
        .iter()
        .take_while(|&&c| c != 0)
        .copied()
        .collect();
    #[cfg(windows)]
    let name = String::from_utf16(&data).map_err(|_| "RAR 파일 이름 인코딩 오류")?;
    #[cfg(not(windows))]
    let name = data
        .into_iter()
        .map(|c| char::from_u32(c as u32).ok_or("RAR 파일 이름 인코딩 오류"))
        .collect::<std::result::Result<String, _>>()?;
    Ok(Entry {
        name,
        size: ((header.unp_size_high as u64) << 32) | header.unp_size as u64,
        compressed: ((header.pack_size_high as u64) << 32) | header.pack_size as u64,
        directory: header.flags & rar::RHDF_DIRECTORY != 0,
        encrypted: header.flags & rar::RHDF_ENCRYPTED != 0,
    })
}
fn walk(path: &Path, password: Option<&str>, sink: Option<&mut Sink<'_>>) -> Result<Vec<Entry>> {
    let name = wide(path.to_str().ok_or("RAR 경로 인코딩 오류")?);
    let mut context = Context {
        sink,
        error: None,
        password: password.unwrap_or(""),
    };
    // 구조체의 0 초기화는 UnRAR C API가 요구하는 초기 상태입니다.
    let mut data: rar::OpenArchiveDataEx = unsafe { std::mem::zeroed() };
    data.archive_name_w = name.as_ptr();
    data.open_mode = if context.sink.is_some() {
        rar::RAR_OM_EXTRACT
    } else {
        rar::RAR_OM_LIST
    };
    data.callback = Some(callback);
    data.user_data = &mut context as *mut _ as rar::LPARAM;
    let raw = unsafe { rar::RAROpenArchiveEx(&raw mut data) };
    if raw.is_null() {
        return Err(context
            .error
            .unwrap_or_else(|| error(data.open_result as i32)));
    }
    let handle = Handle(raw);
    if data.flags & rar::ROADF_VOLUME != 0 {
        return Err("분할 RAR은 이 버전에서 지원하지 않습니다.".into());
    }
    if let Some(password) = password {
        let value = CString::new(password).map_err(|_| "암호에 NUL 문자를 사용할 수 없습니다.")?;
        unsafe {
            rar::RARSetPassword(handle.0, value.as_ptr() as *mut _);
        }
    }
    let mut entries = Vec::new();
    loop {
        let mut header: rar::HeaderDataEx = unsafe { std::mem::zeroed() };
        let code = unsafe { rar::RARReadHeaderEx(handle.0, &raw mut header) };
        if code == rar::ERAR_END_ARCHIVE {
            break;
        }
        if code != 0 {
            return Err(context.error.take().unwrap_or_else(|| error(code)));
        }
        let file = entry(&header)?;
        if let Some(sink) = context.sink.as_mut() {
            sink.start(&file, true)?;
        }
        entries.push(file);
        if entries.len() > MAX_ENTRIES {
            return Err("항목 수가 안전 한도를 초과했습니다.".into());
        }
        let code = unsafe {
            rar::RARProcessFileW(
                handle.0,
                if context.sink.is_some() {
                    rar::RAR_TEST
                } else {
                    rar::RAR_SKIP
                },
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if let Some(error) = context.error.take() {
            return Err(error);
        }
        if code != 0 {
            return Err(error(code));
        }
        if let Some(sink) = context.sink.as_mut() {
            sink.finish_entry()?;
        }
    }
    Ok(entries)
}
pub fn inspect(path: &Path, password: Option<&str>) -> Result<Vec<Entry>> {
    walk(path, password, None)
}
pub fn extract(path: &Path, password: Option<&str>, sink: &mut Sink<'_>) -> Result<()> {
    walk(path, password, Some(sink)).map(|_| ())
}

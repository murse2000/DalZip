use std::path::PathBuf;
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}
pub const EXTENSIONS: &[&str] = &[
    "zip", "zipx", "7z", "rar", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst",
];
#[tauri::command]
pub fn suggest_output(sources: Vec<String>, format: String) -> Result<String, String> {
    if !["zip", "7z", "tar.gz"].contains(&format.as_str()) {
        return Err("지원하지 않는 출력 형식입니다.".into());
    }
    let first = PathBuf::from(sources.first().ok_or("선택 항목이 없습니다.")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let stem = if first.is_dir() {
        first.file_name()
    } else {
        first.file_stem()
    }
    .ok_or("파일 이름이 없습니다.")?
    .to_string_lossy();
    Ok(first
        .parent()
        .ok_or("상위 폴더가 없습니다.")?
        .join(format!("{stem}.{format}"))
        .to_string_lossy()
        .into())
}
#[tauri::command]
pub fn reveal(path: String) -> Result<(), String> {
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(&path)
        .status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let status = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap())
        .status();
    status.map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.is_dir() {
        return Err("폴더만 열 수 있습니다.".into());
    }
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("/usr/bin/open")
        .arg(path)
        .status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("explorer.exe")
        .arg(path)
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let status = std::process::Command::new("xdg-open").arg(path).status();
    status.map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
pub fn set_default(extensions: Vec<String>) -> Result<String, String> {
    if extensions.is_empty() || extensions.iter().any(|v| !EXTENSIONS.contains(&v.as_str())) {
        return Err("연결할 지원 확장자를 선택하세요.".into());
    }
    #[cfg(target_os = "macos")]
    {
        use core_foundation::{
            base::TCFType,
            string::{CFString, CFStringRef},
        };
        #[link(name = "CoreServices", kind = "framework")]
        unsafe extern "C" {
            fn LSSetDefaultRoleHandlerForContentType(
                kind: CFStringRef,
                roles: u32,
                bundle: CFStringRef,
            ) -> i32;
            fn UTTypeCreatePreferredIdentifierForTag(
                class: CFStringRef,
                tag: CFStringRef,
                conforms: CFStringRef,
            ) -> CFStringRef;
        }
        let bundle = CFString::new("com.dalbear.dalzip");
        let class = CFString::new("public.filename-extension");
        for extension in extensions {
            let tag = CFString::new(&extension);
            let kind = unsafe {
                UTTypeCreatePreferredIdentifierForTag(
                    class.as_concrete_TypeRef(),
                    tag.as_concrete_TypeRef(),
                    std::ptr::null(),
                )
            };
            if kind.is_null() {
                return Err(format!(".{extension} 형식 등록을 찾을 수 없습니다."));
            }
            let kind = unsafe { CFString::wrap_under_create_rule(kind) };
            let status = unsafe {
                LSSetDefaultRoleHandlerForContentType(
                    kind.as_concrete_TypeRef(),
                    u32::MAX,
                    bundle.as_concrete_TypeRef(),
                )
            };
            if status != 0 {
                return Err(format!(".{extension} 기본 앱 설정 실패 ({status}). 응용 프로그램 폴더에 설치 후 시도하세요."));
            }
        }
        Ok("선택한 확장자의 기본 앱을 DalZip으로 설정했습니다.".into())
    }
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        for extension in extensions {
            let id = format!("DalZip.{extension}");
            registry(
                &format!("Software\\Classes\\{id}"),
                "",
                &format!("{} 압축 파일", extension.to_uppercase()),
            )?;
            registry(
                &format!("Software\\Classes\\{id}\\shell\\open\\command"),
                "",
                &format!("\"{}\" \"%1\"", exe.display()),
            )?;
            registry(
                &format!("Software\\Classes\\.{extension}\\OpenWithProgids"),
                &id,
                "",
            )?;
            registry(
                "Software\\DalZip\\Capabilities\\FileAssociations",
                &format!(".{extension}"),
                &id,
            )?;
        }
        registry(
            "Software\\DalZip\\Capabilities",
            "ApplicationName",
            "DalZip",
        )?;
        registry(
            "Software\\DalZip\\Capabilities",
            "ApplicationDescription",
            "달베어 압축 도구",
        )?;
        registry(
            "Software\\RegisteredApplications",
            "DalZip",
            "Software\\DalZip\\Capabilities",
        )?;
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:defaultapps?registeredAppUser=DalZip")
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok("Windows 설정에서 선택한 확장자의 기본 앱을 DalZip으로 지정하세요.".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err("이 운영체제는 지원하지 않습니다.".into())
}
#[cfg(target_os = "windows")]
fn registry(path: &str, name: &str, value: &str) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(path)
        .and_then(|(k, _)| k.set_value(name, &value))
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn shell_settings(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let extension = exe
            .parent()
            .and_then(|p| p.parent())
            .ok_or("앱 번들을 찾을 수 없습니다.")?
            .join("PlugIns/DalZipFinder.appex");
        if !extension.exists() {
            return Err("Finder 확장이 포함된 DalZip 설치 패키지를 사용하세요.".into());
        }
        let status = std::process::Command::new("/usr/bin/pluginkit")
            .args(["-a"])
            .arg(extension)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Finder 확장 등록에 실패했습니다.".into());
        }
        std::process::Command::new("/usr/bin/open")
            .args(["-b", "com.apple.systempreferences"])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok("시스템 설정 → 일반 → 로그인 항목 및 확장 프로그램 → Finder에서 DalZip Finder 메뉴를 켜세요.".into())
    }
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        let dll = app
            .path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("DalZipShell.dll");
        if !dll.exists() {
            return Err("탐색기 확장 DLL이 포함된 설치 패키지를 사용하세요.".into());
        }
        let clsid = "{627ED496-2F22-459E-A0EC-D2459E46C191}";
        registry(
            "Software\\DalZip",
            "Executable",
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy(),
        )?;
        registry(
            &format!("Software\\Classes\\CLSID\\{clsid}\\InprocServer32"),
            "",
            &dll.to_string_lossy(),
        )?;
        registry(
            &format!("Software\\Classes\\CLSID\\{clsid}\\InprocServer32"),
            "ThreadingModel",
            "Apartment",
        )?;
        let extract_clsid = "{627ED496-2F22-459E-A0EC-D2459E46C192}";
        registry(
            &format!("Software\\Classes\\CLSID\\{extract_clsid}\\InprocServer32"),
            "",
            &dll.to_string_lossy(),
        )?;
        registry(
            &format!("Software\\Classes\\CLSID\\{extract_clsid}\\InprocServer32"),
            "ThreadingModel",
            "Apartment",
        )?;
        registry(
            "Software\\Classes\\*\\shell\\DalZipExtract",
            "ExplorerCommandHandler",
            extract_clsid,
        )?;
        registry(
            "Software\\Classes\\*\\shell\\DalZipExtract",
            "MultiSelectModel",
            "Player",
        )?;
        for class in ["*", "Directory"] {
            let key = format!("Software\\Classes\\{class}\\shell\\DalZip");
            registry(&key, "ExplorerCommandHandler", clsid)?;
            registry(&key, "MultiSelectModel", "Player")?;
        }
        Ok(
            "탐색기 확장을 등록했습니다. Windows 11에서는 ‘추가 옵션 표시’에서 메뉴를 확인하세요."
                .into(),
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("이 운영체제는 지원하지 않습니다.".into())
    }
}

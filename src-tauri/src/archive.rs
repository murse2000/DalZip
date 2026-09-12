use rayon::prelude::*;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Instant,
};
use unicode_normalization::UnicodeNormalization;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

pub(crate) type Result<T> = std::result::Result<T, String>;
pub(crate) const MAX_ENTRIES: usize = 100_000;
pub(crate) const BUFFER: usize = 256 * 1024;
pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub compressed: u64,
    pub directory: bool,
    pub encrypted: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub format: String,
    pub size_known: bool,
    pub path: String,
    pub entries: Vec<Entry>,
    pub total_size: u64,
    pub archive_size: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub completed: u64,
    pub total: u64,
    pub elapsed: f64,
    pub file: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub warning: Option<String>,
    pub path: String,
    pub bytes: u64,
    pub seconds: f64,
    pub files: usize,
}

pub struct Task<'a> {
    pub cancel: &'a AtomicBool,
    done: AtomicU64,
    total: u64,
    start: Instant,
    last: Mutex<Instant>,
    emit: &'a (dyn Fn(Progress) + Sync),
}
impl<'a> Task<'a> {
    pub fn new(cancel: &'a AtomicBool, total: u64, emit: &'a (dyn Fn(Progress) + Sync)) -> Self {
        Self {
            cancel,
            done: AtomicU64::new(0),
            total,
            start: Instant::now(),
            last: Mutex::new(Instant::now()),
            emit,
        }
    }
    pub(crate) fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("작업을 취소했습니다. 미완성 결과를 삭제했습니다.".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn advance(&self, bytes: u64, file: &str) {
        let done = self.done.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if let Ok(mut last) = self.last.try_lock() {
            if last.elapsed().as_millis() >= 100 || done == self.total {
                (self.emit)(Progress {
                    completed: done,
                    total: self.total,
                    elapsed: self.start.elapsed().as_secs_f64(),
                    file: file.into(),
                });
                *last = Instant::now();
            }
        }
    }
}

// 두 운영체제에서 같은 이름으로 해석되는 경로와 ZIP 경로 탈출을 사전에 차단합니다.
pub(crate) fn safe_name(name: &str) -> Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(format!("절대 경로는 해제할 수 없습니다: {name}"));
    }
    let mut path = PathBuf::new();
    for part in normalized.trim_end_matches('/').split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with([' ', '.'])
            || part.chars().any(|c| c < ' ' || ":<>\"|?*".contains(c))
        {
            return Err(format!("안전하지 않은 파일 경로: {name}"));
        }
        let base = part.split('.').next().unwrap_or("").to_uppercase();
        if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str())
            || ((base.starts_with("COM") || base.starts_with("LPT"))
                && base.chars().count() == 4
                && "123456789¹²³".contains(base.chars().last().unwrap()))
        {
            return Err(format!("예약된 파일 이름: {name}"));
        }
        path.push(part);
    }
    if path.as_os_str().is_empty() {
        return Err("빈 파일 경로입니다.".into());
    }
    Ok(path)
}
fn key(path: &Path) -> String {
    path.to_string_lossy()
        .nfc()
        .collect::<String>()
        .to_lowercase()
}
pub(crate) fn validate_paths(entries: &[(PathBuf, bool)]) -> Result<()> {
    let mut known = HashMap::new();
    for (path, dir) in entries {
        if known.insert(key(path), *dir).is_some() {
            return Err(format!(
                "중복되거나 대소문자/유니코드가 충돌하는 이름: {}",
                path.display()
            ));
        }
    }
    for (path, _) in entries {
        for parent in path
            .ancestors()
            .skip(1)
            .filter(|p| !p.as_os_str().is_empty())
        {
            if known.get(&key(parent)) == Some(&false) {
                return Err(format!(
                    "파일과 폴더 경로가 충돌합니다: {}",
                    parent.display()
                ));
            }
        }
    }
    Ok(())
}
fn open(path: &Path) -> Result<ZipArchive<crate::zip_reader::ZipReader>> {
    ZipArchive::new(crate::zip_reader::ZipReader::open(path).map_err(err)?).map_err(err)
}
pub fn inspect(path: &Path) -> Result<Info> {
    let mut zip = open(path)?;
    if zip.len() > MAX_ENTRIES {
        return Err("ZIP 항목 수가 안전 한도(100,000개)를 초과했습니다.".into());
    }
    let mut entries = Vec::with_capacity(zip.len());
    let mut total = 0u64;
    for i in 0..zip.len() {
        let file = zip.by_index_raw(i).map_err(err)?;
        total = total.checked_add(file.size()).ok_or("전체 크기 오버플로")?;
        entries.push(Entry {
            name: file.name().into(),
            size: file.size(),
            compressed: file.compressed_size(),
            directory: file.is_dir(),
            encrypted: file.encrypted(),
        });
    }
    Ok(Info {
        format: "ZIP".into(),
        size_known: true,
        path: path.to_string_lossy().into(),
        entries,
        total_size: total,
        archive_size: fs::metadata(path).map_err(err)?.len(),
    })
}

pub fn extract(
    path: &Path,
    destination: &Path,
    password: Option<&str>,
    limit: u64,
    cancel: &AtomicBool,
    emit: &(dyn Fn(Progress) + Sync),
) -> Result<Outcome> {
    let started = Instant::now();
    let mut zip = open(path)?;
    if zip.len() > MAX_ENTRIES {
        return Err("ZIP 항목 수가 안전 한도를 초과했습니다.".into());
    }
    let mut entries = Vec::with_capacity(zip.len());
    let mut sizes = Vec::with_capacity(zip.len());
    let mut total = 0u64;
    for i in 0..zip.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err("작업을 취소했습니다.".into());
        }
        let file = zip.by_index_raw(i).map_err(err)?;
        let mode = file.unix_mode().unwrap_or(0) & 0o170000;
        if file.is_symlink() || ![0, 0o100000, 0o040000].contains(&mode) {
            return Err(format!(
                "링크/특수 파일은 해제하지 않습니다: {}",
                file.name()
            ));
        }
        total = total.checked_add(file.size()).ok_or("전체 크기 오버플로")?;
        if total > limit {
            return Err(
                "예상 해제 크기가 설정된 한도를 초과합니다. 설정에서 한도를 확인하세요.".into(),
            );
        }
        entries.push((safe_name(file.name())?, file.is_dir()));
        sizes.push(file.size());
    }
    validate_paths(&entries)?;
    let parent = destination.canonicalize().map_err(err)?;
    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("Archive");
    let prefix = format!(
        "{}-",
        stem.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .take(48)
            .collect::<String>()
    );
    // 독립된 새 폴더에서만 작업하므로 기존 파일/심볼릭 링크를 덮어쓰지 않습니다.
    let staging = tempfile::Builder::new()
        .prefix(&prefix)
        .tempdir_in(&parent)
        .map_err(err)?;
    let directories: HashSet<PathBuf> = entries
        .iter()
        .map(|(relative, directory)| {
            let out = staging.path().join(relative);
            if *directory {
                out
            } else {
                out.parent().unwrap().to_owned()
            }
        })
        .collect();
    for directory in directories {
        fs::create_dir_all(directory).map_err(err)?;
    }
    let task = Task::new(cancel, total, emit);
    let workers = std::thread::available_parallelism()
        .map_or(2, usize::from)
        .min(4);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .map_err(err)?;
    pool.install(|| {
        entries.par_iter().enumerate().try_for_each_init(
            || (zip.clone(), vec![0u8; BUFFER]),
            |(reader, buffer), (i, (relative, dir))| -> Result<()> {
                task.check()?;
                if *dir {
                    return Ok(());
                }
                let mut input = if let Some(pw) = password {
                    reader.by_index_decrypt(i, pw.as_bytes())
                } else {
                    reader.by_index(i)
                }
                .map_err(|e| format!("{}: {e}", relative.display()))?;
                let out = File::options()
                    .write(true)
                    .create_new(true)
                    .open(staging.path().join(relative))
                    .map_err(err)?;
                let mut out = out;
                let mut written = 0u64;
                loop {
                    task.check()?;
                    let count = input
                        .read(buffer)
                        .map_err(|e| format!("무결성 확인 실패 · {}: {e}", relative.display()))?;
                    if count == 0 {
                        break;
                    }
                    written += count as u64;
                    if written > sizes[i] {
                        return Err("실제 해제 크기가 ZIP 메타데이터를 초과했습니다.".into());
                    }
                    out.write_all(&buffer[..count]).map_err(err)?;
                    task.advance(count as u64, &relative.to_string_lossy());
                }
                if written != sizes[i] {
                    return Err(format!(
                        "파일 크기가 일치하지 않습니다: {}",
                        relative.display()
                    ));
                }
                out.flush().map_err(err)?;
                Ok(())
            },
        )
    })?;
    task.check()?;
    let result = staging.keep();
    Ok(Outcome {
        warning: None,
        path: result.to_string_lossy().into(),
        bytes: total,
        seconds: started.elapsed().as_secs_f64(),
        files: entries.iter().filter(|(_, dir)| !dir).count(),
    })
}

pub fn create(
    sources: &[PathBuf],
    output: &Path,
    level: i64,
    cancel: &AtomicBool,
    emit: &(dyn Fn(Progress) + Sync),
) -> Result<Outcome> {
    create_as(sources, output, level, "zip", cancel, emit)
}
pub fn create_as(
    sources: &[PathBuf],
    output: &Path,
    level: i64,
    format: &str,
    cancel: &AtomicBool,
    emit: &(dyn Fn(Progress) + Sync),
) -> Result<Outcome> {
    let started = Instant::now();
    if !output
        .to_string_lossy()
        .to_lowercase()
        .ends_with(&format!(".{format}"))
    {
        return Err(format!(
            "저장 파일 이름은 .{format} 확장자로 끝나야 합니다."
        ));
    }
    if !["zip", "7z", "tar.gz"].contains(&format) {
        return Err("지원하지 않는 출력 형식입니다.".into());
    }
    if sources.is_empty() {
        return Err("압축할 파일을 선택하세요.".into());
    }
    if ![0, 1, 6, 9].contains(&level) {
        return Err("지원하지 않는 압축 수준입니다.".into());
    }
    let parent = output
        .parent()
        .ok_or("출력 폴더가 없습니다.")?
        .canonicalize()
        .map_err(err)?;
    let output = parent.join(output.file_name().ok_or("출력 파일 이름이 없습니다.")?);
    if output.exists() {
        return Err("같은 이름의 파일이 있습니다. 다른 이름을 선택하세요.".into());
    }
    let mut items = Vec::new();
    let mut paths = Vec::new();
    let mut total = 0u64;
    for source in sources {
        if fs::symlink_metadata(source)
            .map_err(err)?
            .file_type()
            .is_symlink()
        {
            return Err("심볼릭 링크는 압축하지 않습니다.".into());
        }
        let source = source.canonicalize().map_err(err)?;
        if output.starts_with(&source) {
            return Err("압축 대상 폴더 밖에 ZIP 파일을 저장하세요.".into());
        }
        let base = source
            .parent()
            .ok_or("파일 시스템 루트는 압축할 수 없습니다.")?;
        for item in walkdir::WalkDir::new(&source).follow_links(false) {
            if cancel.load(Ordering::Relaxed) {
                return Err("작업을 취소했습니다.".into());
            }
            let item = item.map_err(err)?;
            if !item.file_type().is_file() && !item.file_type().is_dir() {
                return Err(format!(
                    "링크/특수 파일은 압축하지 않습니다: {}",
                    item.path().display()
                ));
            }
            let relative = item.path().strip_prefix(base).map_err(err)?;
            let name = relative
                .to_str()
                .ok_or("UTF-8로 표현할 수 없는 파일 이름입니다.")?
                .replace('\\', "/");
            let path = safe_name(&name)?;
            let size = if item.file_type().is_file() {
                item.metadata().map_err(err)?.len()
            } else {
                0
            };
            total = total.checked_add(size).ok_or("전체 크기 오버플로")?;
            paths.push((path, item.file_type().is_dir()));
            items.push((
                item.path().to_owned(),
                name,
                item.file_type().is_dir(),
                size,
            ));
            if items.len() > MAX_ENTRIES {
                return Err("항목 수가 안전 한도(100,000개)를 초과했습니다.".into());
            }
        }
    }
    validate_paths(&paths)?;
    let mut tmp = tempfile::NamedTempFile::new_in(&parent).map_err(err)?;
    let task = Task::new(cancel, total, emit);
    if format != "zip" {
        crate::formats::write_archive(tmp.as_file_mut(), &items, format, level, &task)?;
    } else {
        let mut writer = ZipWriter::new(BufWriter::with_capacity(BUFFER, tmp.as_file_mut()));
        let options = SimpleFileOptions::default()
            .compression_method(if level == 0 {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            })
            .compression_level(if level == 0 { None } else { Some(level) });
        let mut buffer = vec![0u8; BUFFER];
        for (source, name, directory, size) in &items {
            task.check()?;
            if *directory {
                writer.add_directory(name, options).map_err(err)?;
                continue;
            }
            writer
                .start_file(name, options.large_file(*size >= u32::MAX as u64))
                .map_err(err)?;
            let mut input = File::open(source).map_err(err)?;
            let mut read = 0u64;
            loop {
                task.check()?;
                let count = input.read(&mut buffer).map_err(err)?;
                if count == 0 {
                    break;
                }
                read += count as u64;
                if read > *size {
                    return Err(format!("압축 중 원본 크기가 변경되었습니다: {name}"));
                }
                writer.write_all(&buffer[..count]).map_err(err)?;
                task.advance(count as u64, name);
            }
            if read != *size {
                return Err(format!("압축 중 원본 크기가 변경되었습니다: {name}"));
            }
        }
        writer.finish().map_err(err)?.flush().map_err(err)?;
    }
    task.check()?;
    tmp.as_file().sync_all().map_err(err)?;
    tmp.persist_noclobber(&output).map_err(err)?;
    Ok(Outcome {
        warning: None,
        path: output.to_string_lossy().into(),
        bytes: total,
        seconds: started.elapsed().as_secs_f64(),
        files: items.iter().filter(|i| !i.2).count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &Path, names: &[(&str, &[u8])]) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        for (name, data) in names {
            zip.start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    #[test]
    fn blocks_unsafe_paths() {
        for name in [
            "../escape",
            "/etc/passwd",
            "C:\\bad",
            "a\\..\\b",
            "a/./b",
            "NUL.txt",
            "a:stream",
            "a./b",
            "a//b",
            "CONIN$",
        ] {
            assert!(safe_name(name).is_err(), "{name}");
        }
        assert!(safe_name("한글/파일.txt").is_ok());
    }
    #[test]
    fn round_trip_and_no_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("원본");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("한글.txt"), "달베어 ZIP\n".repeat(10000)).unwrap();
        fs::create_dir(source.join("빈 폴더")).unwrap();
        let zip = tmp.path().join("test.zip");
        let cancel = AtomicBool::new(false);
        create(std::slice::from_ref(&source), &zip, 1, &cancel, &|_| {}).unwrap();
        assert!(create(std::slice::from_ref(&source), &zip, 1, &cancel, &|_| {}).is_err());
        let out = extract(&zip, tmp.path(), None, 1 << 30, &cancel, &|_| {}).unwrap();
        assert_eq!(
            fs::read(source.join("한글.txt")).unwrap(),
            fs::read(Path::new(&out.path).join("원본/한글.txt")).unwrap()
        );
        assert!(Path::new(&out.path).join("원본/빈 폴더").is_dir());
    }
    #[test]
    fn rejects_traversal_collisions_and_bombs_without_output() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("bad.zip");
        let cancel = AtomicBool::new(false);
        for names in [
            vec![("../escape", b"x".as_slice())],
            vec![("A.txt", b"x".as_slice()), ("a.txt", b"y".as_slice())],
            vec![("a", b"x".as_slice()), ("a/b", b"y".as_slice())],
        ] {
            fixture(&zip, &names);
            assert!(extract(&zip, tmp.path(), None, 100, &cancel, &|_| {}).is_err());
        }
        fixture(&zip, &[("large", &[0; 101])]);
        assert!(extract(&zip, tmp.path(), None, 100, &cancel, &|_| {}).is_err());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
    #[test]
    fn corrupt_crc_rolls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("bad.zip");
        fixture(&zip, &[("data", b"unique-payload")]);
        let mut bytes = fs::read(&zip).unwrap();
        let pos = bytes
            .windows(14)
            .position(|v| v == b"unique-payload")
            .unwrap();
        bytes[pos] ^= 1;
        fs::write(&zip, bytes).unwrap();
        assert!(extract(
            &zip,
            tmp.path(),
            None,
            100,
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
    #[test]
    fn cancellation_cleans_partial_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, [0; 1024]).unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(create(&[file], &tmp.path().join("out.zip"), 1, &cancelled, &|_| {}).is_err());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn refuses_source_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", tmp.path().join("link")).unwrap();
        assert!(create(
            &[tmp.path().join("link")],
            &tmp.path().join("out.zip"),
            1,
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err());
    }
    #[test]
    fn encrypted_zip_checks_password_and_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("encrypted.zip");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        let options =
            SimpleFileOptions::default().with_aes_encryption(zip::AesMode::Aes256, "달베어-test");
        zip.start_file("secret.txt", options).unwrap();
        zip.write_all(b"verified secret").unwrap();
        zip.finish().unwrap();
        let cancel = AtomicBool::new(false);
        assert!(inspect(&path).unwrap().entries[0].encrypted);
        assert!(extract(&path, tmp.path(), Some("wrong"), 1000, &cancel, &|_| {}).is_err());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
        let out = extract(
            &path,
            tmp.path(),
            Some("달베어-test"),
            1000,
            &cancel,
            &|_| {},
        )
        .unwrap();
        assert_eq!(
            fs::read(Path::new(&out.path).join("secret.txt")).unwrap(),
            b"verified secret"
        );
    }
    #[test]
    fn mid_operation_cancel_removes_output() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("large.bin");
        fs::write(&source, vec![42; BUFFER * 3]).unwrap();
        let zip = tmp.path().join("cancel.zip");
        let cancel = AtomicBool::new(false);
        let cancel_on_progress = |_: Progress| cancel.store(true, Ordering::Relaxed);
        assert!(create(
            std::slice::from_ref(&source),
            &zip,
            1,
            &cancel,
            &cancel_on_progress
        )
        .is_err());
        assert!(!zip.exists());
        cancel.store(false, Ordering::Relaxed);
        create(&[source], &zip, 1, &cancel, &|_| {}).unwrap();
        assert!(extract(
            &zip,
            tmp.path(),
            None,
            1 << 30,
            &cancel,
            &cancel_on_progress
        )
        .is_err());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 2);
    }
    #[test]
    fn rejects_zip_symlink_and_unicode_alias() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("link.zip");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.add_symlink("link", "../outside", SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap();
        assert!(extract(
            &path,
            tmp.path(),
            None,
            1000,
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err());
        assert!(validate_paths(&[
            (PathBuf::from("é.txt"), false),
            (PathBuf::from("e\u{301}.txt"), false)
        ])
        .is_err());
    }
}

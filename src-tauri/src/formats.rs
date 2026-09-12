use crate::archive::{
    self, err, safe_name, validate_paths, Entry, Info, Outcome, Progress, Result, Task, BUFFER,
    MAX_ENTRIES,
};
use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

fn kind(path: &Path) -> String {
    let name = path.to_string_lossy().to_lowercase();
    for (suffix, format) in [
        (".tar.gz", "tar.gz"),
        (".tgz", "tar.gz"),
        (".tar.bz2", "tar.bz2"),
        (".tbz2", "tar.bz2"),
        (".tar.xz", "tar.xz"),
        (".txz", "tar.xz"),
        (".tar.zst", "tar.zst"),
    ] {
        if name.ends_with(suffix) {
            return format.into();
        }
    }
    path.extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase()
}
fn decoder(path: &Path, format: &str) -> Result<Box<dyn Read>> {
    let file = BufReader::with_capacity(BUFFER, File::open(path).map_err(err)?);
    Ok(match format.rsplit('.').next().unwrap_or(format) {
        "gz" => Box::new(flate2::read::MultiGzDecoder::new(file)),
        "bz2" => Box::new(bzip2::read::MultiBzDecoder::new(file)),
        "xz" => Box::new(lzma_rust2::XzReader::new(file, true)),
        "zst" => Box::new(zstd::stream::read::Decoder::new(file).map_err(err)?),
        "tar" => Box::new(file),
        _ => return Err("지원하지 않는 압축 형식입니다.".into()),
    })
}
struct Bounded<R> {
    reader: R,
    remaining: u64,
}
impl<R: Read> Read for Bounded<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let max = buf.len().min(self.remaining.saturating_add(1) as usize);
        let n = self.reader.read(&mut buf[..max])?;
        if n as u64 > self.remaining {
            return Err(io::Error::other("해제 크기가 안전 한도를 초과했습니다."));
        }
        self.remaining -= n as u64;
        Ok(n)
    }
}
fn sz_error(error: sevenz_rust2::Error) -> String {
    match error {
        sevenz_rust2::Error::PasswordRequired => "PASSWORD_REQUIRED".into(),
        sevenz_rust2::Error::MaybeBadPassword(_) => {
            "암호가 틀리거나 암호화 데이터가 손상되었습니다.".into()
        }
        _ => err(error),
    }
}
fn seven_entry(file: &sevenz_rust2::ArchiveEntry) -> Result<Entry> {
    let mode = file.windows_attributes >> 16 & 0o170000;
    if file.is_anti_item
        || file.windows_attributes & 0x400 != 0
        || ![0, 0o100000, 0o040000].contains(&mode)
    {
        return Err(format!("링크/특수 항목을 지원하지 않습니다: {}", file.name));
    }
    Ok(Entry {
        name: file.name.clone(),
        size: file.size,
        compressed: file.compressed_size,
        directory: file.is_directory,
        encrypted: false,
    })
}
fn tar_entry<R: Read>(file: &tar::Entry<'_, R>) -> Result<Entry> {
    let ty = file.header().entry_type();
    if !ty.is_file() && !ty.is_dir() {
        return Err("TAR의 링크/특수 항목은 지원하지 않습니다.".into());
    }
    let path = file.path().map_err(err)?;
    let name = path
        .to_str()
        .ok_or("UTF-8로 표현할 수 없는 TAR 이름입니다.")?
        .strip_prefix("./")
        .unwrap_or(path.to_str().unwrap())
        .to_owned();
    Ok(Entry {
        name,
        size: file.size(),
        compressed: 0,
        directory: ty.is_dir(),
        encrypted: false,
    })
}
fn check_entries(entries: &[Entry], limit: u64) -> Result<u64> {
    if entries.len() > MAX_ENTRIES {
        return Err("항목 수가 안전 한도(100,000개)를 초과했습니다.".into());
    }
    let mut total = 0u64;
    let mut paths = Vec::new();
    for entry in entries {
        total = total.checked_add(entry.size).ok_or("전체 크기 오버플로")?;
        if total > limit {
            return Err("해제 크기가 설정된 한도를 초과했습니다.".into());
        }
        paths.push((safe_name(&entry.name)?, entry.directory));
    }
    validate_paths(&paths)?;
    Ok(total)
}
pub fn inspect(path: &Path, password: Option<&str>, limit: u64) -> Result<Info> {
    let format = kind(path);
    if format == "zip" || format == "zipx" {
        return archive::inspect(path);
    }
    let mut size_known = true;
    let entries = match format.as_str() {
        "7z" => {
            let reader = sevenz_rust2::ArchiveReader::open(path, password.unwrap_or("").into())
                .map_err(sz_error)?;
            if reader.archive().files.len() > MAX_ENTRIES {
                return Err("항목 수가 안전 한도를 초과했습니다.".into());
            }
            let encrypted = reader.archive().blocks.iter().any(|b| {
                b.coders
                    .iter()
                    .any(|c| c.encoder_method_id() == sevenz_rust2::EncoderMethod::ID_AES256_SHA256)
            });
            reader
                .archive()
                .files
                .iter()
                .map(|file| {
                    let mut entry = seven_entry(file)?;
                    entry.encrypted = encrypted;
                    Ok(entry)
                })
                .collect::<Result<Vec<_>>>()?
        }
        "rar" => crate::rar::inspect(path, password)?,
        v if v.starts_with("tar") => {
            let mut tar = tar::Archive::new(Bounded {
                reader: decoder(path, &format)?,
                remaining: limit.saturating_add(64 * 1024 * 1024),
            });
            let mut entries = Vec::new();
            for file in tar.entries().map_err(err)? {
                let file = file.map_err(err)?;
                let entry = tar_entry(&file)?;
                if entry.name.is_empty() && entry.directory {
                    continue;
                }
                entries.push(entry);
                if entries.len() > MAX_ENTRIES {
                    return Err("항목 수가 안전 한도를 초과했습니다.".into());
                }
            }
            entries
        }
        "gz" | "bz2" | "xz" | "zst" => {
            size_known = false;
            vec![Entry {
                name: path
                    .file_stem()
                    .and_then(|v| v.to_str())
                    .unwrap_or("file")
                    .into(),
                size: 0,
                compressed: fs::metadata(path).map_err(err)?.len(),
                directory: false,
                encrypted: false,
            }]
        }
        _ => return Err("지원 형식: ZIP, 7Z, RAR, TAR, GZ, BZ2, XZ, ZST".into()),
    };
    let total_size = check_entries(&entries, limit)?;
    Ok(Info {
        format: format.to_uppercase(),
        size_known,
        path: path.to_string_lossy().into(),
        entries,
        total_size,
        archive_size: fs::metadata(path).map_err(err)?.len(),
    })
}

// 모든 추가 포맷은 직접 파일을 쓰는 공통 경로를 사용하고, 라이브러리의 자동 경로 해제를 사용하지 않습니다.
pub(crate) struct Sink<'a> {
    root: &'a Path,
    task: Task<'a>,
    limit: u64,
    pub bytes: u64,
    pub files: usize,
    output: Option<BufWriter<File>>,
    expected: Option<u64>,
    current: u64,
    name: String,
    paths: Vec<(PathBuf, bool)>,
}
impl<'a> Sink<'a> {
    fn new(
        root: &'a Path,
        total: u64,
        limit: u64,
        cancel: &'a AtomicBool,
        emit: &'a (dyn Fn(Progress) + Sync),
    ) -> Self {
        Self {
            root,
            task: Task::new(cancel, total, emit),
            limit,
            bytes: 0,
            files: 0,
            output: None,
            expected: None,
            current: 0,
            name: String::new(),
            paths: Vec::new(),
        }
    }
    pub(crate) fn start(&mut self, entry: &Entry, known: bool) -> Result<()> {
        self.task.check()?;
        let relative = safe_name(&entry.name)?;
        self.paths.push((relative.clone(), entry.directory));
        if self.paths.len() > MAX_ENTRIES {
            return Err("항목 수가 안전 한도를 초과했습니다.".into());
        }
        let path = self.root.join(&relative);
        self.name = entry.name.clone();
        self.current = 0;
        self.expected = known.then_some(entry.size);
        if entry.directory {
            fs::create_dir_all(&path).map_err(err)?;
            self.output = None;
        } else {
            fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
            self.output = Some(BufWriter::with_capacity(
                BUFFER,
                File::options()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(err)?,
            ));
            self.files += 1;
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, data: &[u8]) -> Result<()> {
        self.task.check()?;
        self.current = self
            .current
            .checked_add(data.len() as u64)
            .ok_or("크기 오버플로")?;
        self.bytes = self
            .bytes
            .checked_add(data.len() as u64)
            .ok_or("크기 오버플로")?;
        if self.bytes > self.limit || self.expected.is_some_and(|n| self.current > n) {
            return Err("실제 해제 크기가 한도를 초과했습니다.".into());
        }
        self.output
            .as_mut()
            .ok_or("폴더 항목에 파일 데이터가 있습니다.")?
            .write_all(data)
            .map_err(err)?;
        self.task.advance(data.len() as u64, &self.name);
        Ok(())
    }
    pub(crate) fn finish_entry(&mut self) -> Result<()> {
        if let Some(mut output) = self.output.take() {
            if self.expected.is_some_and(|n| self.current != n) {
                return Err("해제된 파일 크기가 일치하지 않습니다.".into());
            }
            output.flush().map_err(err)?;
        }
        self.task.check()
    }
    fn copy(&mut self, input: &mut dyn Read) -> Result<()> {
        let mut buffer = vec![0; BUFFER];
        loop {
            self.task.check()?;
            let n = input.read(&mut buffer).map_err(err)?;
            if n == 0 {
                break;
            }
            self.write(&buffer[..n])?;
        }
        self.finish_entry()
    }
}
pub fn extract(
    path: &Path,
    destination: &Path,
    password: Option<&str>,
    limit: u64,
    cancel: &AtomicBool,
    emit: &(dyn Fn(Progress) + Sync),
) -> Result<Outcome> {
    let format = kind(path);
    if format == "zip" || format == "zipx" {
        return archive::extract(path, destination, password, limit, cancel, emit);
    }
    let start = Instant::now();
    let info = inspect(path, password, limit)?;
    let parent = destination.canonicalize().map_err(err)?;
    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("Archive");
    let prefix = format!(
        "{}-",
        stem.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .take(48)
            .collect::<String>()
    );
    let staging = tempfile::Builder::new()
        .prefix(&prefix)
        .tempdir_in(parent)
        .map_err(err)?;
    let mut sink = Sink::new(staging.path(), info.total_size, limit, cancel, emit);
    match format.as_str() {
        "7z" => {
            let mut reader = sevenz_rust2::ArchiveReader::open(path, password.unwrap_or("").into())
                .map_err(sz_error)?;
            reader.set_thread_count(4);
            reader
                .for_each_entries(|entry, data| {
                    let entry = seven_entry(entry).map_err(io::Error::other)?;
                    sink.start(&entry, true).map_err(io::Error::other)?;
                    sink.copy(data).map_err(io::Error::other)?;
                    Ok(true)
                })
                .map_err(sz_error)?;
        }
        "rar" => crate::rar::extract(path, password, &mut sink)?,
        v if v.starts_with("tar") => {
            let mut tar = tar::Archive::new(Bounded {
                reader: decoder(path, &format)?,
                remaining: limit.saturating_add(64 * 1024 * 1024),
            });
            for file in tar.entries().map_err(err)? {
                let mut file = file.map_err(err)?;
                let entry = tar_entry(&file)?;
                if entry.name.is_empty() && entry.directory {
                    continue;
                }
                sink.start(&entry, true)?;
                sink.copy(&mut file)?;
            }
            // TAR 종료 블록 뒤의 압축 스트림도 끝까지 읽어 체크섬과 크기 제한을 확인합니다.
            let mut reader = tar.into_inner();
            let mut buffer = vec![0; BUFFER];
            loop {
                sink.task.check()?;
                if reader.read(&mut buffer).map_err(err)? == 0 {
                    break;
                }
            }
        }
        _ => {
            sink.start(&info.entries[0], false)?;
            sink.copy(&mut decoder(path, &format)?)?;
        }
    }
    validate_paths(&sink.paths)?;
    sink.task.check()?;
    let bytes = sink.bytes;
    let files = sink.files;
    drop(sink);
    Ok(Outcome {
        warning: None,
        path: staging.keep().to_string_lossy().into(),
        bytes,
        files,
        seconds: start.elapsed().as_secs_f64(),
    })
}

struct Source<'a> {
    file: File,
    task: &'a Task<'a>,
    name: &'a str,
    remaining: u64,
}
impl Read for Source<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.task.check().map_err(io::Error::other)?;
        let count = self.file.read(buffer)?;
        if count as u64 > self.remaining || (count == 0 && self.remaining != 0) {
            return Err(io::Error::other("압축 중 원본 크기가 변경되었습니다."));
        }
        self.remaining -= count as u64;
        self.task.advance(count as u64, self.name);
        Ok(count)
    }
}
pub fn write_archive(
    output: &mut File,
    items: &[(PathBuf, String, bool, u64)],
    format: &str,
    level: i64,
    task: &Task<'_>,
) -> Result<()> {
    if format == "7z" {
        let mut writer = sevenz_rust2::ArchiveWriter::new(BufWriter::with_capacity(BUFFER, output))
            .map_err(err)?;
        writer.set_content_methods(vec![if level == 0 {
            sevenz_rust2::EncoderMethod::COPY.into()
        } else {
            sevenz_rust2::EncoderConfiguration::new(sevenz_rust2::EncoderMethod::LZMA2)
                .with_options(sevenz_rust2::encoder_options::EncoderOptions::Lzma2(
                    sevenz_rust2::encoder_options::Lzma2Options::from_level(level as u32),
                ))
        }]);
        for (path, name, directory, size) in items {
            task.check()?;
            let entry = sevenz_rust2::ArchiveEntry::from_path(path, name.clone());
            let source = if *directory {
                None
            } else {
                Some(Source {
                    file: File::open(path).map_err(err)?,
                    task,
                    name,
                    remaining: *size,
                })
            };
            writer.push_archive_entry(entry, source).map_err(err)?;
        }
        writer.finish().map_err(err)?.flush().map_err(err)?;
    } else {
        let gz = flate2::write::GzEncoder::new(
            BufWriter::with_capacity(BUFFER, output),
            flate2::Compression::new(level as u32),
        );
        let mut tar = tar::Builder::new(gz);
        for (path, name, directory, size) in items {
            task.check()?;
            let mut header = tar::Header::new_gnu();
            header.set_metadata(&fs::metadata(path).map_err(err)?);
            header.set_size(*size);
            header.set_cksum();
            if *directory {
                tar.append_data(&mut header, name, io::empty())
                    .map_err(err)?;
            } else {
                let source = Source {
                    file: File::open(path).map_err(err)?,
                    task,
                    name,
                    remaining: *size,
                };
                tar.append_data(&mut header, name, source).map_err(err)?;
            }
        }
        tar.into_inner()
            .map_err(err)?
            .finish()
            .map_err(err)?
            .flush()
            .map_err(err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sevenz_and_targz_round_trip_and_verify_sources() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("한글 원본");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("data.txt"), b"DalZip round trip\n").unwrap();
        fs::create_dir(source.join("empty")).unwrap();
        let cancel = AtomicBool::new(false);
        for format in ["7z", "tar.gz"] {
            let output = temp.path().join(format!("test.{format}"));
            archive::create_as(
                std::slice::from_ref(&source),
                &output,
                1,
                format,
                &cancel,
                &|_| {},
            )
            .unwrap();
            let info = inspect(&output, None, 1 << 20).unwrap();
            assert_eq!(info.entries.len(), 3);
            let result = extract(&output, temp.path(), None, 1 << 20, &cancel, &|_| {}).unwrap();
            assert_eq!(
                fs::read(Path::new(&result.path).join("한글 원본/data.txt")).unwrap(),
                b"DalZip round trip\n"
            );
            crate::verification::verify_sources(
                std::slice::from_ref(&source),
                &output,
                &cancel,
                &|_| {},
            )
            .unwrap();
        }
    }
    fn encode(data: &[u8], format: &str) -> Vec<u8> {
        match format {
            "gz" => {
                let mut w = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
                w.write_all(data).unwrap();
                w.finish().unwrap()
            }
            "bz2" => {
                let mut w = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
                w.write_all(data).unwrap();
                w.finish().unwrap()
            }
            "xz" => {
                let mut w =
                    lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(1))
                        .unwrap();
                w.write_all(data).unwrap();
                w.finish().unwrap()
            }
            "zst" => zstd::stream::encode_all(data, 1).unwrap(),
            _ => data.to_vec(),
        }
    }
    #[test]
    fn tar_and_all_stream_formats_are_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        let payload = vec![b'D'; 8192];
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "data.bin", payload.as_slice())
            .unwrap();
        let tar = builder.into_inner().unwrap();
        for format in ["gz", "bz2", "xz", "zst", "tar"] {
            for is_tar in [false, true] {
                if format == "tar" && !is_tar {
                    continue;
                }
                let suffix = if is_tar && format != "tar" {
                    format!("tar.{format}")
                } else {
                    format.into()
                };
                let file = temp.path().join(format!("data.bin.{suffix}"));
                fs::write(&file, encode(if is_tar { &tar } else { &payload }, format)).unwrap();
                let result = extract(&file, temp.path(), None, 1 << 20, &cancel, &|_| {}).unwrap();
                assert_eq!(
                    fs::read(Path::new(&result.path).join("data.bin")).unwrap(),
                    payload
                );
                assert!(extract(&file, temp.path(), None, 100, &cancel, &|_| {}).is_err());
            }
        }
    }
    #[test]
    fn rar_plain_and_encrypted_streams() {
        let temp = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        for (file, password, name, data) in [
            ("version.rar", None, "VERSION", "unrar-0.4.0"),
            (
                "encrypted.rar",
                Some("unrar"),
                ".gitignore",
                "target\nCargo.lock\n",
            ),
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures")
                .join(file);
            let info = inspect(&path, None, 10000).unwrap();
            assert_eq!(info.entries.len(), 1);
            let result = extract(&path, temp.path(), password, 10000, &cancel, &|_| {}).unwrap();
            assert_eq!(
                fs::read_to_string(Path::new(&result.path).join(name)).unwrap(),
                data
            );
        }
    }
    #[test]
    fn tar_symlink_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("link.tar");
        let mut writer = tar::Builder::new(File::create(&path).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        writer
            .append_link(&mut header, "link", "../outside")
            .unwrap();
        writer.finish().unwrap();
        assert!(extract(
            &path,
            temp.path(),
            None,
            1000,
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
    #[test]
    fn encrypted_sevenz_header_and_data() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("secret.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
        writer.set_content_methods(vec![
            sevenz_rust2::encoder_options::AesEncoderOptions::new("test-password".into()).into(),
            sevenz_rust2::encoder_options::Lzma2Options::from_level(1).into(),
        ]);
        writer.set_encrypt_header(true);
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("secret.txt"),
                Some(b"checked payload".as_slice()),
            )
            .unwrap();
        writer.finish().unwrap();
        assert!(inspect(&path, None, 1000)
            .err()
            .unwrap()
            .contains("PASSWORD_REQUIRED"));
        let out = extract(
            &path,
            temp.path(),
            Some("test-password"),
            1000,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        assert_eq!(
            fs::read(Path::new(&out.path).join("secret.txt")).unwrap(),
            b"checked payload"
        );
    }
    #[test]
    fn corrupted_stream_checksum_rolls_back() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.gz");
        let mut data = encode(b"checksum payload", "gz");
        let index = data.len() - 8;
        data[index] ^= 1;
        fs::write(&path, data).unwrap();
        assert!(extract(
            &path,
            temp.path(),
            None,
            1000,
            &AtomicBool::new(false),
            &|_| {}
        )
        .is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}

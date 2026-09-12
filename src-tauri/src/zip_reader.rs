use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
    sync::Arc,
};

struct Cursor {
    file: Arc<File>,
    position: u64,
    length: u64,
}
impl Read for Cursor {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        #[cfg(unix)]
        let count = {
            use std::os::unix::fs::FileExt;
            self.file.read_at(buffer, self.position)?
        };
        #[cfg(windows)]
        let count = {
            use std::os::windows::fs::FileExt;
            self.file.seek_read(buffer, self.position)?
        };
        self.position += count as u64;
        Ok(count)
    }
}
impl Seek for Cursor {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(n) => self.position as i128 + n as i128,
            SeekFrom::End(n) => self.length as i128 + n as i128,
        };
        self.position = u64::try_from(position).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "ZIP 읽기 위치가 잘못되었습니다.",
            )
        })?;
        Ok(self.position)
    }
}
// 파일 핸들과 ZIP 중앙 디렉터리는 공유하고, 스레드별 읽기 위치와 버퍼는 독립적으로 유지합니다.
pub struct ZipReader(BufReader<Cursor>);
impl ZipReader {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let length = file.metadata()?.len();
        Ok(Self(BufReader::new(Cursor {
            file: Arc::new(file),
            position: 0,
            length,
        })))
    }
}
impl Clone for ZipReader {
    fn clone(&self) -> Self {
        let cursor = self.0.get_ref();
        Self(BufReader::new(Cursor {
            file: cursor.file.clone(),
            position: cursor.position - self.0.buffer().len() as u64,
            length: cursor.length,
        }))
    }
}
impl Read for ZipReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}
impl Seek for ZipReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.0.seek(from)
    }
}
impl BufRead for ZipReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.0.fill_buf()
    }
    fn consume(&mut self, amount: usize) {
        self.0.consume(amount);
    }
}

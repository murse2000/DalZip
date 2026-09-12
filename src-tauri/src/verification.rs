use crate::{
    archive::{err, Outcome, Progress, Result},
    formats,
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn manifest(
    sources: &[PathBuf],
    keep_root: bool,
) -> Result<BTreeMap<PathBuf, Option<blake3::Hash>>> {
    let mut result = BTreeMap::new();
    for source in sources {
        let source = source.canonicalize().map_err(err)?;
        let base = if keep_root {
            source
                .parent()
                .ok_or("파일 시스템 루트는 삭제할 수 없습니다.")?
        } else {
            &source
        };
        for item in walkdir::WalkDir::new(&source).follow_links(false) {
            let item = item.map_err(err)?;
            let relative = item.path().strip_prefix(base).map_err(err)?.to_owned();
            if relative.as_os_str().is_empty() {
                continue;
            }
            let hash = if item.file_type().is_file() {
                let mut hasher = blake3::Hasher::new();
                hasher
                    .update_reader(File::open(item.path()).map_err(err)?)
                    .map_err(err)?;
                Some(hasher.finalize())
            } else if item.file_type().is_dir() {
                None
            } else {
                return Err("검증 중 링크/특수 파일을 발견했습니다. 원본을 보존합니다.".into());
            };
            if result.insert(relative, hash).is_some() {
                return Err("원본 경로가 중복됩니다. 원본을 보존합니다.".into());
            }
        }
    }
    Ok(result)
}
pub fn verify_sources(
    sources: &[PathBuf],
    archive: &Path,
    cancel: &AtomicBool,
    emit: &(dyn Fn(Progress) + Sync),
) -> Result<()> {
    let temp = tempfile::tempdir().map_err(err)?;
    let out = formats::extract(
        archive,
        temp.path(),
        None,
        2048 * 1024 * 1024 * 1024,
        cancel,
        emit,
    )?;
    let saved = manifest(&[PathBuf::from(out.path)], false)?;
    let current = manifest(sources, true)?;
    if saved != current {
        return Err("압축 내용과 현재 원본이 일치하지 않습니다. 원본을 보존합니다.".into());
    }
    Ok(())
}
pub fn verify_then_trash(
    sources: &[PathBuf],
    archive: &Path,
    cancel: &AtomicBool,
    emit: &(dyn Fn(Progress) + Sync),
) -> Result<()> {
    verify_sources(sources, archive, cancel, emit)?;
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("원본 정리를 취소했습니다.".into());
    }
    for source in sources {
        if fs::symlink_metadata(source)
            .map_err(err)?
            .file_type()
            .is_symlink()
        {
            return Err("원본 경로가 변경되었습니다. 원본을 보존합니다.".into());
        }
    }
    let context = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    let context = {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut context = context;
        // Finder 자동화 응답을 기다리지 않고 운영체제의 휴지통 API를 사용합니다.
        context.set_delete_method(DeleteMethod::NsFileManager);
        context
    };
    context.delete_all(sources).map_err(|e| {
        format!("휴지통 이동 중 오류가 발생했습니다: {e}. 남아 있는 원본을 확인하세요.")
    })
}
pub fn attach_warning(mut outcome: Outcome, warning: Result<()>) -> Outcome {
    if let Err(error) = warning {
        outcome.warning = Some(format!("압축 파일은 생성되었습니다. {error}"));
    }
    outcome
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_source_is_never_eligible_for_trash() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        fs::write(&source, "original").unwrap();
        let output = temp.path().join("saved.zip");
        let cancel = AtomicBool::new(false);
        crate::archive::create(std::slice::from_ref(&source), &output, 1, &cancel, &|_| {})
            .unwrap();
        verify_sources(std::slice::from_ref(&source), &output, &cancel, &|_| {}).unwrap();
        fs::write(&source, "modified").unwrap();
        assert!(verify_sources(&[source], &output, &cancel, &|_| {}).is_err());
    }
}

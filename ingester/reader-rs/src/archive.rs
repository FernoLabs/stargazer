use anyhow::{bail, Context, Result};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

pub const ARCHIVE_SUFFIX: &str = ".xdr.zst";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    pub path: PathBuf,
    pub sequence: u32,
}

pub fn parse_archive_filename(path: impl AsRef<Path>) -> Result<ArchiveFile> {
    let path = path.as_ref();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("archive path has no UTF-8 filename")?;
    let stem = name
        .strip_suffix(ARCHIVE_SUFFIX)
        .with_context(|| format!("unsupported archive filename: {name}"))?;
    let (_, sequence) = stem
        .rsplit_once("--")
        .with_context(|| format!("archive filename has no sequence suffix: {name}"))?;
    let sequence = sequence
        .parse::<u32>()
        .with_context(|| format!("archive filename has invalid ledger sequence: {name}"))?;
    Ok(ArchiveFile {
        path: path.to_path_buf(),
        sequence,
    })
}

fn partition_start(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let (_, range) = name.rsplit_once("--")?;
    range.split_once('-')?.0.parse().ok()
}

fn files_in(directory: &Path) -> Result<Vec<ArchiveFile>> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(directory).with_context(|| format!("reading {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("reading entry in {}", directory.display()))?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.to_string_lossy().ends_with(ARCHIVE_SUFFIX) {
            files.push(parse_archive_filename(path)?);
        }
    }
    files.sort_unstable_by_key(|file| file.sequence);
    Ok(files)
}

/// Iterates an archive one partition at a time. Only the current partition's
/// paths (normally at most 64,000) are held in memory.
pub struct ArchiveWalker {
    partitions: VecDeque<PathBuf>,
    current: std::vec::IntoIter<ArchiveFile>,
}

impl ArchiveWalker {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if !root.is_dir() {
            bail!("archive root is not a directory: {}", root.display());
        }

        let mut partitions = Vec::new();
        let mut has_direct_files = false;
        for entry in fs::read_dir(root).with_context(|| format!("reading {}", root.display()))? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                partitions.push(entry.path());
            } else if file_type.is_file()
                && entry.path().to_string_lossy().ends_with(ARCHIVE_SUFFIX)
            {
                has_direct_files = true;
            }
        }
        partitions.sort_unstable_by(|left, right| {
            partition_start(left)
                .unwrap_or(u32::MAX)
                .cmp(&partition_start(right).unwrap_or(u32::MAX))
                .then_with(|| left.cmp(right))
        });
        if has_direct_files {
            partitions.insert(0, root.to_path_buf());
        }

        Ok(Self {
            partitions: partitions.into(),
            current: Vec::new().into_iter(),
        })
    }
}

impl Iterator for ArchiveWalker {
    type Item = Result<ArchiveFile>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(file) = self.current.next() {
                return Some(Ok(file));
            }
            let partition = self.partitions.pop_front()?;
            match files_in(&partition) {
                Ok(files) => self.current = files.into_iter(),
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

pub fn discover_files(root: impl AsRef<Path>) -> Result<Vec<ArchiveFile>> {
    ArchiveWalker::new(root)?.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_galexie_filename() {
        let file = parse_archive_filename(
            "/archive/FC3645FF--63552000-63615999/FC35574D--63613106.xdr.zst",
        )
        .expect("valid Galexie filename");
        assert_eq!(file.sequence, 63_613_106);
    }

    #[test]
    fn rejects_non_archive_filename() {
        assert!(parse_archive_filename("/archive/ledger-1.json").is_err());
        assert!(parse_archive_filename("/archive/partition--not-a-number.xdr.zst").is_err());
    }
}

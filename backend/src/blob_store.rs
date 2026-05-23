use anyhow::{Context, Result, bail};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct BlobStore {
    root: Option<Arc<PathBuf>>,
}

impl BlobStore {
    pub fn new(root: Option<PathBuf>) -> Result<Self> {
        if let Some(root) = root.as_ref()
            && root.as_os_str().is_empty()
        {
            bail!("blob store root cannot be empty");
        }

        Ok(Self {
            root: root.map(Arc::new),
        })
    }

    pub fn enabled(&self) -> bool {
        self.root.is_some()
    }

    pub async fn put_bytes(&self, key: &str, bytes: &[u8]) -> Result<Option<String>> {
        let Some(path) = self.path_for_key(key)? else {
            return Ok(None);
        };

        if fs::metadata(&path).await.is_ok() {
            return Ok(Some(key.to_owned()));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create blob store directory {}", parent.display()))?;
        }

        let tmp_path = path.with_file_name(format!(
            ".{}.{}.tmp",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("blob"),
            Uuid::new_v4()
        ));

        fs::write(&tmp_path, bytes)
            .await
            .with_context(|| format!("write temporary blob {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &path)
            .await
            .with_context(|| format!("promote blob {}", path.display()))?;

        Ok(Some(key.to_owned()))
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let Some(path) = self.path_for_key(key)? else {
            return Ok(None);
        };

        match fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("read blob {}", path.display())),
        }
    }

    fn path_for_key(&self, key: &str) -> Result<Option<PathBuf>> {
        let Some(root) = self.root.as_ref() else {
            return Ok(None);
        };

        if key.trim().is_empty() {
            bail!("blob key cannot be empty");
        }

        let relative = Path::new(key);
        for component in relative.components() {
            match component {
                Component::Normal(_) => {}
                Component::CurDir
                | Component::ParentDir
                | Component::RootDir
                | Component::Prefix(_) => {
                    bail!("blob key must be relative and normalized: {key}");
                }
            }
        }

        Ok(Some(root.join(relative)))
    }
}

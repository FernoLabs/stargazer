use anyhow::{Context, Result, bail};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{
    Method, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::fs;
use tracing::warn;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct BlobStore {
    local_root: Option<Arc<PathBuf>>,
    s3: Option<Arc<S3BlobStore>>,
}

#[derive(Clone)]
pub struct S3BlobStoreConfig {
    pub endpoint_url: String,
    pub bucket: String,
    pub prefix: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

impl fmt::Debug for S3BlobStoreConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("S3BlobStoreConfig")
            .field("endpoint_url", &self.endpoint_url)
            .field("bucket", &self.bucket)
            .field("prefix", &self.prefix)
            .field("access_key_id", &redacted_value(&self.access_key_id))
            .field("secret_access_key", &"<redacted>")
            .field("region", &self.region)
            .finish()
    }
}

#[derive(Clone, Debug)]
struct S3BlobStore {
    endpoint_url: Url,
    bucket: String,
    prefix: Option<String>,
    access_key_id: String,
    secret_access_key: String,
    region: String,
    client: reqwest::Client,
}

impl BlobStore {
    pub fn new(local_root: Option<PathBuf>, s3_config: Option<S3BlobStoreConfig>) -> Result<Self> {
        if let Some(root) = local_root.as_ref()
            && root.as_os_str().is_empty()
        {
            bail!("blob store root cannot be empty");
        }

        Ok(Self {
            local_root: local_root.map(Arc::new),
            s3: s3_config.map(S3BlobStore::new).transpose()?.map(Arc::new),
        })
    }

    pub fn enabled(&self) -> bool {
        self.local_root.is_some() || self.s3.is_some()
    }

    pub async fn put_bytes(&self, key: &str, bytes: &[u8]) -> Result<Option<String>> {
        validate_blob_key(key)?;

        let mut stored = false;
        if let Some(path) = self.local_path_for_key(key) {
            put_local_bytes(&path, bytes).await?;
            stored = true;
        }

        if let Some(s3) = self.s3.as_ref() {
            match s3.put_bytes(key, bytes).await {
                Ok(()) => stored = true,
                Err(error) if stored => {
                    warn!(
                        blob_key = key,
                        error = %error,
                        "failed to mirror blob to S3; local blob remains available",
                    );
                }
                Err(error) => return Err(error),
            }
        }

        if stored {
            Ok(Some(key.to_owned()))
        } else {
            return Ok(None);
        }
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        validate_blob_key(key)?;

        if let Some(path) = self.local_path_for_key(key) {
            match fs::read(&path).await {
                Ok(bytes) => return Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| format!("read blob {}", path.display()));
                }
            }
        }

        if let Some(s3) = self.s3.as_ref()
            && let Some(bytes) = s3.get_bytes(key).await?
        {
            if let Some(path) = self.local_path_for_key(key)
                && let Err(error) = put_local_bytes(&path, &bytes).await
            {
                warn!(
                    blob_key = key,
                    path = %path.display(),
                    error = %error,
                    "failed to hydrate local blob cache from S3",
                );
            }
            return Ok(Some(bytes));
        }

        Ok(None)
    }

    fn local_path_for_key(&self, key: &str) -> Option<PathBuf> {
        self.local_root
            .as_ref()
            .map(|root| root.join(Path::new(key)))
    }
}

impl S3BlobStore {
    fn new(config: S3BlobStoreConfig) -> Result<Self> {
        let endpoint_url =
            Url::parse(config.endpoint_url.trim()).context("parse S3 blob endpoint URL")?;
        let bucket = config.bucket.trim();
        if bucket.is_empty() {
            bail!("S3 blob bucket cannot be empty");
        }
        validate_s3_key_component(bucket, "S3 blob bucket")?;

        let prefix = config
            .prefix
            .as_deref()
            .map(str::trim)
            .map(|value| value.trim_matches('/'))
            .filter(|value| !value.is_empty())
            .map(|value| {
                validate_blob_key(value)?;
                validate_s3_object_key(value, "S3 blob prefix")?;
                Ok::<_, anyhow::Error>(value.to_owned())
            })
            .transpose()?;

        if config.access_key_id.trim().is_empty() {
            bail!("S3 blob access key cannot be empty");
        }
        if config.secret_access_key.trim().is_empty() {
            bail!("S3 blob secret key cannot be empty");
        }
        if config.region.trim().is_empty() {
            bail!("S3 blob region cannot be empty");
        }

        Ok(Self {
            endpoint_url,
            bucket: bucket.to_owned(),
            prefix,
            access_key_id: config.access_key_id.trim().to_owned(),
            secret_access_key: config.secret_access_key.trim().to_owned(),
            region: config.region.trim().to_owned(),
            client: reqwest::Client::new(),
        })
    }

    async fn put_bytes(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let payload_hash = sha256_hex(bytes);
        let object_key = self.object_key(key)?;
        let (url, canonical_uri) = self.request_url(&object_key)?;
        let content_type = content_type_for_key(&object_key);
        let mut headers = self.signed_headers(
            Method::PUT,
            &url,
            &canonical_uri,
            &payload_hash,
            &[
                ("cache-control", "public, max-age=31536000, immutable"),
                ("content-type", content_type),
            ],
        )?;

        headers.insert(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_str(content_type)?,
        );

        let response = self
            .client
            .put(url)
            .headers(headers)
            .body(bytes.to_vec())
            .send()
            .await
            .with_context(|| format!("put S3 blob {object_key}"))?;

        ensure_success(response, "put", &object_key).await?;
        Ok(())
    }

    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let object_key = self.object_key(key)?;
        let (url, canonical_uri) = self.request_url(&object_key)?;
        let payload_hash = sha256_hex(&[]);
        let headers = self.signed_headers(Method::GET, &url, &canonical_uri, &payload_hash, &[])?;

        let response = self
            .client
            .get(url)
            .headers(headers)
            .send()
            .await
            .with_context(|| format!("get S3 blob {object_key}"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        ensure_success(response, "get", &object_key)
            .await?
            .bytes()
            .await
            .map(|bytes| Some(bytes.to_vec()))
            .with_context(|| format!("read S3 blob body {object_key}"))
    }

    fn object_key(&self, key: &str) -> Result<String> {
        validate_s3_object_key(key, "S3 blob key")?;
        match self.prefix.as_deref() {
            Some(prefix) => Ok(format!("{prefix}/{key}")),
            None => Ok(key.to_owned()),
        }
    }

    fn request_url(&self, object_key: &str) -> Result<(Url, String)> {
        let mut url = self.endpoint_url.clone();
        let endpoint_path = url.path().trim_end_matches('/');
        let bucket_path = format!("{}/{}", self.bucket, object_key);
        let canonical_uri = if endpoint_path.is_empty() {
            format!("/{bucket_path}")
        } else {
            format!("{endpoint_path}/{bucket_path}")
        };
        url.set_path(&canonical_uri);
        Ok((url, canonical_uri))
    }

    fn signed_headers(
        &self,
        method: Method,
        url: &Url,
        canonical_uri: &str,
        payload_hash: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<HeaderMap> {
        let now = Utc::now();
        let date = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let scope = format!("{}/{}/s3/aws4_request", date, self.region);

        let mut canonical_headers = BTreeMap::new();
        canonical_headers.insert("host".to_owned(), host_header(url)?);
        canonical_headers.insert("x-amz-content-sha256".to_owned(), payload_hash.to_owned());
        canonical_headers.insert("x-amz-date".to_owned(), amz_date.clone());
        for (name, value) in extra_headers {
            canonical_headers.insert(name.to_ascii_lowercase(), normalize_header_value(value));
        }

        let signed_headers = canonical_headers
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(";");
        let canonical_header_text = canonical_headers
            .iter()
            .map(|(name, value)| format!("{name}:{value}\n"))
            .collect::<String>();
        let canonical_request = format!(
            "{}\n{}\n\n{}\n{}\n{}",
            method.as_str(),
            canonical_uri,
            canonical_header_text,
            signed_headers,
            payload_hash
        );
        let canonical_hash = sha256_hex(canonical_request.as_bytes());
        let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{canonical_hash}");
        let signing_key = signing_key(&self.secret_access_key, &date, &self.region);
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key_id, scope, signed_headers, signature
        );

        let mut headers = HeaderMap::new();
        for (name, value) in canonical_headers {
            if name == "host" {
                continue;
            }
            headers.insert(
                HeaderName::from_bytes(name.as_bytes())?,
                HeaderValue::from_str(&value)?,
            );
        }
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&authorization)?,
        );

        Ok(headers)
    }
}

async fn put_local_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    match fs::metadata(path).await {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("stat blob {}", path.display()));
        }
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
    fs::rename(&tmp_path, path)
        .await
        .with_context(|| format!("promote blob {} to {}", tmp_path.display(), path.display()))?;

    Ok(())
}

async fn ensure_success(
    response: reqwest::Response,
    action: &str,
    object_key: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let preview = body.chars().take(500).collect::<String>();
    bail!("S3 blob {action} failed for {object_key}: {status} {preview}");
}

fn validate_blob_key(key: &str) -> Result<()> {
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

    Ok(())
}

fn validate_s3_object_key(key: &str, label: &str) -> Result<()> {
    for component in key.split('/') {
        validate_s3_key_component(component, label)?;
    }
    Ok(())
}

fn validate_s3_key_component(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{label} cannot contain empty path components");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("{label} can only contain ASCII letters, digits, dashes, underscores, and dots");
    }
    Ok(())
}

fn host_header(url: &Url) -> Result<String> {
    let host = url
        .host_str()
        .context("S3 endpoint URL is missing a host")?;
    match url.port() {
        Some(port)
            if !(url.scheme() == "https" && port == 443)
                && !(url.scheme() == "http" && port == 80) =>
        {
            Ok(format!("{host}:{port}"))
        }
        _ => Ok(host.to_owned()),
    }
}

fn normalize_header_value(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn signing_key(secret_access_key: &str, date: &str, region: &str) -> Vec<u8> {
    let date_key = hmac_sha256(
        format!("AWS4{secret_access_key}").as_bytes(),
        date.as_bytes(),
    );
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, b"s3");
    hmac_sha256(&service_key, b"aws4_request")
}

fn hmac_sha256(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(value);
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn content_type_for_key(key: &str) -> &'static str {
    if key.ends_with(".asm") {
        "text/plain; charset=utf-8"
    } else if key.ends_with(".json") {
        "application/json; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn redacted_value(value: &str) -> String {
    if value.len() <= 6 {
        return "<redacted>".to_owned();
    }
    format!("{}...{}", &value[..3], &value[value.len() - 3..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s3_config(prefix: Option<&str>) -> S3BlobStoreConfig {
        S3BlobStoreConfig {
            endpoint_url: "https://example.r2.cloudflarestorage.com".to_owned(),
            bucket: "firewatch-blobs".to_owned(),
            prefix: prefix.map(str::to_owned),
            access_key_id: "access-key".to_owned(),
            secret_access_key: "secret-key".to_owned(),
            region: "auto".to_owned(),
        }
    }

    #[tokio::test]
    async fn stores_and_reads_local_blobs() -> Result<()> {
        let suffix = Uuid::new_v4();
        let blob_dir = std::env::temp_dir().join(format!("firewatch-blob-store-{suffix}"));
        let store = BlobStore::new(Some(blob_dir.clone()), None)?;
        let key = "disassembly/program/hash.asm";

        assert_eq!(
            store.put_bytes(key, b"asm text").await?,
            Some(key.to_owned())
        );
        assert_eq!(
            store.get_bytes(key).await?.as_deref(),
            Some(&b"asm text"[..])
        );
        assert_eq!(store.get_bytes("missing/blob.bin").await?, None);

        let _ = std::fs::remove_dir_all(blob_dir);
        Ok(())
    }

    #[test]
    fn applies_s3_prefix_to_logical_blob_key() -> Result<()> {
        let store = S3BlobStore::new(s3_config(Some("/blobs/")))?;

        assert_eq!(
            store.object_key("disassembly/program/hash.asm")?,
            "blobs/disassembly/program/hash.asm"
        );
        Ok(())
    }

    #[test]
    fn rejects_unsafe_s3_prefixes() {
        let error = S3BlobStore::new(s3_config(Some("../blobs"))).unwrap_err();
        assert!(error.to_string().contains("relative and normalized"));
    }
}

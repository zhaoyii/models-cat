//! 一些工具
use reqwest::blocking;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs::File, io::Read};
use thiserror::Error;

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum OpsError {
    /// We failed to acquire lock for file `f`. Meaning
    /// Someone else is writing/downloading said file
    #[error("Lock acquisition failed: {0}")]
    LockAcquisition(PathBuf),

    #[error("Build error {0}")]
    BuildError(String),

    #[error("Hub error {0}")]
    HubError(String),

    /// I/O Error
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    /// request error
    #[error("Request error {0}")]
    RequestError(#[from] reqwest::Error),
}

/// A static HTTP client for making blocking requests.
///
/// Uses a custom user agent and allows up to 10 redirects.
/// The client is lazily initialized using `LazyLock` to ensure
/// it is only created when first accessed.
pub(crate) static BLOCKING_CLIENT: LazyLock<blocking::Client> = LazyLock::new(|| {
    blocking::Client::builder()
        .user_agent("curl/7.79.1")
        .redirect(reqwest::redirect::Policy::limited(10)) // 自定义重定向次数
        .build()
        .expect("Failed to build reqwest client")
});

pub(crate) fn sha256(file_path: impl AsRef<Path>) -> Result<String, std::io::Error> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 1024 * 8];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sha256() {
        let testfile = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sha256-testfile.txt");
        let sha256 = super::sha256(testfile).unwrap();
        assert_eq!(
            sha256,
            "c2aeccc42d2a579c281daae7e464a14d747924159e28617ad01850f0dd1bd135"
        );
    }
}

//! 一些工具
use reqwest::blocking;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use std::{fs::File, io::Read};

/// A static HTTP client for making blocking requests.
///
/// Uses a custom user agent and allows up to 10 redirects.
/// The client is lazily initialized using `LazyLock` to ensure
/// it is only created when first accessed.
pub static BLOCKING_CLIENT: LazyLock<blocking::Client> = LazyLock::new(|| {
    blocking::Client::builder()
        .user_agent("curl/7.79.1")
        .redirect(reqwest::redirect::Policy::limited(10)) // 自定义重定向次数
        .build()
        .expect("Failed to build reqwest client")
});

pub fn sha256(file_path: &str) -> Result<String, std::io::Error> {
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

//! 一些工具
use reqwest::blocking;
use std::sync::LazyLock;

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

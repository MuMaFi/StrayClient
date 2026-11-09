use std::sync::OnceLock;
use std::time::Duration;

pub const USER_AGENT: &str = concat!(
    "StrayLauncher/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/MuMaFi/StrayClient)"
);

/// One shared connection pool for the whole app (HTTP/2 + keep-alive = far fewer TLS handshakes).
pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .pool_max_idle_per_host(64)
            .build()
            .expect("http client")
    })
}

pub async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> crate::Result<T> {
    Ok(client()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

//! Microsoft account login (device-code flow) → Xbox Live → XSTS → Minecraft.
//!
//! Primary flow (like Lunar Client): the app opens Microsoft's own sign-in page in a separate
//! window, the user signs in there, and we catch the redirect carrying the authorization code
//! (OAuth 2.0 authorization code + PKCE). The device-code flow is kept as a fallback.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::paths::Paths;
use crate::{http, Error, Result};

const MS_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

/// Azure application (client) id. Set `STRAY_MSA_CLIENT_ID` at build time (GitHub secret).
pub fn client_id() -> Option<&'static str> {
    option_env!("STRAY_MSA_CLIENT_ID").filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub uuid: String,
    pub xuid: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub access_token: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub refresh_token: String,
    pub expires_at: u64,
}

/// What the UI gets – never the tokens.
#[derive(Debug, Clone, Serialize)]
pub struct PublicAccount {
    pub name: String,
    pub uuid: String,
}

impl Account {
    pub fn public(&self) -> PublicAccount {
        PublicAccount {
            name: self.name.clone(),
            uuid: self.uuid.clone(),
        }
    }
    pub fn expired(&self) -> bool {
        now() + 300 >= self.expires_at
    }
    /// The active account.
    pub async fn load(paths: &Paths) -> Option<Account> {
        let store = AccountStore::load(paths).await;
        store.active().cloned()
    }
    /// Add or update this account and make it the active one.
    pub async fn save(&self, paths: &Paths) -> Result<()> {
        let mut store = AccountStore::load(paths).await;
        store.accounts.retain(|a| a.uuid != self.uuid);
        store.accounts.push(self.clone());
        store.active = Some(self.uuid.clone());
        store.save(paths).await
    }
    /// Remove the active account (the next one becomes active).
    pub async fn delete(paths: &Paths) {
        let mut store = AccountStore::load(paths).await;
        if let Some(uuid) = store.active.clone() {
            store.remove(&uuid);
            let _ = store.save(paths).await;
        }
    }
}

/// Several Microsoft accounts with one active, like Lunar Client's account switcher.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AccountStore {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountList {
    pub active: Option<String>,
    pub accounts: Vec<PublicAccount>,
}

impl AccountStore {
    pub async fn load(paths: &Paths) -> Self {
        if let Ok(b) = tokio::fs::read(paths.accounts_file()).await {
            if let Ok(store) = serde_json::from_slice(&b) {
                return store;
            }
        }
        // Migrate the single-account file from earlier versions.
        if let Ok(b) = tokio::fs::read(paths.account_file()).await {
            if let Ok(acc) = serde_json::from_slice::<Account>(&b) {
                return AccountStore {
                    active: Some(acc.uuid.clone()),
                    accounts: vec![acc],
                };
            }
        }
        AccountStore::default()
    }

    pub async fn save(&self, paths: &Paths) -> Result<()> {
        tokio::fs::create_dir_all(&paths.root).await?;
        let file = paths.accounts_file();
        tokio::fs::write(&file, serde_json::to_vec(self)?).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).await?;
        }
        let _ = tokio::fs::remove_file(paths.account_file()).await;
        Ok(())
    }

    pub fn active(&self) -> Option<&Account> {
        let uuid = self.active.as_deref()?;
        self.accounts.iter().find(|a| a.uuid == uuid)
    }

    pub fn list(&self) -> AccountList {
        AccountList {
            active: self.active().map(|a| a.uuid.clone()),
            accounts: self.accounts.iter().map(Account::public).collect(),
        }
    }

    pub fn switch(&mut self, uuid: &str) -> bool {
        let ok = self.accounts.iter().any(|a| a.uuid == uuid);
        if ok {
            self.active = Some(uuid.to_string());
        }
        ok
    }

    pub fn remove(&mut self, uuid: &str) {
        self.accounts.retain(|a| a.uuid != uuid);
        if self.active.as_deref() == Some(uuid) || self.active().is_none() {
            self.active = self.accounts.first().map(|a| a.uuid.clone());
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn require_client_id() -> Result<&'static str> {
    client_id().ok_or_else(|| Error::Auth("no_client_id".into()))
}

/// Microsoft's standard redirect for native/desktop apps. Register it in Azure under
/// "Mobile and desktop applications".
pub const REDIRECT_URI: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

/// PKCE pair for one sign-in attempt.
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl Pkce {
    pub fn new() -> Self {
        use base64::Engine;
        use sha2::Digest;
        // 64 random hex chars (two v4 UUIDs = 244 random bits) – within RFC 7636's 43–128 range.
        let verifier = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(verifier.as_bytes()));
        Pkce {
            verifier,
            challenge,
            state: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

/// URL of Microsoft's sign-in page for the popup window.
pub fn authorize_url(pkce: &Pkce) -> Result<String> {
    let cid = require_client_id()?;
    Ok(format!(
        "{MS_BASE}/authorize?client_id={cid}&response_type=code&redirect_uri={}&scope={}\
         &code_challenge={}&code_challenge_method=S256&state={}&prompt=select_account",
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(SCOPE),
        pkce.challenge,
        pkce.state
    ))
}

/// Result of inspecting a URL the sign-in window navigated to.
pub enum Redirect {
    /// Not our redirect – let the window keep navigating.
    NotOurs,
    Code(String),
    Error(String),
}

pub fn parse_redirect(url: &str, pkce: &Pkce) -> Redirect {
    let Some(query) = url
        .strip_prefix(REDIRECT_URI)
        .and_then(|r| r.strip_prefix('?'))
    else {
        return Redirect::NotOurs;
    };
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = urlencoding::decode(v)
            .map(|c| c.into_owned())
            .unwrap_or_default();
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "error" => error = Some(v),
            _ => {}
        }
    }
    if let Some(e) = error {
        return Redirect::Error(e);
    }
    if state.as_deref() != Some(pkce.state.as_str()) {
        return Redirect::Error("state_mismatch".into());
    }
    code.map(Redirect::Code)
        .unwrap_or_else(|| Redirect::Error("no_code".into()))
}

/// Exchange the authorization code from the popup for a Minecraft account.
pub async fn exchange_code(code: &str, pkce: &Pkce) -> Result<Account> {
    let cid = require_client_id()?;
    let t: MsToken = http::client()
        .post(format!("{MS_BASE}/token"))
        .form(&[
            ("client_id", cid),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", pkce.verifier.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?
        .json()
        .await?;
    match t.access_token {
        Some(at) => minecraft_login(&at, t.refresh_token.unwrap_or_default()).await,
        None => Err(Error::Auth(
            t.error.unwrap_or_else(|| "token_failed".into()),
        )),
    }
}

pub async fn start_device_code() -> Result<DeviceCode> {
    let cid = require_client_id()?;
    let res = http::client()
        .post(format!("{MS_BASE}/devicecode"))
        .form(&[("client_id", cid), ("scope", SCOPE)])
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(Error::Auth(res.text().await.unwrap_or_default()));
    }
    Ok(res.json().await?)
}

#[derive(Deserialize)]
struct MsToken {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
}

/// Poll until the user finished signing in on microsoft.com/link.
pub async fn poll_device_code(code: &DeviceCode) -> Result<Account> {
    let cid = require_client_id()?;
    let deadline = now() + code.expires_in;
    let mut interval = code.interval.max(1);
    while now() < deadline {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let t: MsToken = http::client()
            .post(format!("{MS_BASE}/token"))
            .form(&[
                ("client_id", cid),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", code.device_code.as_str()),
            ])
            .send()
            .await?
            .json()
            .await?;
        match (t.access_token, t.error.as_deref()) {
            (Some(at), _) => {
                return minecraft_login(&at, t.refresh_token.unwrap_or_default()).await
            }
            (None, Some("authorization_pending")) => continue,
            (None, Some("slow_down")) => interval += 5,
            (None, Some(e)) => return Err(Error::Auth(e.to_string())),
            (None, None) => return Err(Error::Auth("unknown".into())),
        }
    }
    Err(Error::Auth("expired_token".into()))
}

pub async fn refresh(acc: &Account) -> Result<Account> {
    let cid = require_client_id()?;
    let t: MsToken = http::client()
        .post(format!("{MS_BASE}/token"))
        .form(&[
            ("client_id", cid),
            ("grant_type", "refresh_token"),
            ("refresh_token", acc.refresh_token.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?
        .json()
        .await?;
    match t.access_token {
        Some(at) => {
            minecraft_login(
                &at,
                t.refresh_token.unwrap_or_else(|| acc.refresh_token.clone()),
            )
            .await
        }
        None => Err(Error::Auth(
            t.error.unwrap_or_else(|| "refresh_failed".into()),
        )),
    }
}

/// Returns a valid account, refreshing and persisting it if needed.
pub async fn valid_account(paths: &Paths) -> Result<Account> {
    let acc = Account::load(paths).await.ok_or(Error::NotLoggedIn)?;
    if !acc.expired() {
        return Ok(acc);
    }
    let fresh = refresh(&acc).await?;
    fresh.save(paths).await?;
    Ok(fresh)
}

async fn minecraft_login(ms_access: &str, refresh_token: String) -> Result<Account> {
    let c = http::client();
    let xbl: serde_json::Value = c
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let xbl_token = xbl["Token"]
        .as_str()
        .ok_or_else(|| Error::Auth("xbl".into()))?;

    let xsts_res = c
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    let xsts: serde_json::Value = xsts_res.json().await?;
    if let Some(code) = xsts["XErr"].as_u64() {
        // 2148916233 = no Xbox account, 2148916238 = child account needs family approval.
        return Err(Error::Auth(format!("xsts_{code}")));
    }
    let xsts_token = xsts["Token"]
        .as_str()
        .ok_or_else(|| Error::Auth("xsts".into()))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .unwrap_or_default();

    let mc: serde_json::Value = c
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mc_token = mc["access_token"]
        .as_str()
        .ok_or_else(|| Error::Auth("mc".into()))?
        .to_string();
    let expires = mc["expires_in"].as_u64().unwrap_or(86400);

    let profile_res = c
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_token)
        .send()
        .await?;
    if profile_res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::Auth("no_minecraft".into()));
    }
    let profile: serde_json::Value = profile_res.error_for_status()?.json().await?;

    Ok(Account {
        name: profile["name"].as_str().unwrap_or_default().to_string(),
        uuid: profile["id"].as_str().unwrap_or_default().to_string(),
        xuid: xbl["DisplayClaims"]["xui"][0]["uhs"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        access_token: mc_token,
        refresh_token,
        expires_at: now() + expires,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(name: &str) -> Account {
        Account {
            name: name.into(),
            uuid: format!("uuid-{name}"),
            xuid: String::new(),
            access_token: "t".into(),
            refresh_token: "r".into(),
            expires_at: 0,
        }
    }

    #[tokio::test]
    async fn multiple_accounts() {
        let dir = std::env::temp_dir().join(format!("stray-acc-{}", uuid::Uuid::new_v4().simple()));
        let paths = Paths::new(dir.clone());
        acc("Alice").save(&paths).await.unwrap();
        acc("Bob").save(&paths).await.unwrap();
        let mut store = AccountStore::load(&paths).await;
        assert_eq!(store.accounts.len(), 2);
        assert_eq!(store.active().unwrap().name, "Bob"); // newest sign-in is active
        assert!(store.switch("uuid-Alice"));
        store.save(&paths).await.unwrap();
        assert_eq!(Account::load(&paths).await.unwrap().name, "Alice");
        acc("Alice").save(&paths).await.unwrap(); // re-login updates, no duplicate
        assert_eq!(AccountStore::load(&paths).await.accounts.len(), 2);
        Account::delete(&paths).await; // removes the active one
        let store = AccountStore::load(&paths).await;
        assert_eq!(store.list().accounts.len(), 1);
        assert_eq!(store.active().unwrap().name, "Bob");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pkce_shape() {
        let p = Pkce::new();
        assert_eq!(p.verifier.len(), 64);
        assert_eq!(p.challenge.len(), 43); // base64url(sha256) without padding
        assert!(!p.challenge.contains('='));
    }

    #[test]
    fn redirect_parsing() {
        let p = Pkce::new();
        let ok = format!("{REDIRECT_URI}?code=M.C1%2Babc&state={}", p.state);
        assert!(matches!(parse_redirect(&ok, &p), Redirect::Code(c) if c == "M.C1+abc"));
        let bad = format!("{REDIRECT_URI}?code=x&state=evil");
        assert!(matches!(parse_redirect(&bad, &p), Redirect::Error(e) if e == "state_mismatch"));
        let denied = format!("{REDIRECT_URI}?error=access_denied&state={}", p.state);
        assert!(matches!(parse_redirect(&denied, &p), Redirect::Error(e) if e == "access_denied"));
        assert!(matches!(
            parse_redirect("https://login.live.com/ppsecure", &p),
            Redirect::NotOurs
        ));
    }
}

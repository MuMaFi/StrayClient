//! Microsoft account login (device-code flow) → Xbox Live → XSTS → Minecraft.
//!
//! Device code is used instead of an embedded browser: no webview cookies to manage, works
//! the same on Windows and macOS, and the user signs in on Microsoft's own page.

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
    pub async fn load(paths: &Paths) -> Option<Account> {
        let b = tokio::fs::read(paths.account_file()).await.ok()?;
        serde_json::from_slice(&b).ok()
    }
    pub async fn save(&self, paths: &Paths) -> Result<()> {
        tokio::fs::create_dir_all(&paths.root).await?;
        let file = paths.account_file();
        tokio::fs::write(&file, serde_json::to_vec(self)?).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).await?;
        }
        Ok(())
    }
    pub async fn delete(paths: &Paths) {
        let _ = tokio::fs::remove_file(paths.account_file()).await;
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

//! Minecraft Server List Ping – used for the live "Online — N players" card.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Default)]
pub struct ServerStatus {
    pub online: bool,
    pub players: u32,
    pub max: u32,
    pub version: String,
    pub protocol: i32,
    pub motd: String,
    pub ping_ms: u32,
}

#[derive(Deserialize)]
struct Raw {
    version: Option<RawVersion>,
    players: Option<RawPlayers>,
    description: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct RawVersion {
    name: String,
    protocol: i32,
}
#[derive(Deserialize)]
struct RawPlayers {
    online: u32,
    max: u32,
}

fn write_varint(buf: &mut Vec<u8>, mut v: i32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint(s: &mut TcpStream) -> Result<i32> {
    let mut out = 0i32;
    for i in 0..5 {
        let b = s.read_u8().await?;
        out |= ((b & 0x7f) as i32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(out);
        }
    }
    Err(Error::other("bad varint"))
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

/// Follow the `_minecraft._tcp` SRV record like the game does.
async fn resolve(host: &str, port: u16) -> (String, u16) {
    if let Ok(r) = hickory_resolver::TokioResolver::builder_tokio().map(|b| b.build()) {
        if let Ok(srv) = r.srv_lookup(format!("_minecraft._tcp.{host}")).await {
            if let Some(rec) = srv.iter().next() {
                return (
                    rec.target().to_utf8().trim_end_matches('.').to_string(),
                    rec.port(),
                );
            }
        }
    }
    (host.to_string(), port)
}

fn flatten_motd(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(o) => {
            let mut s = o
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(serde_json::Value::Array(extra)) = o.get("extra") {
                for e in extra {
                    s.push_str(&flatten_motd(e));
                }
            }
            s
        }
        serde_json::Value::Array(a) => a.iter().map(flatten_motd).collect(),
        _ => String::new(),
    }
}

pub async fn status(host: &str, port: u16) -> Result<ServerStatus> {
    tokio::time::timeout(Duration::from_secs(6), status_inner(host, port))
        .await
        .map_err(|_| Error::other("timeout"))?
}

async fn status_inner(host: &str, port: u16) -> Result<ServerStatus> {
    let (addr, port) = resolve(host, port).await;
    let start = Instant::now();
    let mut s = TcpStream::connect((addr.as_str(), port)).await?;
    s.set_nodelay(true)?;
    let mut hs = Vec::new();
    write_varint(&mut hs, -1); // protocol: "any"
    write_varint(&mut hs, host.len() as i32);
    hs.extend_from_slice(host.as_bytes());
    hs.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut hs, 1); // next state: status
    let mut out = packet(0, &hs);
    out.extend(packet(0, &[]));
    s.write_all(&out).await?;
    let _len = read_varint(&mut s).await?;
    let ping_ms = start.elapsed().as_millis() as u32;
    let _id = read_varint(&mut s).await?;
    let json_len = read_varint(&mut s).await? as usize;
    if json_len > 1 << 20 {
        return Err(Error::other("status too large"));
    }
    let mut buf = vec![0u8; json_len];
    s.read_exact(&mut buf).await?;
    let raw: Raw = serde_json::from_slice(&buf)?;
    Ok(ServerStatus {
        online: true,
        players: raw.players.as_ref().map_or(0, |p| p.online),
        max: raw.players.as_ref().map_or(0, |p| p.max),
        version: raw
            .version
            .as_ref()
            .map(|v| v.name.clone())
            .unwrap_or_default(),
        protocol: raw.version.as_ref().map_or(0, |v| v.protocol),
        motd: raw
            .description
            .as_ref()
            .map(flatten_motd)
            .unwrap_or_default(),
        ping_ms,
    })
}

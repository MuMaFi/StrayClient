//! PvP stats from public tier lists:
//! * CISTiers (cistiers.com): `GET /api/profile/<nickname>`, as used by their official mod.
//! * MCTiers (mctiers.com): `GET https://mctiers.net/api/v2/profile/by-name/<name>`, as used by
//!   the TierTagger mod.
//! * MCPVP (mcpvp.com): no API; the kit tiers on the public profile page `/@<name>` (the site's
//!   robots.txt allows general use, `use=reference`). Credited and linked in the UI.

use serde::Serialize;
use serde_json::Value;

use crate::{http, Error, Result};

const CISTIERS: &str = "https://cistiers.com/api/profile/";
const MCTIERS: &str = "https://mctiers.net/api/v2/profile/by-name/";
/// Public player pages (no API): the kit tiers shown there are read from the page.
const MCPVP: &str = "https://www.mcpvp.com/@";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KitTier {
    pub kit: String,
    /// `HT1` (best) … `LT5`; `R…` = retired.
    pub tier: String,
    pub points: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CisProfile {
    pub nickname: String,
    pub rank_position: Option<i64>,
    pub total_points: i64,
    pub tiers: Vec<KitTier>,
    /// MCTiers only (NA, EU, …).
    pub region: Option<String>,
}

/// Same card for both lists.
pub type TierProfile = CisProfile;

/// MCTiers ranking: tier 1–5, pos 0 = high / 1 = low, `retired` → `R` prefix.
pub fn parse_mctiers(v: &Value) -> Option<TierProfile> {
    let nickname = v.get("name")?.as_str()?.to_string();
    let mut tiers: Vec<KitTier> = v
        .get("rankings")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(kit, r)| {
                    let tier = r.get("tier")?.as_i64()?;
                    let pos = r.get("pos").and_then(Value::as_i64).unwrap_or(1);
                    let retired = r.get("retired").and_then(Value::as_bool).unwrap_or(false);
                    let name = format!(
                        "{}{}T{tier}",
                        if retired { "R" } else { "" },
                        if pos == 0 { "H" } else { "L" }
                    );
                    // the same weighting as the tier lists: HT1 best
                    let points = (12 - (tier * 2 + pos)).max(0);
                    Some(KitTier {
                        kit: kit.clone(),
                        tier: name,
                        points,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    tiers.sort_by(|a, b| b.points.cmp(&a.points).then(a.kit.cmp(&b.kit)));
    Some(TierProfile {
        nickname,
        rank_position: v.get("overall").and_then(Value::as_i64).filter(|&n| n > 0),
        total_points: v.get("points").and_then(Value::as_i64).unwrap_or(0),
        tiers,
        region: v.get("region").and_then(Value::as_str).map(str::to_string),
    })
}

/// A player's MCTiers profile; `Ok(None)` when the player isn't on the list.
pub async fn mctiers(nickname: &str) -> Result<Option<TierProfile>> {
    let name = valid_name(nickname)?;
    let resp = http::client()
        .get(format!("{MCTIERS}{name}"))
        .send()
        .await
        .map_err(|_| Error::other("stats_offline"))?;
    if matches!(resp.status().as_u16(), 404 | 400) {
        return Ok(None);
    }
    let v: Value = resp
        .error_for_status()
        .map_err(|_| Error::other("stats_offline"))?
        .json()
        .await
        .map_err(|_| Error::other("stats_offline"))?;
    Ok(parse_mctiers(&v))
}

/// Value of `attr="…"` inside one HTML tag.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&#039;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// MCPVP tier: HT/MT/LT/ST + 1–6 (1 best). Unranked kits (`???`) are skipped.
fn mcpvp_points(tier: &str) -> Option<i64> {
    let b = tier.as_bytes();
    if b.len() != 3 || b[1] != b'T' {
        return None;
    }
    let level = (b[2] as char).to_digit(10)? as i64;
    let sub = match b[0] {
        b'H' => 3,
        b'M' => 2,
        b'L' => 1,
        b'S' => 0,
        _ => return None,
    };
    Some((7 - level) * 4 + sub)
}

/// Read the kit tiers from a public MCPVP profile page.
pub fn parse_mcpvp(html: &str) -> Option<TierProfile> {
    let nickname = html
        .split("<title>")
        .nth(1)
        .and_then(|t| t.split("</title>").next())
        .and_then(|t| t.split('|').nth(1))
        .map(|n| unescape(n.trim()))
        .filter(|n| !n.is_empty() && n != "Home")?;
    let mut tiers = Vec::new();
    for part in html.split("<button").skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        if !tag.contains("class=\"ranking-kit\"") {
            continue;
        }
        let (Some(label), Some(tier)) = (attr(tag, "data-label"), attr(part, "data-tier-code"))
        else {
            continue;
        };
        if let Some(points) = mcpvp_points(tier) {
            tiers.push(KitTier {
                kit: unescape(label),
                tier: tier.to_string(),
                points,
            });
        }
    }
    tiers.sort_by(|a, b| b.points.cmp(&a.points).then(a.kit.cmp(&b.kit)));
    Some(TierProfile {
        nickname,
        rank_position: None,
        total_points: 0,
        tiers,
        region: None,
    })
}

/// A player's MCPVP tiers; `Ok(None)` when there is no profile (the site sends unknown
/// names to its home page).
pub async fn mcpvp(nickname: &str) -> Result<Option<TierProfile>> {
    let name = valid_name(nickname)?;
    let resp = http::client()
        .get(format!("{MCPVP}{name}"))
        .send()
        .await
        .map_err(|_| Error::other("stats_offline"))?;
    if resp.status().as_u16() == 404 || !resp.url().path().starts_with("/@") {
        return Ok(None);
    }
    let html = resp
        .error_for_status()
        .map_err(|_| Error::other("stats_offline"))?
        .text()
        .await
        .map_err(|_| Error::other("stats_offline"))?;
    Ok(parse_mcpvp(&html))
}

fn valid_name(nickname: &str) -> Result<&str> {
    let name = nickname.trim();
    if name.is_empty()
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::other("stats_bad_name"));
    }
    Ok(name)
}

/// Lenient parse: unknown or missing fields don't break the card.
pub fn parse_profile(v: &Value) -> Option<CisProfile> {
    let nickname = v.get("nickname")?.as_str()?.to_string();
    let stats = v.get("tierStats");
    let mut tiers: Vec<KitTier> = stats
        .and_then(|s| s.get("currentTiers"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    Some(KitTier {
                        kit: t.get("kit")?.as_str()?.to_string(),
                        tier: t.get("tier")?.as_str()?.to_string(),
                        points: t.get("points").and_then(Value::as_i64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    tiers.sort_by(|a, b| b.points.cmp(&a.points).then(a.kit.cmp(&b.kit)));
    Some(CisProfile {
        nickname,
        rank_position: v.get("rankPosition").and_then(Value::as_i64),
        total_points: stats
            .and_then(|s| s.get("totalPoints"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        tiers,
        region: None,
    })
}

/// A player's CISTiers profile; `Ok(None)` when the player isn't on the list.
pub async fn cistiers(nickname: &str) -> Result<Option<CisProfile>> {
    let name = valid_name(nickname)?;
    let resp = http::client()
        .get(format!("{CISTIERS}{name}"))
        .send()
        .await
        .map_err(|_| Error::other("stats_offline"))?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    let v: Value = resp
        .error_for_status()
        .map_err(|_| Error::other("stats_offline"))?
        .json()
        .await
        .map_err(|_| Error::other("stats_offline"))?;
    Ok(parse_profile(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_api_shape() {
        let v: Value = serde_json::from_str(
            r#"{"type":"player","nickname":"StrayZii","rankPosition":42,"id":"x",
               "tierStats":{"totalPoints":75,"currentTiers":[
                 {"kit":"sword","tier":"LT3","points":6},{"kit":"crystal","tier":"HT1","points":60},
                 {"kit":"pot","tier":"HT4","points":4},{"kit":"bad"}],"tierHistory":[]}}"#,
        )
        .unwrap();
        let p = parse_profile(&v).unwrap();
        assert_eq!(p.nickname, "StrayZii");
        assert_eq!(p.rank_position, Some(42));
        assert_eq!(p.total_points, 75);
        assert_eq!(
            p.tiers.iter().map(|t| t.tier.as_str()).collect::<Vec<_>>(),
            ["HT1", "LT3", "HT4"]
        );
        // a profile without tiers still parses
        let empty = parse_profile(&serde_json::json!({"nickname": "New"})).unwrap();
        assert!(empty.tiers.is_empty() && empty.rank_position.is_none());
    }

    #[tokio::test]
    async fn rejects_bad_names() {
        for n in ["", "a b", "../x", "toolongname_1234567"] {
            assert!(cistiers(n).await.is_err());
            assert!(mctiers(n).await.is_err());
            assert!(mcpvp(n).await.is_err());
        }
    }

    #[test]
    fn parses_mcpvp_page() {
        let html = r#"<html><head><title>MCPVP | Kylaz</title></head><body>
          <button class="ranking-kit" type="button" data-kit="early_game" data-label="Early Game" title="x">
            <img class="kit-icon" src="/e.png"><span class="kit-rank tier-color-label" data-tier-code="MT2">MT2</span></button>
          <button class="ranking-kit" type="button" data-kit="sword" data-label="Sword"><span data-tier-code="???">???</span></button>
          <button class="ranking-kit" type="button" data-kit="end_game" data-label="End Game"><span data-tier-code="HT3">HT3</span></button>
          <button class="time-btn">1D</button></body></html>"#;
        let p = parse_mcpvp(html).unwrap();
        assert_eq!(p.nickname, "Kylaz");
        assert_eq!(
            p.tiers
                .iter()
                .map(|t| format!("{}:{}", t.kit, t.tier))
                .collect::<Vec<_>>(),
            ["Early Game:MT2", "End Game:HT3"]
        );
        assert!(parse_mcpvp("<title>MCPVP | Home</title>").is_none());
        assert!(
            mcpvp_points("HT1") > mcpvp_points("MT1") && mcpvp_points("LT1") > mcpvp_points("HT2")
        );
    }

    #[test]
    fn parses_mctiers() {
        let v: Value = serde_json::from_str(
            r#"{"uuid":"x","name":"StrayZii","region":"EU","points":120,"overall":57,"badges":[],"combat_master":false,
               "rankings":{"sword":{"tier":3,"pos":1,"attained":0,"retired":false},
                           "vanilla":{"tier":1,"pos":0,"attained":0,"retired":false},
                           "mace":{"tier":2,"pos":0,"attained":0,"retired":true}}}"#,
        )
        .unwrap();
        let p = parse_mctiers(&v).unwrap();
        assert_eq!(p.nickname, "StrayZii");
        assert_eq!(p.rank_position, Some(57));
        assert_eq!(p.region.as_deref(), Some("EU"));
        assert_eq!(
            p.tiers
                .iter()
                .map(|t| format!("{}:{}", t.kit, t.tier))
                .collect::<Vec<_>>(),
            ["vanilla:HT1", "mace:RHT2", "sword:LT3"]
        );
    }
}

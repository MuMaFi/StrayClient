//! PvP stats from public tier lists:
//! * CISTiers (cistiers.com): `GET /api/profile/<nickname>`, as used by their official mod.
//! * MCTiers (mctiers.com): `GET https://mctiers.net/api/v2/profile/by-name/<name>`, as used by
//!   the TierTagger mod.

use serde::Serialize;
use serde_json::Value;

use crate::{http, Error, Result};

const CISTIERS: &str = "https://cistiers.com/api/profile/";
const MCTIERS: &str = "https://mctiers.net/api/v2/profile/by-name/";

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
        }
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

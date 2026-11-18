//! PvP stats from CISTiers (cistiers.com), the tier list of the CIS PvP community.
//! Public endpoint used by their official mod: `GET /api/profile/<nickname>`.

use serde::Serialize;
use serde_json::Value;

use crate::{http, Error, Result};

const CISTIERS: &str = "https://cistiers.com/api/profile/";

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
    })
}

/// A player's CISTiers profile; `Ok(None)` when the player isn't on the list.
pub async fn cistiers(nickname: &str) -> Result<Option<CisProfile>> {
    let name = nickname.trim();
    if name.is_empty()
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::other("stats_bad_name"));
    }
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
        }
    }
}

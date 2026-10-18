//! Who owns a Minecraft token: one call to Mojang's profile endpoint at sign-in. The token is used for that
//! call only and never stored or logged; afterwards the launcher uses the relay's own session token.

use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Dashed lowercase UUID.
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Mojang didn't accept the token.
    Unauthorized,
    /// The account doesn't own Minecraft: Java Edition.
    NoProfile,
    /// Mojang didn't answer properly.
    Unavailable(String),
}

#[derive(Deserialize)]
struct Raw {
    id: String,
    name: String,
}

/// "8667ba71b85a4004af54457a9734eed7" → "8667ba71-b85a-4004-af54-457a9734eed7"
pub fn dashed_uuid(id: &str) -> Option<String> {
    let hex: String = id.chars().filter(|c| *c != '-').collect::<String>().to_ascii_lowercase();
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]))
}

pub async fn verify(http: &reqwest::Client, url: &str, token: &str) -> Result<Profile, ProfileError> {
    if token.is_empty() || token.len() > 8192 || token.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(ProfileError::Unauthorized);
    }
    let res = http
        .get(url)
        .bearer_auth(token)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| ProfileError::Unavailable(e.without_url().to_string()))?;
    match res.status().as_u16() {
        200 => {
            let raw: Raw = res.json().await.map_err(|e| ProfileError::Unavailable(e.without_url().to_string()))?;
            let uuid = dashed_uuid(&raw.id).ok_or_else(|| ProfileError::Unavailable("bad profile id".into()))?;
            Ok(Profile { uuid, name: raw.name })
        }
        401 | 403 => Err(ProfileError::Unauthorized),
        404 => Err(ProfileError::NoProfile),
        code => Err(ProfileError::Unavailable(format!("HTTP {code}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_uuids() {
        assert_eq!(dashed_uuid("8667BA71B85A4004AF54457A9734EED7").as_deref(), Some("8667ba71-b85a-4004-af54-457a9734eed7"));
        assert_eq!(dashed_uuid("8667ba71-b85a-4004-af54-457a9734eed7").as_deref(), Some("8667ba71-b85a-4004-af54-457a9734eed7"));
        assert_eq!(dashed_uuid("nope"), None);
        assert_eq!(dashed_uuid("zz67ba71b85a4004af54457a9734eed7"), None);
    }
}

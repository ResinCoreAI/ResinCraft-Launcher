//! The invite code a host hands to friends: where to reach them and the secret the tunnel is keyed with.
//!
//! Bytes: version, 12-byte secret, expiry (unix seconds, u32), then the routes, then the first two bytes of SHA-256
//! over everything before them, so a typo is caught before anything connects. Shown as Crockford base32 in groups of
//! five; reading ignores case, spaces and dashes and takes O as 0 and I or L as 1.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use sha2::{Digest, Sha256};

use crate::bail_tr;

const VERSION: u8 = 1;
/// Codes that let offline players in: launchers that can't use one ask for an update instead of failing later.
const VERSION_OFFLINE: u8 = 2;
pub const SECRET_LEN: usize = 12;
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const TAG_V4: u8 = 1;
const TAG_V6: u8 = 2;
const TAG_RELAY: u8 = 3;
/// No address: the world lets offline players in (friends/offline.rs).
const TAG_OFFLINE: u8 = 4;
const MAX_ROUTES: usize = 8;

/// One way to reach the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// An address and port the host listens on (or that a router forwards to it).
    Direct(SocketAddr),
    /// A ResinCraft Relay name: this launcher's relay domain supplies the rest of the address.
    Relay(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub secret: [u8; SECRET_LEN],
    pub expires: u32,
    pub routes: Vec<Route>,
    /// The world lets offline players in.
    pub offline: bool,
}

impl Invite {
    pub fn encode(&self) -> String {
        let mut b = vec![if self.offline { VERSION_OFFLINE } else { VERSION }];
        b.extend_from_slice(&self.secret);
        b.extend_from_slice(&self.expires.to_be_bytes());
        for route in self.routes.iter().take(MAX_ROUTES) {
            match route {
                Route::Direct(SocketAddr::V4(a)) => {
                    b.push(TAG_V4);
                    b.extend_from_slice(&a.ip().octets());
                    b.extend_from_slice(&a.port().to_be_bytes());
                }
                Route::Direct(SocketAddr::V6(a)) => {
                    b.push(TAG_V6);
                    b.extend_from_slice(&a.ip().octets());
                    b.extend_from_slice(&a.port().to_be_bytes());
                }
                Route::Relay(name) => {
                    b.push(TAG_RELAY);
                    b.push(name.len().min(255) as u8);
                    b.extend_from_slice(&name.as_bytes()[..name.len().min(255)]);
                }
            }
        }
        if self.offline {
            b.push(TAG_OFFLINE);
        }
        let sum = Sha256::digest(&b);
        b.extend_from_slice(&sum[..2]);
        let text = to_base32(&b);
        let groups: Vec<&str> = text.as_bytes().chunks(5).map(|c| std::str::from_utf8(c).unwrap_or_default()).collect();
        groups.join("-")
    }

    pub fn decode(text: &str) -> anyhow::Result<Invite> {
        let bad = || crate::err_tr!("friends.badCode", "That code isn't right. Copy the whole code again from your friend.");
        let b = from_base32(text).ok_or_else(bad)?;
        if b.len() < 1 + SECRET_LEN + 4 + 2 {
            return Err(bad());
        }
        let (body, sum) = b.split_at(b.len() - 2);
        if Sha256::digest(body)[..2] != *sum {
            return Err(bad());
        }
        if body[0] != VERSION && body[0] != VERSION_OFFLINE {
            bail_tr!("friends.codeVersion", "This code came from a newer ResinCraft Launcher. Update the launcher and try again.");
        }
        let secret: [u8; SECRET_LEN] = body[1..1 + SECRET_LEN].try_into().map_err(|_| bad())?;
        let mut at = 1 + SECRET_LEN;
        let expires = u32::from_be_bytes(body[at..at + 4].try_into().map_err(|_| bad())?);
        at += 4;
        let mut routes = Vec::new();
        let mut offline = false;
        while at < body.len() {
            let tag = body[at];
            at += 1;
            let take = |n: usize, at: usize| body.get(at..at + n).ok_or_else(bad);
            match tag {
                TAG_V4 => {
                    let f = take(6, at)?;
                    let ip = Ipv4Addr::new(f[0], f[1], f[2], f[3]);
                    routes.push(Route::Direct(SocketAddr::new(IpAddr::V4(ip), u16::from_be_bytes([f[4], f[5]]))));
                    at += 6;
                }
                TAG_V6 => {
                    let f = take(18, at)?;
                    let octets: [u8; 16] = f[..16].try_into().map_err(|_| bad())?;
                    routes.push(Route::Direct(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), u16::from_be_bytes([f[16], f[17]]))));
                    at += 18;
                }
                TAG_RELAY => {
                    let len = take(1, at)?[0] as usize;
                    let name = std::str::from_utf8(take(len, at + 1)?).map_err(|_| bad())?;
                    routes.push(Route::Relay(name.to_string()));
                    at += 1 + len;
                }
                TAG_OFFLINE if body[0] == VERSION_OFFLINE => offline = true,
                _ => return Err(bad()),
            }
        }
        Ok(Invite { secret, expires, routes, offline })
    }
}

fn to_base32(bytes: &[u8]) -> String {
    let (mut out, mut acc, mut bits) = (String::with_capacity(bytes.len() * 8 / 5 + 1), 0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
        acc &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn from_base32(text: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (Vec::with_capacity(text.len() * 5 / 8), 0u32, 0u32);
    for c in text.chars().filter(|c| !c.is_whitespace() && *c != '-') {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let v = ALPHABET.iter().position(|a| *a as char == c)? as u32;
        acc = (acc << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Invite {
        Invite {
            secret: [7; SECRET_LEN],
            expires: 1_900_000_000,
            routes: vec![
                Route::Direct("203.0.113.9:25580".parse().unwrap()),
                Route::Direct("192.168.1.20:25580".parse().unwrap()),
                Route::Direct("[2001:db8::5]:25581".parse().unwrap()),
                Route::Relay("friend-a1b2c3".into()),
            ],
            offline: false,
        }
    }

    #[test]
    fn offline_codes_round_trip_and_plain_ones_stay_version_1() {
        let offline = Invite { offline: true, ..sample() };
        assert_eq!(Invite::decode(&offline.encode()).unwrap(), offline);
        assert_eq!(from_base32(&sample().encode()).unwrap()[0], VERSION);
        assert_eq!(from_base32(&offline.encode()).unwrap()[0], VERSION_OFFLINE);
    }

    #[test]
    fn round_trip_with_messy_typing() {
        let code = sample().encode();
        assert_eq!(Invite::decode(&code).unwrap(), sample());
        let messy = code.to_lowercase().replace('-', " ").replace('0', "o").replace('1', "l");
        assert_eq!(Invite::decode(&format!("  {messy}\n")).unwrap(), sample());
    }

    #[test]
    fn typos_and_junk_are_refused() {
        let code = sample().encode();
        let at = code.find(|c: char| c != '-').unwrap();
        let swapped = if code.as_bytes()[at] == b'2' { '3' } else { '2' };
        let mut typo = code.clone();
        typo.replace_range(at..at + 1, &swapped.to_string());
        assert!(Invite::decode(&typo).is_err());
        assert!(Invite::decode(&code[..code.len() - 7]).is_err(), "a cut-off code");
        assert!(Invite::decode("").is_err());
        assert!(Invite::decode("hello world!").is_err());
    }

    #[test]
    fn newer_codes_ask_for_an_update() {
        let mut b = vec![VERSION_OFFLINE + 1];
        b.extend_from_slice(&[0; SECRET_LEN + 4]);
        let sum = Sha256::digest(&b);
        b.extend_from_slice(&sum[..2]);
        let e = Invite::decode(&to_base32(&b)).unwrap_err();
        assert!(e.to_string().contains("newer"));
    }
}

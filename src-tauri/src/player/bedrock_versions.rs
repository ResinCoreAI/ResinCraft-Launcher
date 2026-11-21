//! The older versions of Minecraft for Windows a player can switch to, and where Microsoft keeps each one.
//!
//! There are two package formats, and Microsoft publishes no list of old versions for either:
//! - **UWP** (0.14 to 1.21.114): an `.appx` on Microsoft's update service, found by its update id. No more UWP
//!   versions will ever exist, so the list is a fixed table inside the launcher (`bedrock_uwp_versions.txt`: every id
//!   in it was answered by Microsoft's service when the table was made, with the package's size).
//! - **GDK** (1.21.120 onwards): an `.msixvc` on the Xbox CDN. Microsoft's service only ever names the current
//!   package, so the addresses of older ones exist only where somebody wrote them down at the time: the
//!   MinecraftBedrockArchiver/GdkLinks project does, a snapshot of its file is inside the launcher
//!   (`bedrock_gdk_versions.json`) and a newer copy is fetched when there is one.
//!
//! Whatever a list says, a package is only ever downloaded from Microsoft's own hosts (`allowed_gdk_url`,
//! `allowed_uwp_url`), and for GDK only under the exact file name Microsoft gives that version.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::err_tr;

/// How a version is packaged, which decides how Windows takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// 1.21.120 onwards: Windows installs Microsoft's signed package as it is (needs Gaming Services).
    Gdk,
    /// Up to 1.21.114: unpacked and registered with Windows (needs Developer Mode).
    Uwp,
}

/// Where a version's package is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Its id on Microsoft's update service.
    Uwp { update_id: String },
    /// Its addresses on the Xbox CDN (mirrors of one file).
    Gdk { urls: Vec<String> },
}

/// One version a player can switch to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// The game's version, e.g. "1.21.114.1".
    pub name: String,
    pub format: Format,
    /// The download's size in bytes, when known.
    pub size: Option<u64>,
    pub source: Source,
}

/// What the Bedrock page shows for a version.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: String,
    pub format: Format,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl From<&Version> for VersionInfo {
    fn from(v: &Version) -> Self {
        VersionInfo { version: v.name.clone(), format: v.format, size: v.size }
    }
}

// ---------- Version numbers ----------

/// "1.21.114.01" → [1, 21, 114, 1]. None when a part isn't a number.
pub fn parse_version(v: &str) -> Option<Vec<u32>> {
    let parts: Option<Vec<u32>> = v.trim().split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

/// Orders game versions by their numbers ("1.21.9" before "1.21.10"); a missing part counts as 0. Names that aren't
/// versions sort first.
pub fn compare(a: &str, b: &str) -> Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some(a), Some(b)) => {
            let len = a.len().max(b.len());
            (0..len).map(|i| a.get(i).copied().unwrap_or(0).cmp(&b.get(i).copied().unwrap_or(0))).find(|o| o.is_ne()).unwrap_or(Ordering::Equal)
        }
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// The same version written two ways ("1.21.114.01" and "1.21.114.1", "1.21.120" and "1.21.120.0").
pub fn same_version(a: &str, b: &str) -> bool {
    parse_version(a).is_some() && compare(a, b).is_eq()
}

/// "1.21.114.01" → "1.21.114.1": every part without leading zeros.
fn tidy(v: &str) -> Option<String> {
    Some(parse_version(v)?.iter().map(u32::to_string).collect::<Vec<_>>().join("."))
}

/// A GDK game version's app package version: "1.21.120.4" → "1.21.12004.0", "1.26.65.1" → "1.26.6501.0". The
/// third number carries the game's third and fourth, two digits for the fourth.
pub fn package_version(game: &str) -> Option<String> {
    let p = parse_version(game)?;
    let (major, minor, patch) = (*p.first()?, *p.get(1)?, *p.get(2)?);
    let revision = p.get(3).copied().unwrap_or(0);
    if p.len() > 4 || revision > 99 {
        return None;
    }
    Some(format!("{major}.{minor}.{}.0", patch * 100 + revision))
}

// ---------- UWP: the fixed table ----------

const UWP_TABLE: &str = include_str!("bedrock_uwp_versions.txt");

/// An update id as Microsoft's service writes it: 8-4-4-4-12 hex digits. It goes into a request, so nothing else
/// passes.
pub fn is_update_id(id: &str) -> bool {
    let groups: Vec<&str> = id.split('-').collect();
    groups.len() == 5 && groups.iter().zip([8, 4, 4, 4, 12]).all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Reads the table: one "version update-id size" per line; `#` starts a comment.
pub fn parse_uwp_table(text: &str) -> Vec<Version> {
    let mut out: Vec<Version> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let mut parts = line.split_whitespace();
        let (Some(name), Some(id)) = (parts.next().and_then(tidy), parts.next()) else { continue };
        if !is_update_id(id) || out.iter().any(|v| v.name == name) {
            continue;
        }
        let size = parts.next().and_then(|s| s.parse().ok()).filter(|s| *s > 0);
        out.push(Version { name, format: Format::Uwp, size, source: Source::Uwp { update_id: id.to_ascii_lowercase() } });
    }
    out
}

// ---------- GDK: the recorded addresses ----------

const GDK_SNAPSHOT: &str = include_str!("bedrock_gdk_versions.json");
/// The GdkLinks project's list, as GitHub serves it.
const GDK_LIST_URL: &str = "https://raw.githubusercontent.com/MinecraftBedrockArchiver/GdkLinks/refs/heads/master/urls.min.json";
/// How long a fetched copy is used before asking again.
const GDK_LIST_AGE: Duration = Duration::from_secs(6 * 3600);
const GDK_HOSTS: [&str; 2] = ["assets1.xboxlive.com", "assets2.xboxlive.com"];
/// Minecraft for Windows' app package (the GDK builds kept the UWP-era name).
const RELEASE_PACKAGE: &str = "Microsoft.MinecraftUWP";
const PUBLISHER_ID: &str = "8wekyb3d8bbwe";

/// The GdkLinks file: `{"release": {"1.21.120.4": [url, …]}, "preview": {…}}`.
#[derive(Debug, Default, Deserialize)]
struct GdkLinks {
    #[serde(default)]
    release: BTreeMap<String, Vec<String>>,
}

/// The file name Microsoft gives a release's package: "Microsoft.MinecraftUWP_1.21.12004.0_x64__8wekyb3d8bbwe.msixvc".
pub fn gdk_file_name(game: &str) -> Option<String> {
    Some(format!("{RELEASE_PACKAGE}_{}_x64__{PUBLISHER_ID}.msixvc", package_version(game)?))
}

/// Splits "http://host/path" into (scheme, host, path); no credentials, ports, queries or fragments pass.
fn split_url(url: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let (host, path) = rest.split_once('/')?;
    if host.is_empty() || host.contains(['@', ':']) || path.contains(['?', '#', '\\']) || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    Some((scheme, host, path))
}

/// Whether an address from the list is Microsoft's package for exactly this version: on the Xbox CDN, and named as
/// Microsoft names that version's package (the CDN's file names differ only in letter case).
pub fn allowed_gdk_url(url: &str, game: &str) -> bool {
    let Some((scheme, host, path)) = split_url(url) else { return false };
    let Some(expected) = gdk_file_name(game) else { return false };
    let file = path.rsplit('/').next().unwrap_or_default();
    (scheme == "http" || scheme == "https") && GDK_HOSTS.iter().any(|h| host.eq_ignore_ascii_case(h)) && !path.contains("..") && file.eq_ignore_ascii_case(&expected)
}

/// Reads a GdkLinks file, keeping releases whose addresses pass `allowed_gdk_url`.
pub fn parse_gdk_list(json: &str) -> Result<Vec<Version>> {
    let links: GdkLinks = serde_json::from_str(json)?;
    let mut out: Vec<Version> = Vec::new();
    for (name, urls) in links.release {
        let Some(tidy_name) = tidy(&name) else { continue };
        let urls: Vec<String> = urls.into_iter().filter(|u| allowed_gdk_url(u, &tidy_name)).collect();
        if urls.is_empty() || out.iter().any(|v| v.name == tidy_name) {
            continue;
        }
        out.push(Version { name: tidy_name, format: Format::Gdk, size: None, source: Source::Gdk { urls } });
    }
    Ok(out)
}

/// Every version from the lists, the newest first; a version in several lists is kept from the first that has it.
pub fn merge(lists: Vec<Vec<Version>>) -> Vec<Version> {
    let mut out: Vec<Version> = Vec::new();
    for v in lists.into_iter().flatten() {
        if !out.iter().any(|o| same_version(&o.name, &v.name)) {
            out.push(v);
        }
    }
    out.sort_by(|a, b| compare(&b.name, &a.name));
    out
}

fn gdk_cache_file() -> PathBuf {
    crate::core::paths::cache().join("bedrock-gdk-versions.json")
}

/// The GDK list: the fetched copy (asked for again when it is older than `GDK_LIST_AGE`, kept when GitHub can't be
/// reached) together with the snapshot inside the launcher, so a version once known never goes missing.
async fn gdk_versions() -> Vec<Version> {
    let file = gdk_cache_file();
    let fresh = std::fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < GDK_LIST_AGE);
    let mut fetched = None;
    if !fresh {
        if let Ok(text) = crate::core::net::fetch_text(GDK_LIST_URL).await {
            // Only a list that reads and still names versions replaces the copy on disk.
            if parse_gdk_list(&text).is_ok_and(|l| !l.is_empty()) {
                if let Some(parent) = file.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let _ = tokio::fs::write(&file, &text).await;
                fetched = Some(text);
            }
        }
    }
    let cached = match fetched {
        Some(t) => Some(t),
        None => tokio::fs::read_to_string(&file).await.ok(),
    };
    let from_cache = cached.and_then(|t| parse_gdk_list(&t).ok()).unwrap_or_default();
    let from_snapshot = parse_gdk_list(GDK_SNAPSHOT).unwrap_or_default();
    merge(vec![from_cache, from_snapshot])
}

/// Every version a player can switch to, the newest first.
pub async fn all() -> Vec<Version> {
    merge(vec![gdk_versions().await, parse_uwp_table(UWP_TABLE)])
}

/// The version with this name, or an error the page can show.
pub async fn find(name: &str) -> Result<Version> {
    all().await.into_iter().find(|v| same_version(&v.name, name)).ok_or_else(|| err_tr!("bedrock.switch.unknownVersion", "ResinCraft doesn't know where Microsoft keeps Minecraft {version}.", version = name))
}

// ---------- UWP: asking Microsoft's update service where a package is ----------

/// Microsoft's update service (the one Windows Update and the Store use).
pub const UPDATE_SERVICE_URL: &str = "https://fe3.delivery.mp.microsoft.com/ClientWebService/client.asmx/secured";

/// The request for a package's download addresses (`GetExtendedUpdateInfo2`), without an account: enough for
/// release versions. `created` and `expires` are UTC times like "2026-11-20T14:00:00.000Z". None when `update_id`
/// isn't an update id.
pub fn download_request(update_id: &str, created: &str, expires: &str) -> Option<String> {
    if !is_update_id(update_id) {
        return None;
    }
    Some(format!(
        concat!(
            r#"<s:Envelope xmlns:a="http://www.w3.org/2005/08/addressing" xmlns:s="http://www.w3.org/2003/05/soap-envelope">"#,
            r#"<s:Header>"#,
            r#"<a:Action s:mustUnderstand="1">http://www.microsoft.com/SoftwareDistribution/Server/ClientWebService/GetExtendedUpdateInfo2</a:Action>"#,
            r#"<a:MessageID>urn:uuid:{message}</a:MessageID>"#,
            r#"<a:To s:mustUnderstand="1">{url}</a:To>"#,
            r#"<o:Security s:mustUnderstand="1" xmlns:o="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd">"#,
            r#"<Timestamp xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">"#,
            r#"<Created>{created}</Created><Expires>{expires}</Expires></Timestamp>"#,
            r#"<wuws:WindowsUpdateTicketsToken wsu:id="ClientMSA" xmlns:wsu="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd" xmlns:wuws="http://schemas.microsoft.com/msus/2014/10/WindowsUpdateAuthorization">"#,
            r#"<TicketType Name="AAD" Version="1.0" Policy="MBI_SSL"></TicketType>"#,
            r#"</wuws:WindowsUpdateTicketsToken></o:Security></s:Header>"#,
            r#"<s:Body><GetExtendedUpdateInfo2 xmlns="http://www.microsoft.com/SoftwareDistribution/Server/ClientWebService">"#,
            r#"<updateIDs><UpdateIdentity><UpdateID>{id}</UpdateID><RevisionNumber>1</RevisionNumber></UpdateIdentity></updateIDs>"#,
            r#"<infoTypes><XmlUpdateFragmentType>FileUrl</XmlUpdateFragmentType></infoTypes>"#,
            r#"<deviceAttributes>E:BranchReadinessLevel=CBB&amp;CurrentBranch=rs4_release&amp;FlightRing=Retail&amp;InstallLanguage=en-US&amp;OSUILocale=en-US&amp;InstallationType=Client&amp;OSSkuId=48&amp;App=WU&amp;AppVer=10.0.17134.471&amp;OSArchitecture=AMD64&amp;IsFlightingEnabled=0&amp;TelemetryLevel=1&amp;WuClientVer=10.0.17134.471&amp;OSVersion=10.0.17134.472&amp;DeviceFamily=Windows.Desktop</deviceAttributes>"#,
            r#"</GetExtendedUpdateInfo2></s:Body></s:Envelope>"#
        ),
        message = uuid::Uuid::new_v4(),
        url = UPDATE_SERVICE_URL,
        created = created,
        expires = expires,
        id = update_id,
    ))
}

/// "&amp;" and the other XML entities back to their characters.
fn xml_text(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// A file the update service points to: where it is, and its SHA-1 as the service states it (hex). The answer
/// comes over HTTPS from Microsoft, the file itself over plain HTTP, so the digest is what makes the file trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub url: String,
    pub sha1: String,
}

/// Every file in the service's answer that has both an address and a digest (20 bytes of SHA-1, in base64).
pub fn parse_downloads(xml: &str) -> Vec<Download> {
    use base64::Engine;
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<FileLocation>") {
        rest = &rest[start + "<FileLocation>".len()..];
        let Some(end) = rest.find("</FileLocation>") else { break };
        let block = &rest[..end];
        rest = &rest[end..];
        let inner = |tag: &str| -> Option<String> {
            let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
            let from = block.find(&open)? + open.len();
            let to = block[from..].find(&close)? + from;
            Some(xml_text(block[from..to].trim()))
        };
        let (Some(url), Some(digest)) = (inner("Url"), inner("FileDigest")) else { continue };
        match base64::engine::general_purpose::STANDARD.decode(digest) {
            Ok(bytes) if bytes.len() == 20 => out.push(Download { url, sha1: hex::encode(bytes) }),
            _ => continue,
        }
    }
    out
}

/// Whether an address from the update service is on Microsoft's delivery network.
pub fn allowed_uwp_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    let host = rest.split(['/', '?']).next().unwrap_or_default().to_ascii_lowercase();
    (scheme == "http" || scheme == "https") && !host.contains(['@', ':']) && !url.chars().any(|c| c.is_whitespace() || c.is_control()) && host.ends_with(".delivery.mp.microsoft.com")
}

/// The package to download, out of the service's answer: on Microsoft's delivery network, and on the `tlu.dl` host
/// (the one that serves the whole package to anyone who has the address; the other address is a different file).
pub fn pick_download(xml: &str) -> Option<Download> {
    parse_downloads(xml).into_iter().find(|d| allowed_uwp_url(&d.url) && d.url.contains("://tlu.dl."))
}

/// Asks Microsoft's update service where a UWP version's package is and what its SHA-1 is. The address is good
/// for a short time only, so it is asked for right before the download.
pub async fn uwp_download(update_id: &str, version: &str) -> Result<Download> {
    uwp_download_with(&update_service_client()?, update_id, version).await
}

/// "Microsoft Root Certificate Authority 2011" (SHA-1 8F43288AD272F3103B6FB1428485EA3014C0BCFE, good until 2036),
/// as Windows' own trust store has it.
const MICROSOFT_ROOT_2011: &[u8] = include_bytes!("microsoft_root_2011.cer");

/// Microsoft's update service proves who it is with a certificate under Microsoft's own root, which Windows
/// trusts but the public web's list (what the launcher's HTTP client goes by) doesn't have. This client trusts
/// that one root and nothing else, and is used for that one service.
fn update_service_client() -> Result<reqwest::Client> {
    let root = reqwest::Certificate::from_der(MICROSOFT_ROOT_2011)?;
    Ok(reqwest::Client::builder()
        .tls_built_in_root_certs(false)
        .add_root_certificate(root)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()?)
}

async fn uwp_download_with(http: &reqwest::Client, update_id: &str, version: &str) -> Result<Download> {
    let now = chrono::Utc::now();
    let stamp = |t: chrono::DateTime<chrono::Utc>| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let unknown = || err_tr!("bedrock.switch.unknownVersion", "ResinCraft doesn't know where Microsoft keeps Minecraft {version}.", version = version);
    let body = download_request(update_id, &stamp(now), &stamp(now + chrono::Duration::minutes(5))).ok_or_else(unknown)?;
    let answer = http
        .post(UPDATE_SERVICE_URL)
        .header(reqwest::header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| err_tr!("bedrock.switch.serviceOffline", "Couldn't reach Microsoft's update service: {error}", error = e.to_string()))?
        .text()
        .await?;
    pick_download(&answer).ok_or_else(|| err_tr!("bedrock.switch.notServed", "Microsoft's update service no longer has Minecraft {version}.", version = version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_by_number() {
        assert_eq!(parse_version("1.21.114.01"), Some(vec![1, 21, 114, 1]));
        assert_eq!(parse_version("1.21.x"), None);
        assert_eq!(parse_version(""), None);
        assert_eq!(compare("1.21.9", "1.21.10"), Ordering::Less);
        assert_eq!(compare("1.26.65.1", "1.21.130.4"), Ordering::Greater);
        assert_eq!(compare("1.21.120", "1.21.120.0"), Ordering::Equal);
        assert_eq!(compare("0.14.0.1", "1.2.3.3"), Ordering::Less);
        assert!(same_version("1.21.114.01", "1.21.114.1"));
        assert!(!same_version("1.21.114.1", "1.21.113.1"));
        assert!(!same_version("latest", "latest"), "names that aren't versions never match");
        assert_eq!(tidy("1.20.81.01").as_deref(), Some("1.20.81.1"));
    }

    #[test]
    fn game_versions_read_as_package_versions() {
        assert_eq!(package_version("1.21.120.4").as_deref(), Some("1.21.12004.0"));
        assert_eq!(package_version("1.26.65.1").as_deref(), Some("1.26.6501.0"));
        assert_eq!(package_version("1.26.0.2").as_deref(), Some("1.26.2.0"));
        assert_eq!(package_version("1.21.120").as_deref(), Some("1.21.12000.0"));
        assert_eq!(package_version("1.21"), None);
        assert_eq!(package_version("1.21.120.100"), None, "the fourth number has two digits");
        assert_eq!(package_version("1.21.120.4.1"), None);
        assert_eq!(gdk_file_name("1.21.120.4").as_deref(), Some("Microsoft.MinecraftUWP_1.21.12004.0_x64__8wekyb3d8bbwe.msixvc"));
    }

    #[test]
    fn reads_the_uwp_table() {
        let table = "\
# version id size
1.21.114.01 3738C248-0603-4560-abf4-fdd66e7cd852 929000000
0.14.0.1 b17c2923-587a-442b-9bc6-76a13c02978c   # no size
1.21.114.1 00000000-0000-0000-0000-000000000000 5
1.20.0.1 not-an-id 5
bad 3738c248-0603-4560-abf4-fdd66e7cd852 5

";
        let v = parse_uwp_table(table);
        assert_eq!(v.len(), 2, "the repeated version, the bad id and the bad version are left out");
        assert_eq!(v[0].name, "1.21.114.1");
        assert_eq!(v[0].format, Format::Uwp);
        assert_eq!(v[0].size, Some(929_000_000));
        assert_eq!(v[0].source, Source::Uwp { update_id: "3738c248-0603-4560-abf4-fdd66e7cd852".into() });
        assert_eq!(v[1].size, None);
    }

    #[test]
    fn update_ids_are_strict() {
        assert!(is_update_id("3738c248-0603-4560-abf4-fdd66e7cd852"));
        assert!(is_update_id("985D1EE4-0E9D-49DE-9A99-E208ADC08D0C"));
        assert!(!is_update_id("3738c248-0603-4560-abf4-fdd66e7cd85"));
        assert!(!is_update_id("3738c248-0603-4560-abf4-fdd66e7cd852</UpdateID>"));
        assert!(!is_update_id("3738c24806034560abf4fdd66e7cd852"));
        assert!(!is_update_id(""));
        assert!(download_request("<x>", "a", "b").is_none());
    }

    #[test]
    fn the_table_inside_the_launcher_reads() {
        let v = parse_uwp_table(UWP_TABLE);
        assert!(v.len() >= 100, "{} versions", v.len());
        assert!(v.iter().all(|v| v.format == Format::Uwp && v.size.is_some()));
        assert!(v.iter().any(|v| v.name == "1.21.114.1"), "the last UWP release");
        // Nothing in it is a GDK version.
        assert!(v.iter().all(|v| compare(&v.name, "1.21.120") == Ordering::Less));
    }

    const CDN: &str = "http://assets1.xboxlive.com/1/7d219e7c-a46e-4afc-8176-cec808fc1e1b/7792d9ce-355a-493c-afbd-768f4a77c3b0/1.21.12004.0.0ed99274-787a-4d02-8344-7c6bd1ef77c8/";

    #[test]
    fn only_microsofts_package_for_that_version_passes() {
        let ok = format!("{CDN}MICROSOFT.MINECRAFTUWP_1.21.12004.0_x64__8wekyb3d8bbwe.msixvc");
        assert!(allowed_gdk_url(&ok, "1.21.120.4"));
        assert!(allowed_gdk_url(&ok.replace("assets1", "assets2").replace("http://", "https://"), "1.21.120.4"));
        // Another version's package under this version's name.
        assert!(!allowed_gdk_url(&ok, "1.21.121.1"));
        // Other hosts, however they are dressed up.
        for bad in [
            ok.replace("assets1.xboxlive.com", "example.com"),
            ok.replace("assets1.xboxlive.com", "assets1.xboxlive.com.example.com"),
            ok.replace("assets1.xboxlive.com", "example.com@assets1.xboxlive.com"),
            ok.replace("assets1.xboxlive.com", "assets1.xboxlive.com:8080"),
            ok.replace("http://", "ftp://"),
            ok.replace("http://", "file://"),
            format!("{ok}?x=1"),
            format!("{ok}#x"),
            ok.replace("/1/", "/../"),
            ok.replace(".msixvc", ".exe"),
            ok.replace("MINECRAFTUWP", "MinecraftWindowsBeta"),
            format!("{CDN}evil/..%2f/x.msixvc"),
            "assets1.xboxlive.com/x.msixvc".to_string(),
            String::new(),
        ] {
            assert!(!allowed_gdk_url(&bad, "1.21.120.4"), "{bad}");
        }
    }

    #[test]
    fn reads_a_gdk_list() {
        let json = format!(
            r#"{{"release": {{
                "1.21.120.4": ["{CDN}MICROSOFT.MINECRAFTUWP_1.21.12004.0_x64__8wekyb3d8bbwe.msixvc", "http://example.com/Microsoft.MinecraftUWP_1.21.12004.0_x64__8wekyb3d8bbwe.msixvc"],
                "1.21.121.1": ["http://example.com/Microsoft.MinecraftUWP_1.21.12101.0_x64__8wekyb3d8bbwe.msixvc"],
                "1.21.122.1": [],
                "soon": ["{CDN}x.msixvc"]
            }}, "preview": {{"1.21.120.21": ["{CDN}Microsoft.MinecraftWindowsBeta_1.21.12021.0_x64__8wekyb3d8bbwe.msixvc"]}}, "other": 1}}"#
        );
        let v = parse_gdk_list(&json).unwrap();
        assert_eq!(v.len(), 1, "only the release with an address on Microsoft's CDN");
        assert_eq!(v[0].name, "1.21.120.4");
        assert_eq!(v[0].format, Format::Gdk);
        let Source::Gdk { urls } = &v[0].source else { panic!("a GDK source") };
        assert_eq!(urls.len(), 1, "the address elsewhere is dropped");
        // What a broken list server sends instead (an error page), and lists with nothing usable in them.
        assert!(parse_gdk_list("<html><head><title>502 Bad Gateway</title></head></html>").is_err());
        assert!(parse_gdk_list("").is_err());
        assert!(parse_gdk_list("{}").unwrap().is_empty());
        assert!(parse_gdk_list("[]").map_or(true, |l| l.is_empty()));
    }

    #[test]
    fn the_snapshot_inside_the_launcher_reads() {
        let v = parse_gdk_list(GDK_SNAPSHOT).unwrap();
        assert!(v.len() >= 30, "{} versions", v.len());
        assert!(v.iter().any(|v| v.name == "1.21.120.4"), "the first GDK release");
        // Every release in the file passed: none was dropped for its address.
        let raw: serde_json::Value = serde_json::from_str(GDK_SNAPSHOT).unwrap();
        assert_eq!(v.len(), raw["release"].as_object().unwrap().len());
        assert!(v.iter().all(|v| compare(&v.name, "1.21.120") != Ordering::Less));
    }

    #[test]
    fn lists_merge_newest_first() {
        let uwp = parse_uwp_table("1.21.114.1 3738c248-0603-4560-abf4-fdd66e7cd852 5\n1.20.81.1 39b4a4c9-cd0a-49c2-971f-156aa4dbbc01 6\n");
        let gdk = |name: &str, host: &str| Version { name: name.into(), format: Format::Gdk, size: None, source: Source::Gdk { urls: vec![host.into()] } };
        let all = merge(vec![vec![gdk("1.21.120.4", "new")], vec![gdk("1.26.65.1", "snap"), gdk("1.21.120.04", "old")], uwp]);
        assert_eq!(all.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["1.26.65.1", "1.21.120.4", "1.21.114.1", "1.20.81.1"]);
        assert_eq!(all[1].source, Source::Gdk { urls: vec!["new".into()] }, "the first list wins");
        let info = VersionInfo::from(&all[2]);
        assert_eq!(serde_json::to_value(&info).unwrap(), serde_json::json!({"version": "1.21.114.1", "format": "uwp", "size": 5}));
    }

    #[test]
    fn builds_the_update_service_request() {
        let r = download_request("3738c248-0603-4560-abf4-fdd66e7cd852", "2026-11-20T14:00:00.000Z", "2026-11-20T14:05:00.000Z").unwrap();
        assert!(r.contains("<UpdateID>3738c248-0603-4560-abf4-fdd66e7cd852</UpdateID><RevisionNumber>1</RevisionNumber>"));
        assert!(r.contains("<Created>2026-11-20T14:00:00.000Z</Created><Expires>2026-11-20T14:05:00.000Z</Expires>"));
        assert!(r.contains("GetExtendedUpdateInfo2</a:Action>"));
        assert!(r.contains(UPDATE_SERVICE_URL));
        // No account's ticket goes with it.
        assert!(!r.contains("Name=\"MSA\""));
        assert!(r.starts_with("<s:Envelope") && r.ends_with("</s:Envelope>"));
    }

    #[test]
    fn reads_the_update_services_answer() {
        // The shape of a real answer (2026-11-20, 1.21.114.1): two files, each with its SHA-1 in base64.
        let answer = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"><s:Body><GetExtendedUpdateInfo2Response xmlns="http://www.microsoft.com/SoftwareDistribution/Server/ClientWebService"><GetExtendedUpdateInfo2Result><FileLocations><FileLocation><FileDigest>oUHLzen+ADSe3ZiBD69i/mtV17o=</FileDigest><Url>http://dl.delivery.mp.microsoft.com/filestreamingservice/files/a0585ab5</Url></FileLocation><FileLocation><FileDigest>DjOXCoVbKuPaPIywtF613BpbpAI=</FileDigest><Url>http://tlu.dl.delivery.mp.microsoft.com/filestreamingservice/files/69fa4c18?P1=1&amp;P2=404</Url></FileLocation><FileLocation><FileDigest>DjOXCoVbKuPaPIywtF613BpbpAI=</FileDigest><Url>http://example.com/tlu.dl.delivery.mp.microsoft.com/x</Url></FileLocation><FileLocation><Url>http://tlu.dl.delivery.mp.microsoft.com/no-digest</Url></FileLocation><FileLocation><FileDigest>c2hvcnQ=</FileDigest><Url>http://tlu.dl.delivery.mp.microsoft.com/short-digest</Url></FileLocation></FileLocations></GetExtendedUpdateInfo2Result></GetExtendedUpdateInfo2Response></s:Body></s:Envelope>"#;
        let files = parse_downloads(answer);
        assert_eq!(files.len(), 3, "a file without a digest, or with one that isn't 20 bytes, is left out");
        assert_eq!(files[0], Download { url: "http://dl.delivery.mp.microsoft.com/filestreamingservice/files/a0585ab5".into(), sha1: "a141cbcde9fe00349edd98810faf62fe6b55d7ba".into() });
        assert_eq!(
            pick_download(answer),
            Some(Download { url: "http://tlu.dl.delivery.mp.microsoft.com/filestreamingservice/files/69fa4c18?P1=1&P2=404".into(), sha1: "0e33970a855b2ae3da3c8cb0b45eb5dc1a5ba402".into() }),
            "the package: on Microsoft's tlu.dl host, with its digest"
        );
        // An answer without files (a version Microsoft no longer serves, or one that needs an account).
        let empty = r#"<s:Envelope><s:Body><GetExtendedUpdateInfo2Response><GetExtendedUpdateInfo2Result /></GetExtendedUpdateInfo2Response></s:Body></s:Envelope>"#;
        assert!(parse_downloads(empty).is_empty());
        assert_eq!(pick_download(empty), None);
        assert_eq!(pick_download("<FileLocation><FileDigest>DjOXCoVbKuPaPIywtF613BpbpAI=</FileDigest><Url>http://example.com/a.appx</Url></FileLocation>"), None);
        assert_eq!(pick_download("<FileLocation><Url>broken"), None);
    }

    #[test]
    fn the_root_inside_the_launcher_is_microsofts() {
        use sha1::{Digest, Sha1};
        assert_eq!(hex::encode(Sha1::digest(MICROSOFT_ROOT_2011)), "8f43288ad272f3103b6fb1428485ea3014c0bcfe");
        assert!(update_service_client().is_ok());
    }

    #[test]
    fn only_microsofts_delivery_hosts_pass() {
        assert!(allowed_uwp_url("http://tlu.dl.delivery.mp.microsoft.com/filestreamingservice/files/x?P1=1"));
        assert!(allowed_uwp_url("https://dl.delivery.mp.microsoft.com/x"));
        for bad in [
            "http://delivery.mp.microsoft.com.example.com/x",
            "http://example.com/?h=.delivery.mp.microsoft.com",
            "http://user@tlu.dl.delivery.mp.microsoft.com/x",
            "http://tlu.dl.delivery.mp.microsoft.com:81/x",
            "ftp://tlu.dl.delivery.mp.microsoft.com/x",
            "http://tlu.dl.delivery.mp.microsoft.com/x y",
            "tlu.dl.delivery.mp.microsoft.com/x",
            "",
        ] {
            assert!(!allowed_uwp_url(bad), "{bad}");
        }
    }

    /// Microsoft's real service: `cargo test --lib real_update_service -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "asks Microsoft's update service"]
    async fn real_update_service() {
        let v = parse_uwp_table(UWP_TABLE);
        let last = v.iter().find(|v| v.name == "1.21.114.1").unwrap();
        let Source::Uwp { update_id } = &last.source else { panic!("a UWP source") };
        // The public web's roots don't cover the service (the reason for `update_service_client`).
        assert!(reqwest::Client::new().get(UPDATE_SERVICE_URL).send().await.is_err());
        let client = update_service_client().unwrap();
        let file = uwp_download_with(&client, update_id, &last.name).await.unwrap();
        // The client is for that service only: another site isn't trusted by it.
        assert!(client.get("https://www.minecraft.net/").send().await.is_err());
        println!("{} sha1 {}", file.url, file.sha1);
        assert!(allowed_uwp_url(&file.url));
        assert_eq!(file.sha1.len(), 40);
    }
}

//! Switching Minecraft for Windows to another version.
//!
//! `plan` works out, without changing anything, what a switch to a version needs and what stands in its way, so the
//! Bedrock page can say so before the player agrees. `run` then does it in two tasks on the edition's card:
//!
//! 1. Preparing, which can be cancelled and changes nothing on the PC but ResinCraft's own folders: the checks
//!    again, a backup of every world that is verified (and stops the switch if it fails), the download of
//!    Microsoft's package for the version, and for a UWP version unpacking it.
//! 2. Replacing, which can't be stopped halfway: Windows removes the installed game (when it has to) and installs
//!    the other one. If Windows refuses it after the removal, the Microsoft Store installs its own version again.
//!
//! The two package formats differ (see `bedrock_versions`). A GDK version is Microsoft's signed `.msixvc`, handed
//! to Windows as it is: Windows and Gaming Services check the signature and the player's license, as for the
//! Store's own install. A UWP version is Microsoft's `.appx`, checked here against the SHA-1 Microsoft's update
//! service states for it and against Minecraft's package identity, unpacked into ResinCraft's folder and registered
//! with Windows, which takes Developer Mode: that setting is the player's to turn on (ResinCraft only reads it and
//! opens its page). The game's own license check is untouched either way: without a license from the Store it
//! doesn't install or runs as the trial.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

use super::bedrock::{Edition, InstalledGame};
use super::bedrock_backup::BackupInfo;
use super::bedrock_store::{Mode, Outcome};
use super::bedrock_versions::{self as versions, Format, Source, Version};
use crate::core::events::Task;
use crate::core::net::{DownloadItem, Hash, HashAlgo};
use crate::{bail_tr, err_tr, tr};

/// The first release in the GDK format; everything before it is UWP.
const FIRST_GDK: &str = "1.21.120";

/// The format a release of this game version comes in.
pub fn format_of(game_version: &str) -> Format {
    if versions::compare(game_version, FIRST_GDK) == Ordering::Less {
        Format::Uwp
    } else {
        Format::Gdk
    }
}

/// Something that has to be put right before a switch can start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Blocker {
    /// Minecraft for Windows isn't installed: there is nothing to switch, and no license from the Store on this PC.
    NotInstalled,
    /// That version is the one installed.
    SameVersion,
    /// Minecraft is open.
    Running,
    /// A UWP version needs Windows' Developer Mode, and it is off.
    DeveloperMode,
    /// A GDK version needs Gaming Services, and it isn't installed.
    GamingServices,
    /// Not enough free disk space for the backup, the download and the game.
    DiskSpace,
}

/// What is true on this PC right now.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// The installed game version, if Minecraft for Windows is installed.
    pub installed: Option<String>,
    pub developer_mode: bool,
    pub gaming_services: bool,
    pub running: bool,
    pub worlds: usize,
    pub worlds_bytes: u64,
    /// Free bytes on the drive with ResinCraft's data (None: unknown).
    pub free_bytes: Option<u64>,
}

/// What a switch to a version would do and need.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub version: String,
    pub format: Format,
    /// The download's size in bytes, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// The installed version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_format: Option<Format>,
    /// The version is older than the installed one.
    pub older: bool,
    /// Worlds that will be backed up first.
    pub worlds: usize,
    pub worlds_bytes: u64,
    /// Whether the version reads the worlds folder the installed one uses (both GDK). Otherwise it starts with its
    /// own worlds, and the current ones stay where they are.
    pub shares_worlds: bool,
    /// Free disk space the switch should have (an estimate), and what there is.
    pub needed_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_bytes: Option<u64>,
    pub blockers: Vec<Blocker>,
}

/// Room for the download when its size isn't known: a little more than the largest package so far.
const UNKNOWN_SIZE: u64 = 3 * 1024 * 1024 * 1024;
const SPACE_MARGIN: u64 = 512 * 1024 * 1024;

pub fn plan(target: &Version, facts: &Facts) -> Plan {
    let from_format = facts.installed.as_deref().map(format_of);
    // The backup, the package, and the game unpacked from it.
    let needed_bytes = facts.worlds_bytes + target.size.unwrap_or(UNKNOWN_SIZE) * 2 + SPACE_MARGIN;
    let mut blockers = Vec::new();
    match facts.installed.as_deref() {
        None => blockers.push(Blocker::NotInstalled),
        Some(v) if versions::same_version(v, &target.name) => blockers.push(Blocker::SameVersion),
        Some(_) => {}
    }
    if facts.running {
        blockers.push(Blocker::Running);
    }
    match target.format {
        Format::Uwp if !facts.developer_mode => blockers.push(Blocker::DeveloperMode),
        Format::Gdk if !facts.gaming_services => blockers.push(Blocker::GamingServices),
        _ => {}
    }
    if facts.free_bytes.is_some_and(|free| free < needed_bytes) {
        blockers.push(Blocker::DiskSpace);
    }
    Plan {
        version: target.name.clone(),
        format: target.format,
        size: target.size,
        older: facts.installed.as_deref().is_some_and(|v| versions::compare(&target.name, v) == Ordering::Less),
        from: facts.installed.clone(),
        from_format,
        worlds: facts.worlds,
        worlds_bytes: facts.worlds_bytes,
        shares_worlds: target.format == Format::Gdk && from_format == Some(Format::Gdk),
        needed_bytes,
        free_bytes: facts.free_bytes,
        blockers,
    }
}

/// Minecraft for Windows' program.
const GAME_PROCESS: &str = "minecraft.windows";

/// Whether Minecraft (Bedrock) is open.
pub fn game_running() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        name.strip_suffix(".exe").unwrap_or(&name) == GAME_PROCESS
    })
}

/// A GDK package's size, asked of Microsoft's CDN (the list doesn't carry sizes). None when it can't be asked.
async fn gdk_size(urls: &[String]) -> Option<u64> {
    for url in urls {
        let answer = crate::core::ctx::http().head(url).timeout(std::time::Duration::from_secs(15)).send().await;
        if let Ok(res) = answer {
            if res.status().is_success() {
                // The length header, not `content_length()`: a HEAD answer has no body to measure.
                if let Some(size) = res.headers().get(reqwest::header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()) {
                    return Some(size);
                }
            }
        }
    }
    None
}

/// The plan for switching Minecraft for Windows to `version` on this PC. Changes nothing.
pub async fn plan_for(version: &str) -> Result<Plan> {
    Ok(target_and_plan(version).await?.1)
}

/// The version as the lists have it (with its download size when Microsoft says it), and the plan for it.
async fn target_and_plan(version: &str) -> Result<(Version, Plan)> {
    let mut target = versions::find(version).await?;
    if target.size.is_none() {
        if let Source::Gdk { urls } = &target.source {
            target.size = gdk_size(urls).await;
        }
    }
    let status = super::bedrock::status().await?;
    let release = status.editions.iter().find(|e| e.edition == Edition::Release);
    let roots = super::bedrock::world_roots(Edition::Release);
    let (worlds_bytes, running) = tokio::task::spawn_blocking(move || (roots.iter().map(|r| crate::core::fsutil::dir_size(r)).sum(), game_running())).await?;
    let facts = Facts {
        installed: release.and_then(|e| e.version.clone()),
        developer_mode: status.developer_mode,
        gaming_services: status.gaming_services,
        running,
        worlds: release.map_or(0, |e| e.worlds.len()),
        worlds_bytes,
        free_bytes: crate::core::storage::free_bytes(&crate::core::paths::root()),
    };
    let plan = plan(&target, &facts);
    Ok((target, plan))
}

// ---------- The switch itself ----------

/// How a switch ended.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Switched {
    /// The version installed now.
    pub version: String,
    /// The world backup made first (none when there were no worlds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<BackupInfo>,
}

/// Minecraft for Windows' app package and Microsoft as its publisher: what an unpacked UWP package must say it is
/// (together they give the package family the player's license is for).
const PACKAGE_NAME: &str = "Microsoft.MinecraftUWP";
const PACKAGE_PUBLISHER: &str = "CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US";
/// The package's signature covers the packed file; Windows doesn't take it with files registered from a folder.
const SIGNATURE_FILE: &str = "AppxSignature.p7x";
const MANIFEST_FILE: &str = "AppxManifest.xml";
/// Written into a version's folder once it is unpacked whole and checked.
const UNPACKED_MARK: &str = ".resincraft-unpacked";
/// How long Windows gets to remove or install a package before the switch gives up on it.
const DEPLOY_TIMEOUT: Duration = Duration::from_secs(45 * 60);
const MIB: u64 = 1024 * 1024;

/// The first thing in the way of a switch, as an error the page can show.
fn blocker_error(blocker: Blocker, plan: &Plan) -> anyhow::Error {
    match blocker {
        Blocker::NotInstalled => err_tr!("bedrock.switch.notInstalled", "Install Minecraft for Windows first, then switch its version."),
        Blocker::SameVersion => err_tr!("bedrock.switch.sameVersion", "Minecraft {version} is the version that is installed.", version = plan.version),
        Blocker::Running => err_tr!("bedrock.switch.closeFirst", "Close Minecraft first, then switch its version."),
        Blocker::DeveloperMode => err_tr!(
            "bedrock.switch.developerMode",
            "Minecraft versions before 1.21.120 need Windows' Developer Mode. Turn it on in Windows Settings (System > For developers), then try again."
        ),
        Blocker::GamingServices => err_tr!(
            "bedrock.switch.gamingServices",
            "Minecraft {version} needs Gaming Services, which isn't installed. Get it from the Microsoft Store, then try again.",
            version = plan.version
        ),
        Blocker::DiskSpace => err_tr!(
            "bedrock.switch.diskSpace",
            "There isn't enough free disk space to switch versions: about {needed} MB is needed.",
            needed = (plan.needed_bytes / MIB).to_string()
        ),
    }
}

/// Where downloaded packages wait until Windows has installed them.
fn packages_dir() -> PathBuf {
    crate::core::paths::cache().join("bedrock-packages")
}

/// Downloads with the launcher's downloader as it is, showing how far the `.part` file it writes has got.
async fn download(item: DownloadItem, size: Option<u64>, version: &str, task: &Task) -> Result<()> {
    let part = PathBuf::from(format!("{}.part", item.dest.display()));
    let job = crate::core::net::download_file(&item);
    tokio::pin!(job);
    loop {
        tokio::select! {
            r = &mut job => return r,
            _ = tokio::time::sleep(Duration::from_millis(300)) => {
                let have = tokio::fs::metadata(&part).await.map_or(0, |m| m.len());
                match size.filter(|s| *s >= MIB) {
                    Some(total) => task.update(
                        tr!(
                            "bedrock.switch.downloading",
                            "Downloading Minecraft {version} from Microsoft ({done} of {total} MB)",
                            version = version,
                            done = (have / MIB).to_string(),
                            total = (total / MIB).to_string()
                        ),
                        Some((have as f64 / total as f64).clamp(0.0, 1.0)),
                    ),
                    None => task.update(tr!("bedrock.switch.downloadingStart", "Downloading Minecraft {version} from Microsoft", version = version), None),
                }
            }
        }
    }
}

fn download_error(e: anyhow::Error, version: &str) -> anyhow::Error {
    err_tr!("bedrock.switch.downloadFailed", "Couldn't download Minecraft {version} from Microsoft: {error}", version = version, error = e.to_string())
}

/// Gets the version's package from Microsoft into ResinCraft's cache and returns the file. A UWP package is
/// checked against the SHA-1 Microsoft's update service states for it (the downloader refuses anything else); a
/// GDK package against the size Microsoft's CDN states, and Windows checks its signature when it installs it.
async fn fetch(target: &Version, size: Option<u64>, task: &Task) -> Result<PathBuf> {
    let dir = packages_dir();
    match &target.source {
        Source::Uwp { update_id } => {
            let file = versions::uwp_download(update_id, &target.name).await?;
            let dest = dir.join(format!("{PACKAGE_NAME}_{}.appx", target.name));
            // Left by a switch that stopped later: used again only if it is still exactly that file.
            if dest.is_file() && crate::core::net::hash_file(&dest, HashAlgo::Sha1).await.is_ok_and(|h| h.eq_ignore_ascii_case(&file.sha1)) {
                return Ok(dest);
            }
            let item = DownloadItem::new(file.url, &dest).hash(Some(Hash::sha1(file.sha1))).size(size);
            download(item, size, &target.name, task).await.map_err(|e| download_error(e, &target.name))?;
            Ok(dest)
        }
        Source::Gdk { urls } => {
            let unknown = || err_tr!("bedrock.switch.unknownVersion", "ResinCraft doesn't know where Microsoft keeps Minecraft {version}.", version = target.name);
            let dest = dir.join(versions::gdk_file_name(&target.name).ok_or_else(unknown)?);
            let length = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
            let whole = |p: &Path| p.is_file() && (size.is_none() || length(p) == size);
            // Left by a switch that stopped later: used again only when Microsoft's CDN says it is that long.
            if !(size.is_some() && whole(&dest)) {
                let mut failure = Some(unknown());
                for url in urls.iter().filter(|u| versions::allowed_gdk_url(u, &target.name)) {
                    match download(DownloadItem::new(url, &dest).size(size), size, &target.name, task).await {
                        Ok(()) => {
                            failure = None;
                            break;
                        }
                        Err(e) => failure = Some(download_error(e, &target.name)),
                    }
                }
                if let Some(e) = failure {
                    return Err(e);
                }
            }
            if !whole(&dest) {
                let _ = tokio::fs::remove_file(&dest).await;
                bail_tr!("bedrock.switch.badDownload", "The download of Minecraft {version} isn't complete. Try again.", version = target.name);
            }
            Ok(dest)
        }
    }
}

/// `&quot;` and the other XML entities in an attribute value back to their characters.
fn xml_value(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The package identity an `AppxManifest.xml` states: the Name and Publisher of its `Identity` element.
pub fn manifest_identity(xml: &str) -> Option<(String, String)> {
    let element = regex::Regex::new(r"<Identity\b[^>]*>").ok()?.find(xml)?.as_str();
    let attribute = |name: &str| -> Option<String> {
        let found = regex::Regex::new(&format!(r#"\s{name}\s*=\s*(?:"([^"]*)"|'([^']*)')"#)).ok()?.captures(element)?;
        found.get(1).or_else(|| found.get(2)).map(|m| xml_value(m.as_str()))
    };
    Some((attribute("Name")?, attribute("Publisher")?))
}

/// The framework packages an `AppxManifest.xml` needs (`PackageDependency`): name and lowest version.
pub fn manifest_dependencies(xml: &str) -> Vec<(String, String)> {
    let Ok(element) = regex::Regex::new(r"<PackageDependency\b[^>]*>") else { return Vec::new() };
    let attribute = |el: &str, name: &str| -> Option<String> {
        let found = regex::Regex::new(&format!(r#"\s{name}\s*=\s*(?:"([^"]*)"|'([^']*)')"#)).ok()?.captures(el)?;
        found.get(1).or_else(|| found.get(2)).map(|m| xml_value(m.as_str()))
    };
    element
        .find_iter(xml)
        .filter_map(|m| Some((attribute(m.as_str(), "Name")?, attribute(m.as_str(), "MinVersion").unwrap_or_else(|| "0.0.0.0".into()))))
        // The name goes into a PowerShell command: nothing but a package name passes.
        .filter(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .collect()
}

/// An installed package as the dependency check reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    /// "X64", "Neutral", "X86"…
    pub architecture: String,
}

/// The first framework `needs` lists that no installed package satisfies (same name, x64 or neutral, at least the
/// version asked for).
pub fn missing_dependency(needs: &[(String, String)], installed: &[InstalledPackage]) -> Option<(String, String)> {
    needs
        .iter()
        .find(|(name, min)| {
            !installed.iter().any(|p| {
                p.name.eq_ignore_ascii_case(name)
                    && (p.architecture.eq_ignore_ascii_case("X64") || p.architecture.eq_ignore_ascii_case("Neutral"))
                    && versions::compare(&p.version, min) != Ordering::Less
            })
        })
        .cloned()
}

/// The installed packages with these names, from Get-AppxPackage.
async fn installed_packages(names: &[String]) -> Vec<InstalledPackage> {
    if names.is_empty() {
        return Vec::new();
    }
    let list = names.iter().map(|n| format!("@(Get-AppxPackage -Name {})", ps_quote(n))).collect::<Vec<_>>().join(" + ");
    let script = format!("{list} | Select-Object Name, @{{n='Version';e={{[string]$_.Version}}}}, @{{n='Architecture';e={{[string]$_.Architecture}}}} | ConvertTo-Json -Compress");
    let Ok(out) = crate::game::java::hidden_command("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output().await else { return Vec::new() };
    parse_installed_packages(&String::from_utf8_lossy(&out.stdout))
}

pub fn parse_installed_packages(text: &str) -> Vec<InstalledPackage> {
    let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap_or(serde_json::Value::Null);
    let list = match v {
        serde_json::Value::Array(a) => a,
        o @ serde_json::Value::Object(_) => vec![o],
        _ => Vec::new(),
    };
    list.iter()
        .filter_map(|p| {
            Some(InstalledPackage {
                name: p.get("Name")?.as_str()?.to_string(),
                version: p.get("Version")?.as_str()?.to_string(),
                architecture: p.get("Architecture").and_then(|a| a.as_str()).unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// Before anything is removed: Windows registers a UWP version only when the Microsoft frameworks it needs are
/// installed, and ResinCraft can't get those itself. Asking now leaves the installed game alone when one is missing.
async fn check_dependencies(dir: &Path, version: &str) -> Result<()> {
    let manifest = tokio::fs::read_to_string(dir.join(MANIFEST_FILE)).await.unwrap_or_default();
    let needs = manifest_dependencies(&manifest);
    let names: Vec<String> = needs.iter().map(|(n, _)| n.clone()).collect();
    if let Some((framework, min)) = missing_dependency(&needs, &installed_packages(&names).await) {
        bail_tr!(
            "bedrock.switch.missingFramework",
            "Minecraft {version} needs {framework} {min} from Microsoft, which isn't installed on this PC, so nothing was changed. Choose another version.",
            version = version,
            framework = framework,
            min = min
        );
    }
    Ok(())
}

/// Unpacks a UWP package into `dir` so Windows can register it from there: every file but the package's
/// signature, written next to `dir` first and given its name only once the unpacked package says it is Minecraft
/// for Windows from Microsoft. A folder that was unpacked whole before is used as it is.
pub fn unpack(appx: &Path, dir: &Path) -> Result<()> {
    if dir.join(UNPACKED_MARK).is_file() && dir.join(MANIFEST_FILE).is_file() {
        return Ok(());
    }
    let part = PathBuf::from(format!("{}.unpacking", dir.display()));
    // Left by an unpack that stopped: ResinCraft's own folders, holding nothing but files from the package.
    let _ = std::fs::remove_dir_all(&part);
    let _ = std::fs::remove_dir_all(dir);
    let result = (|| -> Result<()> {
        let bad = |e: String| err_tr!("bedrock.switch.badPackage", "The package Microsoft sent couldn't be unpacked: {error}", error = e);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(appx)?).map_err(|e| bad(e.to_string()))?;
        std::fs::create_dir_all(&part)?;
        crate::content::index::extract_where(&mut zip, &part, |name| (!name.eq_ignore_ascii_case(SIGNATURE_FILE)).then(|| name.to_string())).map_err(|e| bad(e.to_string()))?;
        let manifest = std::fs::read_to_string(part.join(MANIFEST_FILE)).unwrap_or_default();
        if manifest_identity(&manifest) != Some((PACKAGE_NAME.to_string(), PACKAGE_PUBLISHER.to_string())) {
            bail_tr!("bedrock.switch.wrongPackage", "The package Microsoft sent isn't Minecraft for Windows, so nothing was installed.");
        }
        std::fs::write(part.join(UNPACKED_MARK), b"")?;
        std::fs::rename(&part, dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&part);
    }
    result
}

/// What Windows said when it refused to remove, install or register a package.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    /// Windows' error code ("0x80073CF9").
    pub code: String,
    pub message: String,
}

/// Reads what the deployment scripts print about a failure: "0x80131500 Deployment failed with HRESULT:
/// 0x80073CF9, Install failed. … NOTE: For additional information…". Windows' own code is the one in the text;
/// the leading one is only PowerShell's.
pub fn refusal(text: &str) -> Refusal {
    let text = text.trim();
    let is_code = |s: &str| s.len() == 10 && (s.starts_with("0x") || s.starts_with("0X")) && s[2..].chars().all(|c| c.is_ascii_hexdigit());
    let (leading, rest) = text.split_once(' ').unwrap_or((text, ""));
    let (leading, rest) = if is_code(leading) { (Some(leading), rest) } else { (None, text) };
    let stated = rest.find("HRESULT: ").map(|i| &rest[i + 9..]).and_then(|s| s.get(..10)).filter(|s| is_code(s));
    let code = stated.or(leading).unwrap_or("0x80004005");
    let message = rest.split(" NOTE:").next().unwrap_or(rest).trim();
    Refusal { code: format!("0x{}", code[2..].to_uppercase()), message: message.to_string() }
}

/// How a deployment script ended: its last line is `ok`, `missing` (nothing to remove) or `error <code> <text>`.
pub fn deploy_result(out: &str) -> std::result::Result<(), Refusal> {
    let last = out.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or_default();
    match last {
        "ok" | "missing" => Ok(()),
        _ => Err(refusal(last.strip_prefix("error ").or_else(|| last.strip_prefix("refused ")).unwrap_or(last))),
    }
}

/// A path (or any text) as a PowerShell single-quoted string.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Removes Minecraft for Windows for this Windows user. A copy registered from a folder keeps its app data
/// (its worlds are in there); Windows allows that only for such copies.
const REMOVE_SCRIPT: &str = "$ErrorActionPreference = 'Stop'
try {
  $p = @(Get-AppxPackage -Name 'Microsoft.MinecraftUWP')
  if ($p.Count -eq 0) { 'missing'; exit }
  foreach ($x in $p) {
    if ($x.IsDevelopmentMode) { Remove-AppxPackage -Package $x.PackageFullName -PreserveApplicationData } else { Remove-AppxPackage -Package $x.PackageFullName }
  }
  'ok'
} catch {
  $e = $_.Exception
  while ($null -ne $e.InnerException) { $e = $e.InnerException }
  'error ' + ('0x{0:X8}' -f $e.HResult) + ' ' + ($e.Message -replace '\\s+', ' ')
}";

/// Has Windows install a signed package ($Path) as it is. Gaming Services can refuse a GDK package on a drive it
/// doesn't keep games on, so the drive the removed game was on ($Drive, may be empty) is tried first, then
/// Windows' default, then the other drives. A refusal because a newer version is still installed ends it at once.
const ADD_SCRIPT: &str = "$ErrorActionPreference = 'Stop'
function Say($s) { [Console]::Out.WriteLine($s); [Console]::Out.Flush() }
function Why($e) { while ($null -ne $e.InnerException) { $e = $e.InnerException }; ('0x{0:X8}' -f $e.HResult) + ' ' + ($e.Message -replace '\\s+', ' ') }
$tries = @()
try {
  $all = @(Get-AppxVolume | Where-Object { -not $_.IsOffline })
  if ($Drive) { $tries += @($all | Where-Object { $_.PackageStorePath -like ($Drive + '*') }) }
  $tries += $null
  $tries += @($all | Where-Object { (-not $Drive) -or ($_.PackageStorePath -notlike ($Drive + '*')) })
} catch { $tries = @($null) }
$last = '0x80004005 '
foreach ($v in $tries) {
  try {
    if ($null -eq $v) { Add-AppxPackage -Path $Path } else { Add-AppxPackage -Path $Path -Volume $v }
    Say 'ok'
    exit
  } catch {
    $last = Why $_.Exception
    Say ('refused ' + $last)
    if ($last -match '0x80073D06') { break }
  }
}
Say ('error ' + $last)";

/// Registers an unpacked package with Windows from its manifest ($Path): Windows' development mode, which is
/// what Developer Mode allows.
const REGISTER_SCRIPT: &str = "$ErrorActionPreference = 'Stop'
try {
  Add-AppxPackage -Register $Path
  'ok'
} catch {
  $e = $_.Exception
  while ($null -ne $e.InnerException) { $e = $e.InnerException }
  'error ' + ('0x{0:X8}' -f $e.HResult) + ' ' + ($e.Message -replace '\\s+', ' ')
}";

/// Runs a deployment script hidden in Windows PowerShell and reads how it ended.
async fn deploy(script: String) -> std::result::Result<(), Refusal> {
    // A double quote wouldn't pass through -Command as it is (see bedrock_store.rs); paths can't hold one.
    if script.contains('"') {
        return Err(Refusal { code: "0x80070057".into(), message: "The path has a character Windows PowerShell can't take.".into() });
    }
    let mut command = crate::game::java::hidden_command("powershell");
    command.args(["-NoProfile", "-NonInteractive", "-Command", &script]).stdin(std::process::Stdio::null());
    match tokio::time::timeout(DEPLOY_TIMEOUT, command.output()).await {
        Ok(Ok(out)) => deploy_result(&String::from_utf8_lossy(&out.stdout)),
        Ok(Err(e)) => Err(Refusal { code: "0x80004005".into(), message: e.to_string() }),
        Err(_) => Err(Refusal { code: "0x800705B4".into(), message: "Windows didn't finish in time.".into() }),
    }
}

/// Whether Windows has to remove the installed game before it takes `target`: it won't put an older package over
/// a newer one, a copy registered from a folder can't be updated by a package, and registering from a folder
/// needs the family's place to be free. A newer GDK version goes over an older installed one as an update.
pub(crate) fn needs_removal(installed: &InstalledGame, target: &Version) -> bool {
    installed.development || target.format == Format::Uwp || versions::compare(&target.name, &installed.version) == Ordering::Less
}

/// "C:\" from "C:\XboxGames\Minecraft for Windows\Content": the drive a package is on.
fn drive_of(location: &str) -> Option<String> {
    let b = location.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\').then(|| location[..3].to_string())
}

/// What is ready to go in place of the installed game.
enum Ready {
    /// A signed GDK package, for Windows to install.
    Package(PathBuf),
    /// An unpacked UWP version's folder, for Windows to register.
    Folder(PathBuf),
}

struct Prepared {
    target: Version,
    backup: Option<BackupInfo>,
    ready: Ready,
    /// The downloaded package, removed once Windows has taken the version.
    package: PathBuf,
}

fn owner() -> Option<String> {
    Some("bedrock:release".to_string())
}

/// Step 1 of a switch: everything that can be cancelled. Changes nothing but ResinCraft's own folders.
async fn prepare(version: String, task: std::sync::Arc<Task>) -> Result<Prepared> {
    task.update(tr!("bedrock.switch.checking", "Checking this PC"), None);
    let (target, plan) = target_and_plan(&version).await?;
    if let Some(blocker) = plan.blockers.first() {
        return Err(blocker_error(*blocker, &plan));
    }
    // Worlds first: the copy is checked file by file, and a copy that isn't exact ends the switch here.
    let backup = super::bedrock_backup::run(Edition::Release, plan.from.clone(), &task).await?;
    let package = fetch(&target, plan.size, &task).await?;
    let ready = match target.format {
        Format::Gdk => Ready::Package(package.clone()),
        Format::Uwp => {
            task.update(tr!("bedrock.switch.unpacking", "Unpacking Minecraft {version}", version = target.name), None);
            let dir = super::bedrock::versions_dir().join(&target.name);
            let (from, to) = (package.clone(), dir.clone());
            tokio::task::spawn_blocking(move || unpack(&from, &to)).await??;
            if let Err(e) = check_dependencies(&dir, &target.name).await {
                // Windows won't take this version on this PC: its package and unpacked files are of no use.
                let _ = tokio::fs::remove_file(&package).await;
                let _ = tokio::fs::remove_dir_all(&dir).await;
                return Err(e);
            }
            Ready::Folder(dir)
        }
    };
    Ok(Prepared { target, backup, ready, package })
}

/// Why a switch failed after Windows was asked to install the version, and what state that left.
fn refused_error(version: &str, refusal: &Refusal, removed: bool, store: Option<&anyhow::Error>) -> anyhow::Error {
    let (code, error) = (refusal.code.clone(), if refusal.message.is_empty() { "-".to_string() } else { refusal.message.clone() });
    match (removed, store) {
        (false, _) => err_tr!(
            "bedrock.switch.refusedKept",
            "Windows refused Minecraft {version} ({code}): {error} The installed version was left as it is.",
            version = version,
            code = code,
            error = error
        ),
        (true, None) => err_tr!(
            "bedrock.switch.refused",
            "Windows refused Minecraft {version} ({code}): {error} The Store version is installed again.",
            version = version,
            code = code,
            error = error
        ),
        (true, Some(store)) => err_tr!(
            "bedrock.switch.refusedStoreFailed",
            "Windows refused Minecraft {version} ({code}): {error} Putting the Store version back failed too ({store}). Install Minecraft for Windows again from this page.",
            version = version,
            code = code,
            error = error,
            store = store.to_string()
        ),
    }
}

/// Removes the installed Minecraft for Windows for this Windows user.
async fn remove_installed() -> Result<()> {
    match deploy(REMOVE_SCRIPT.to_string()).await {
        Ok(()) => Ok(()),
        // 0x80073D02: the app is open.
        Err(r) if r.code == "0x80073D02" => Err(err_tr!("bedrock.switch.closeFirst", "Close Minecraft first, then switch its version.")),
        Err(r) => Err(err_tr!("bedrock.switch.removeFailed", "Windows couldn't remove the installed Minecraft ({code}): {error}", code = r.code, error = r.message)),
    }
}

/// Removes the unpacked UWP versions that aren't the one in use (`keep`): ResinCraft's own folders, holding only
/// files from Microsoft's packages, which can be downloaded again.
fn prune_versions(dir: &Path, keep: Option<&str>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let (path, name) = (entry.path(), entry.file_name().to_string_lossy().into_owned());
        let ours = path.join(MANIFEST_FILE).is_file() || name.ends_with(".unpacking");
        if path.is_dir() && ours && keep != Some(name.as_str()) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// Whether the installed copy is the version a switch was for.
fn is_target(installed: &InstalledGame, target: &Version) -> bool {
    versions::same_version(&installed.version, &target.name)
}

/// Step 2 of a switch: Windows removes the installed game when it has to and takes the other version. Not
/// cancellable: stopped halfway, the player would be left without Minecraft.
async fn replace(prepared: Prepared, task: std::sync::Arc<Task>) -> Result<Switched> {
    let Prepared { target, backup, ready, package } = prepared;
    let installed = super::bedrock::installed_release().await;
    let removed = installed.as_ref().is_some_and(|i| needs_removal(i, &target));
    if removed {
        let from = installed.as_ref().map(|i| i.version.clone()).unwrap_or_default();
        task.update(tr!("bedrock.switch.removing", "Removing Minecraft {version}", version = from), None);
        // A failure here leaves the installed game as it was.
        remove_installed().await?;
    }
    task.update(tr!("bedrock.switch.installing", "Windows is installing Minecraft {version}", version = target.name), None);
    let outcome = match &ready {
        Ready::Package(path) => {
            let drive = installed.as_ref().and_then(|i| i.location.as_deref()).and_then(drive_of).unwrap_or_default();
            deploy(format!("$Path = {}; $Drive = {}\n{ADD_SCRIPT}", ps_quote(&path.to_string_lossy()), ps_quote(&drive))).await
        }
        Ready::Folder(dir) => deploy(format!("$Path = {}\n{REGISTER_SCRIPT}", ps_quote(&dir.join(MANIFEST_FILE).to_string_lossy()))).await,
    };
    // Windows saying yes isn't enough: the version has to be the one that is installed now.
    let outcome = match outcome {
        Ok(()) => match super::bedrock::installed_release().await {
            Some(now) if is_target(&now, &target) => Ok(()),
            now => Err(Refusal {
                code: "0x80004005".into(),
                message: format!("Windows reported no error, but Minecraft {} is installed.", now.map_or("no".to_string(), |n| n.version)),
            }),
        },
        refused => refused,
    };
    let versions_dir = super::bedrock::versions_dir();
    match outcome {
        Ok(()) => {
            let _ = crate::core::fsutil::write_json(&super::bedrock::switch_record_file(), &super::bedrock::SwitchRecord { version: target.name.clone() }).await;
            let _ = tokio::fs::remove_file(&package).await;
            let keep = (target.format == Format::Uwp).then(|| target.name.clone());
            let _ = tokio::task::spawn_blocking(move || prune_versions(&versions_dir, keep.as_deref())).await;
            Ok(Switched { version: target.name, backup })
        }
        Err(refusal) => {
            // A package Windows refused for anything but "a newer version is installed" may be damaged: the next
            // try downloads it again.
            if refusal.code != "0x80073D06" {
                let _ = tokio::fs::remove_file(&package).await;
            }
            if !removed {
                return Err(refused_error(&target.name, &refusal, false, None));
            }
            task.update(tr!("bedrock.switch.puttingBack", "Windows refused it. Putting the Store version back"), None);
            let store = super::bedrock_store::run_in(&task, Edition::Release, Mode::Install).await;
            let _ = tokio::fs::remove_file(super::bedrock::switch_record_file()).await;
            let _ = tokio::task::spawn_blocking(move || prune_versions(&versions_dir, None)).await;
            Err(refused_error(&target.name, &refusal, true, store.as_ref().err()))
        }
    }
}

/// Downloads Microsoft's package for a version into ResinCraft's cache and checks it; a UWP one is unpacked and its
/// identity checked too. Returns the package file, or the unpacked folder. Installs nothing and asks nothing of
/// Windows: the real check of these steps in `examples/check_bedrock.rs`.
pub async fn download_only(version: &str) -> Result<PathBuf> {
    let mut target = versions::find(version).await?;
    if target.size.is_none() {
        if let Source::Gdk { urls } = &target.source {
            target.size = gdk_size(urls).await;
        }
    }
    let title = tr!("bedrock.switch.downloadingStart", "Downloading Minecraft {version} from Microsoft", version = target.name);
    Task::run(title, None, move |task| async move {
        let package = fetch(&target, target.size, &task).await?;
        if target.format == Format::Gdk {
            return Ok(package);
        }
        task.update(tr!("bedrock.switch.unpacking", "Unpacking Minecraft {version}", version = target.name), None);
        let dir = super::bedrock::versions_dir().join(&target.name);
        let to = dir.clone();
        tokio::task::spawn_blocking(move || unpack(&package, &to)).await??;
        Ok(dir)
    })
    .await
}

/// Switches Minecraft for Windows to `version`: see the module's description. The backup made on the way is in
/// the result, and stays in ResinCraft's backups whatever happens after it.
pub async fn run(version: &str) -> Result<Switched> {
    if !cfg!(windows) {
        bail_tr!("bedrock.store.windowsOnly", "Bedrock Edition installs only on Windows.");
    }
    let name = Edition::Release.name();
    let title = || tr!("bedrock.switch.task", "Switching {name} to {version}", name = name, version = version);
    let wanted = version.to_string();
    let prepared = Task::run(title(), owner(), move |task| prepare(wanted, task)).await?;
    Task::run_to_end(title(), owner(), move |task| replace(prepared, task)).await
}

/// Puts the Microsoft Store's current version back after a switch: a verified backup of the worlds first, then a
/// copy registered from a folder is removed (keeping its app data) and the Store installs its version; a GDK
/// version is updated by the Store in place.
pub async fn back_to_store() -> Result<Switched> {
    if !cfg!(windows) {
        bail_tr!("bedrock.store.windowsOnly", "Bedrock Edition installs only on Windows.");
    }
    let name = Edition::Release.name();
    let title = || tr!("bedrock.switch.backTask", "Putting {name} back on the Store version", name = name);
    let backup = Task::run(title(), owner(), move |task| async move {
        task.update(tr!("bedrock.switch.checking", "Checking this PC"), None);
        if tokio::task::spawn_blocking(game_running).await? {
            bail_tr!("bedrock.switch.closeFirst", "Close Minecraft first, then switch its version.");
        }
        let version = super::bedrock::installed_release().await.map(|i| i.version);
        super::bedrock_backup::run(Edition::Release, version, &task).await
    })
    .await?;
    Task::run_to_end(title(), owner(), move |task| async move {
        let installed = super::bedrock::installed_release().await;
        let mode = match &installed {
            // Registered from a folder: the Store can't update that, so it goes first.
            Some(i) if i.development => {
                task.update(tr!("bedrock.switch.removing", "Removing Minecraft {version}", version = i.version), None);
                remove_installed().await?;
                Mode::Install
            }
            Some(_) => Mode::Update,
            None => Mode::Install,
        };
        let outcome = super::bedrock_store::run_in(&task, Edition::Release, mode).await?;
        let now = super::bedrock::installed_release().await;
        // The Store found nothing newer although ResinCraft had switched the game to an older version.
        if outcome == Outcome::UpToDate && installed.is_some() && now.as_ref().map(|n| &n.version) == installed.as_ref().map(|i| &i.version) {
            let record: Option<super::bedrock::SwitchRecord> = crate::core::fsutil::read_json(&super::bedrock::switch_record_file()).await;
            if record.is_some_and(|r| now.as_ref().is_some_and(|n| versions::same_version(&r.version, &n.version))) {
                bail_tr!("bedrock.switch.storeHasNothing", "The Microsoft Store found no newer version to install. Open the Store and update Minecraft there.");
            }
        }
        let _ = tokio::fs::remove_file(super::bedrock::switch_record_file()).await;
        let versions_dir = super::bedrock::versions_dir();
        let _ = tokio::task::spawn_blocking(move || prune_versions(&versions_dir, None)).await;
        Ok(Switched { version: now.map(|n| n.version).unwrap_or_default(), backup })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gdk(name: &str, size: Option<u64>) -> Version {
        Version { name: name.into(), format: Format::Gdk, size, source: Source::Gdk { urls: vec![] } }
    }

    fn uwp(name: &str, size: u64) -> Version {
        Version { name: name.into(), format: Format::Uwp, size: Some(size), source: Source::Uwp { update_id: String::new() } }
    }

    fn ready() -> Facts {
        Facts { installed: Some("1.26.65.1".into()), developer_mode: true, gaming_services: true, running: false, worlds: 2, worlds_bytes: 1_000, free_bytes: Some(u64::MAX) }
    }

    #[test]
    fn the_format_follows_the_version() {
        assert_eq!(format_of("1.21.114.1"), Format::Uwp);
        assert_eq!(format_of("0.14.0.1"), Format::Uwp);
        assert_eq!(format_of("1.21.120.4"), Format::Gdk);
        assert_eq!(format_of("1.26.65.1"), Format::Gdk);
    }

    #[test]
    fn a_ready_pc_has_nothing_in_the_way() {
        let p = plan(&gdk("1.21.130.4", Some(2_000)), &ready());
        assert!(p.blockers.is_empty());
        assert!(p.older);
        assert!(p.shares_worlds, "both versions read %APPDATA%\\Minecraft Bedrock");
        assert_eq!((p.from.as_deref(), p.from_format), (Some("1.26.65.1"), Some(Format::Gdk)));
        assert_eq!(p.needed_bytes, 1_000 + 4_000 + SPACE_MARGIN);
        let p = plan(&uwp("1.20.81.1", 900), &ready());
        assert!(p.blockers.is_empty());
        assert!(!p.shares_worlds, "a UWP version keeps its own worlds");
    }

    #[test]
    fn each_format_asks_for_its_own_thing() {
        let bare = Facts { developer_mode: false, gaming_services: false, ..ready() };
        assert_eq!(plan(&uwp("1.20.81.1", 900), &bare).blockers, [Blocker::DeveloperMode]);
        assert_eq!(plan(&gdk("1.21.130.4", Some(1)), &bare).blockers, [Blocker::GamingServices]);
        // Developer Mode doesn't matter for a GDK version, nor Gaming Services for a UWP one.
        assert!(plan(&gdk("1.21.130.4", Some(1)), &Facts { developer_mode: false, ..ready() }).blockers.is_empty());
        assert!(plan(&uwp("1.20.81.1", 900), &Facts { gaming_services: false, ..ready() }).blockers.is_empty());
    }

    #[test]
    fn says_everything_that_is_in_the_way() {
        let facts = Facts { installed: None, running: true, developer_mode: false, free_bytes: Some(10), ..ready() };
        assert_eq!(plan(&uwp("1.20.81.1", 900), &facts).blockers, [Blocker::NotInstalled, Blocker::Running, Blocker::DeveloperMode, Blocker::DiskSpace]);
        let same = plan(&gdk("1.26.65.01", Some(1)), &ready());
        assert_eq!(same.blockers, [Blocker::SameVersion]);
        assert!(!same.older);
        // A newer version than the installed one (after a switch to an older one) is not "older".
        let up = plan(&gdk("1.26.65.1", Some(1)), &Facts { installed: Some("1.21.130.4".into()), ..ready() });
        assert!(up.blockers.is_empty() && !up.older);
        // Unknown free space doesn't block; an unknown download size counts as a large one.
        let unknown = plan(&gdk("1.21.130.4", None), &Facts { free_bytes: None, ..ready() });
        assert!(unknown.blockers.is_empty());
        assert_eq!(unknown.needed_bytes, 1_000 + UNKNOWN_SIZE * 2 + SPACE_MARGIN);
    }

    #[test]
    fn the_page_gets_the_plan_in_its_own_words() {
        let p = plan(&uwp("1.20.81.1", 900), &Facts { developer_mode: false, free_bytes: Some(5), ..ready() });
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["format"], "uwp");
        assert_eq!(v["fromFormat"], "gdk");
        assert_eq!(v["blockers"], serde_json::json!(["developerMode", "diskSpace"]));
        assert_eq!(v["sharesWorlds"], false);
        assert_eq!(v["worldsBytes"], 1_000);
        assert_eq!(v["freeBytes"], 5);
    }

    fn key(e: &anyhow::Error) -> String {
        crate::core::text::text_of(e).map(|t| t.key.clone()).unwrap_or_default()
    }

    #[test]
    fn each_blocker_has_its_own_reason() {
        let p = plan(&uwp("1.20.81.1", 900), &ready());
        let all = [Blocker::NotInstalled, Blocker::SameVersion, Blocker::Running, Blocker::DeveloperMode, Blocker::GamingServices, Blocker::DiskSpace];
        let keys: Vec<String> = all.iter().map(|b| key(&blocker_error(*b, &p))).collect();
        assert_eq!(keys, [
            "bedrock.switch.notInstalled",
            "bedrock.switch.sameVersion",
            "bedrock.switch.closeFirst",
            "bedrock.switch.developerMode",
            "bedrock.switch.gamingServices",
            "bedrock.switch.diskSpace"
        ]);
        // Developer Mode is the player's to turn on: the message says where, and nothing else does it.
        assert!(blocker_error(Blocker::DeveloperMode, &p).to_string().contains("Turn it on in Windows Settings"));
    }

    #[test]
    fn reads_what_windows_says() {
        let r = refusal("0x80131500 Deployment failed with HRESULT: 0x80073CF9, Install failed. Please contact your software vendor. NOTE: For additional information, look for [ActivityId] 1234 in the Event Log");
        assert_eq!(r, Refusal { code: "0x80073CF9".into(), message: "Deployment failed with HRESULT: 0x80073CF9, Install failed. Please contact your software vendor.".into() });
        assert_eq!(refusal("0x80073d06 A higher version is installed."), Refusal { code: "0x80073D06".into(), message: "A higher version is installed.".into() });
        assert_eq!(refusal("Something odd"), Refusal { code: "0x80004005".into(), message: "Something odd".into() });
        assert_eq!(refusal("0x80004005 "), Refusal { code: "0x80004005".into(), message: String::new() });
        assert_eq!(refusal(""), Refusal { code: "0x80004005".into(), message: String::new() });
        // Windows in another language: the code is still found, and no character boundary is cut.
        assert_eq!(refusal("0x80131500 การปรับใช้ล้มเหลวด้วย HRESULT: 0x80073CF3, แพคเกจ").code, "0x80073CF3");
        assert_eq!(refusal("ผิดพลาด HRESULT: 0x8").code, "0x80004005");
    }

    #[test]
    fn reads_how_a_deployment_ended() {
        assert_eq!(deploy_result("ok\r\n"), Ok(()));
        assert_eq!(deploy_result("missing"), Ok(()));
        // Refused on one drive, taken on the next.
        assert_eq!(deploy_result("refused 0x80131500 Deployment failed with HRESULT: 0xD05E0138, x\r\nok\r\n\r\n"), Ok(()));
        let e = deploy_result("refused 0x80131500 Deployment failed with HRESULT: 0x80073D06, A higher version\nerror 0x80131500 Deployment failed with HRESULT: 0x80073D06, A higher version\n").unwrap_err();
        assert_eq!(e.code, "0x80073D06");
        assert_eq!(deploy_result("").unwrap_err().code, "0x80004005");
        assert_eq!(deploy_result("WARNING: noise").unwrap_err().message, "WARNING: noise");
    }

    #[test]
    fn scripts_pass_through_the_command_line_and_stay_in_their_lane() {
        for script in [REMOVE_SCRIPT, ADD_SCRIPT, REGISTER_SCRIPT] {
            // Rust quotes double quotes as \" on Windows, which PowerShell's -Command doesn't read back the same way.
            assert!(!script.contains('"'));
            // No script changes Developer Mode or any other setting, or reaches past this Windows user.
            for word in ["AppModelUnlock", "Set-ItemProperty", "New-ItemProperty", "reg add", "Set-ExecutionPolicy", "-AllUsers", "Invoke-CommandInDesktopPackage"] {
                assert!(!script.contains(word), "{word}");
            }
        }
        // Only a copy registered from a folder may keep its app data, and only the register script registers.
        assert!(REMOVE_SCRIPT.contains("if ($x.IsDevelopmentMode) { Remove-AppxPackage -Package $x.PackageFullName -PreserveApplicationData }"));
        assert!(REGISTER_SCRIPT.contains("Add-AppxPackage -Register $Path"));
        assert!(!ADD_SCRIPT.contains("-Register"));
        assert_eq!(ps_quote("C:\\Users\\O'Brien\\x.msixvc"), "'C:\\Users\\O''Brien\\x.msixvc'");
    }

    #[test]
    fn knows_when_the_installed_game_has_to_go_first() {
        let store = InstalledGame { version: "1.26.65.1".into(), development: false, location: Some("C:\\XboxGames\\Minecraft for Windows\\Content".into()) };
        assert!(needs_removal(&store, &gdk("1.21.130.4", None)), "Windows won't put an older version over a newer one");
        assert!(needs_removal(&store, &uwp("1.20.81.1", 1)));
        let older = InstalledGame { version: "1.21.130.4".into(), ..store.clone() };
        assert!(!needs_removal(&older, &gdk("1.26.65.1", None)), "a newer version goes over it as an update");
        let registered = InstalledGame { version: "1.20.81.1".into(), development: true, location: Some("D:\\ResinCraft\\bedrock-versions\\1.20.81.1".into()) };
        assert!(needs_removal(&registered, &gdk("1.26.65.1", None)));
        assert!(needs_removal(&registered, &uwp("1.21.114.1", 1)));
        assert!(is_target(&older, &gdk("1.21.130.04", None)) && !is_target(&older, &gdk("1.21.131.1", None)));
        assert_eq!(drive_of("C:\\XboxGames\\Minecraft for Windows\\Content").as_deref(), Some("C:\\"));
        assert_eq!(drive_of("\\\\server\\share\\x"), None);
        assert_eq!(drive_of(""), None);
    }

    #[test]
    fn says_what_state_a_refusal_left() {
        let r = Refusal { code: "0x80073CF9".into(), message: "Install failed.".into() };
        let kept = refused_error("1.21.130.4", &r, false, None);
        assert_eq!(key(&kept), "bedrock.switch.refusedKept");
        assert_eq!(kept.to_string(), "Windows refused Minecraft 1.21.130.4 (0x80073CF9): Install failed. The installed version was left as it is.");
        assert_eq!(key(&refused_error("1.21.130.4", &r, true, None)), "bedrock.switch.refused");
        let store = anyhow::anyhow!("no license");
        let both = refused_error("1.21.130.4", &r, true, Some(&store));
        assert_eq!(key(&both), "bedrock.switch.refusedStoreFailed");
        assert!(both.to_string().contains("(no license)"));
    }

    const GOOD_MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?><Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"><Identity Name="Microsoft.MinecraftUWP" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US" Version="1.20.8101.0" ProcessorArchitecture="x64" /><Properties><DisplayName>Minecraft</DisplayName><PublisherDisplayName>Microsoft Studios</PublisherDisplayName></Properties></Package>"#;

    #[test]
    fn reads_a_package_identity() {
        assert_eq!(manifest_identity(GOOD_MANIFEST), Some((PACKAGE_NAME.into(), PACKAGE_PUBLISHER.into())));
        assert_eq!(manifest_identity("<Package><Identity Publisher='CN=A &amp; B' Name = 'X.Y'/></Package>"), Some(("X.Y".into(), "CN=A & B".into())));
        assert_eq!(manifest_identity("<Package><Identity Name=\"X\"/></Package>"), None);
        assert_eq!(manifest_identity("<Package><Properties><PublisherDisplayName>Microsoft</PublisherDisplayName></Properties></Package>"), None);
        assert_eq!(manifest_identity(""), None);
    }

    /// A scratch folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!("resincraft-bedrock-switch-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A package file with these entries.
    fn package(path: &Path, files: &[(&str, &[u8])]) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn unpacks_only_minecraft_from_microsoft() {
        let s = Scratch::new("unpack");
        let good = s.0.join("good.appx");
        package(&good, &[(MANIFEST_FILE, GOOD_MANIFEST.as_bytes()), ("Minecraft.Windows.exe", b"MZ"), ("data/skin_packs/vanilla/skins.json", b"{}"), (SIGNATURE_FILE, b"signature")]);
        let dir = s.0.join("versions").join("1.20.81.1");
        unpack(&good, &dir).unwrap();
        assert_eq!(std::fs::read(dir.join("Minecraft.Windows.exe")).unwrap(), b"MZ");
        assert_eq!(std::fs::read(dir.join("data").join("skin_packs").join("vanilla").join("skins.json")).unwrap(), b"{}");
        assert!(dir.join(MANIFEST_FILE).is_file() && dir.join(UNPACKED_MARK).is_file());
        assert!(!dir.join(SIGNATURE_FILE).exists(), "Windows doesn't take the signature with files registered from a folder");
        assert!(!s.0.join("versions").join("1.20.81.1.unpacking").exists());
        // Unpacked whole before: used as it is, the package isn't even opened.
        unpack(&s.0.join("gone.appx"), &dir).unwrap();

        // Another app's package, however it got here: nothing stays.
        let other = s.0.join("other.appx");
        package(&other, &[(MANIFEST_FILE, GOOD_MANIFEST.replace("Microsoft.MinecraftUWP", "Contoso.Game").as_bytes()), ("a.exe", b"MZ")]);
        let wrong = s.0.join("versions").join("1.19.83.1");
        assert_eq!(key(&unpack(&other, &wrong).unwrap_err()), "bedrock.switch.wrongPackage");
        assert!(!wrong.exists() && !s.0.join("versions").join("1.19.83.1.unpacking").exists());
        // The right name from another publisher.
        package(&other, &[(MANIFEST_FILE, GOOD_MANIFEST.replace("CN=Microsoft Corporation, O=Microsoft Corporation", "CN=Someone, O=Else").as_bytes())]);
        assert_eq!(key(&unpack(&other, &wrong).unwrap_err()), "bedrock.switch.wrongPackage");
        // No manifest at all, and a file that isn't a package.
        package(&other, &[("a.exe", b"MZ")]);
        assert_eq!(key(&unpack(&other, &wrong).unwrap_err()), "bedrock.switch.wrongPackage");
        std::fs::write(&other, b"<html>502 Bad Gateway</html>").unwrap();
        assert_eq!(key(&unpack(&other, &wrong).unwrap_err()), "bedrock.switch.badPackage");
        // A package that tries to write outside its folder.
        package(&other, &[(MANIFEST_FILE, GOOD_MANIFEST.as_bytes()), ("../../escaped.txt", b"x")]);
        assert_eq!(key(&unpack(&other, &wrong).unwrap_err()), "bedrock.switch.badPackage");
        assert!(!s.0.join("escaped.txt").exists() && !s.0.join("versions").join("escaped.txt").exists() && !wrong.exists());

        // A folder left half unpacked (no mark) is unpacked again from scratch.
        std::fs::remove_file(dir.join(UNPACKED_MARK)).unwrap();
        std::fs::write(dir.join("stale.txt"), b"x").unwrap();
        unpack(&good, &dir).unwrap();
        assert!(dir.join(UNPACKED_MARK).is_file() && !dir.join("stale.txt").exists());
    }

    #[test]
    fn finds_a_framework_that_is_not_installed() {
        // As in Minecraft 1.20.81.1's manifest; the injection attempt is dropped.
        let xml = r#"<Dependencies><TargetDeviceFamily Name="Windows.Universal" MinVersion="10.0.10240.0"/><PackageDependency Name="Microsoft.VCLibs.140.00" MinVersion="14.0.22929.0" Publisher="CN=Microsoft Corporation"/><PackageDependency MinVersion='10.0.1610.0' Name='Microsoft.Services.Store.Engagement'/><PackageDependency Name="x'; Remove-Item C:\" MinVersion="1.0"/></Dependencies>"#;
        let needs = manifest_dependencies(xml);
        assert_eq!(needs, vec![("Microsoft.VCLibs.140.00".to_string(), "14.0.22929.0".to_string()), ("Microsoft.Services.Store.Engagement".to_string(), "10.0.1610.0".to_string())]);
        // What this PC had (2026-11-20): VCLibs, no Store Engagement.
        let installed = parse_installed_packages(r#"[{"Name":"Microsoft.VCLibs.140.00","Version":"14.0.33519.0","Architecture":"X64"},{"Name":"Microsoft.VCLibs.140.00","Version":"14.0.33519.0","Architecture":"X86"}]"#);
        assert_eq!(installed.len(), 2);
        assert_eq!(missing_dependency(&needs, &installed), Some(("Microsoft.Services.Store.Engagement".to_string(), "10.0.1610.0".to_string())));
        assert_eq!(missing_dependency(&needs[..1], &installed), None);
        // Too old, or only for another processor.
        let old = parse_installed_packages(r#"{"Name":"Microsoft.VCLibs.140.00","Version":"14.0.2000.0","Architecture":"X64"}"#);
        assert!(missing_dependency(&needs[..1], &old).is_some());
        let x86 = parse_installed_packages(r#"[{"Name":"Microsoft.VCLibs.140.00","Version":"14.0.33519.0","Architecture":"X86"}]"#);
        assert!(missing_dependency(&needs[..1], &x86).is_some());
        let neutral = parse_installed_packages(r#"[{"Name":"microsoft.services.store.engagement","Version":"10.0.23012.0","Architecture":"Neutral"}]"#);
        assert_eq!(missing_dependency(&needs[1..], &neutral), None);
        assert!(parse_installed_packages("").is_empty());
        assert_eq!(key(&blocker_error(Blocker::Running, &plan(&uwp("1.20.81.1", 1), &ready()))), "bedrock.switch.closeFirst");
    }

    #[test]
    fn prunes_the_unpacked_versions_that_are_not_in_use() {
        let s = Scratch::new("prune");
        for version in ["1.20.81.1", "1.19.83.1"] {
            std::fs::create_dir_all(s.0.join(version)).unwrap();
            std::fs::write(s.0.join(version).join(MANIFEST_FILE), GOOD_MANIFEST).unwrap();
        }
        std::fs::create_dir_all(s.0.join("1.18.31.4.unpacking").join("data")).unwrap();
        // Not ResinCraft's unpacked packages: left alone.
        std::fs::create_dir_all(s.0.join("my notes")).unwrap();
        std::fs::write(s.0.join("my notes").join("a.txt"), b"x").unwrap();
        std::fs::write(s.0.join("file.txt"), b"x").unwrap();
        prune_versions(&s.0, Some("1.20.81.1"));
        assert!(s.0.join("1.20.81.1").join(MANIFEST_FILE).is_file());
        assert!(!s.0.join("1.19.83.1").exists() && !s.0.join("1.18.31.4.unpacking").exists());
        assert!(s.0.join("my notes").join("a.txt").is_file() && s.0.join("file.txt").is_file());
        prune_versions(&s.0, None);
        assert!(!s.0.join("1.20.81.1").exists() && s.0.join("my notes").is_dir());
        prune_versions(&s.0.join("nowhere"), None);
    }
}

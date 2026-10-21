//! Installing, updating and repairing Bedrock from inside ResinCraft. The Microsoft Store's own install service
//! (AppInstallManager, which winget's Store source also uses) does the work, with the account signed in to the
//! Store and its license: ResinCraft only asks it to start, shows its progress and can cancel it. It never
//! downloads or unpacks the game itself. winget can't do this: its Store source doesn't list Minecraft (checked
//! with `winget search --id 9NBLGGH2JHXJ --source msstore`, and the same for Preview).
//!
//! `bedrock_store.ps1` runs hidden in Windows PowerShell (windows-rs doesn't carry the Store's API) and prints one
//! line per step; `parse_line` reads them and `failure` turns the Store's error codes into messages.

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::bedrock::Edition;
use crate::core::events::Task;
use crate::core::text::Text;
use crate::{err_tr, tr};

/// What the player asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Install,
    Update,
    /// Installs it again over itself, fixing broken files (worlds stay).
    Repair,
}

impl Mode {
    fn arg(self) -> &'static str {
        match self {
            Mode::Install => "install",
            Mode::Update => "update",
            Mode::Repair => "repair",
        }
    }
}

/// How it ended (failures are errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Done,
    /// There was no update to install.
    UpToDate,
}

/// The Store's install states (AppInstallState).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Pending,
    ReadyToDownload,
    Starting,
    AcquiringLicense,
    Downloading,
    RestoringData,
    Installing,
    Completed,
    Canceled,
    Paused,
    PausedLowBattery,
    PausedWiFi,
    Error,
}

impl State {
    fn parse(s: &str) -> Option<State> {
        Some(match s {
            "Pending" => State::Pending,
            "ReadyToDownload" => State::ReadyToDownload,
            "Starting" => State::Starting,
            "AcquiringLicense" => State::AcquiringLicense,
            "Downloading" => State::Downloading,
            "RestoringData" => State::RestoringData,
            "Installing" => State::Installing,
            "Completed" => State::Completed,
            "Canceled" => State::Canceled,
            "Paused" => State::Paused,
            "PausedLowBattery" => State::PausedLowBattery,
            "PausedWiFiRecommended" | "PausedWiFiRequired" => State::PausedWiFi,
            "Error" => State::Error,
            _ => return None,
        })
    }
}

/// One line from `bedrock_store.ps1`.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// The Store took the request (`count` items: the game and what it needs first).
    Started(usize),
    /// The game's progress: percent 0–100, bytes downloaded of the download's size (0 while unknown).
    Status { state: State, percent: f64, done: u64, total: u64 },
    /// Something the game needs (Gaming Services) is being installed first.
    Dependency { state: State, percent: f64 },
    Done,
    Canceled,
    UpToDate,
    /// A policy on this PC (or the Store's settings) doesn't allow installing it.
    Blocked,
    /// No Microsoft Store install service on this Windows.
    NoStore,
    Error { code: u32, message: String },
}

pub fn parse_line(line: &str) -> Option<Line> {
    let line = line.trim();
    let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
    let mut parts = rest.split_whitespace();
    Some(match word {
        "started" => Line::Started(rest.trim().parse().unwrap_or(1)),
        "status" => Line::Status {
            state: State::parse(parts.next()?)?,
            percent: parts.next().and_then(|p| p.parse().ok()).unwrap_or(0.0),
            done: parts.next().and_then(|p| p.parse().ok()).unwrap_or(0),
            total: parts.next().and_then(|p| p.parse().ok()).unwrap_or(0),
        },
        "dep" => Line::Dependency { state: State::parse(parts.next()?)?, percent: parts.next().and_then(|p| p.parse().ok()).unwrap_or(0.0) },
        "done" => Line::Done,
        "canceled" => Line::Canceled,
        "uptodate" => Line::UpToDate,
        "blocked" => Line::Blocked,
        "nostore" => Line::NoStore,
        "error" => {
            let (code, message) = rest.trim().split_once(' ').unwrap_or((rest.trim(), ""));
            let code = u32::from_str_radix(code.trim_start_matches("0x").trim_start_matches("0X"), 16).ok()?;
            Line::Error { code, message: message.trim().to_string() }
        }
        _ => return None,
    })
}

/// Why it failed, so the UI can offer the right way out (the error's key says which).
pub fn failure(edition: Edition, mode: Mode, code: u32, message: &str) -> anyhow::Error {
    let name = edition.name();
    let code_hex = format!("0x{code:08X}");
    match code {
        // The Store's licensing: no license for the account signed in to the Store, or nobody signed in.
        0x803F_8000..=0x803F_8FFF | 0x8007_0520 | 0x8004_A029 => err_tr!(
            "bedrock.store.notOwned",
            "The Microsoft Store couldn't find a license for {name} on the account signed in to the Store ({code}). Open the Store, sign in with the Microsoft account that owns Minecraft (or get it there), then try again.",
            name = name,
            code = code_hex
        ),
        // Not installed, so there's nothing to update.
        0x8007_0490 if mode == Mode::Update => {
            err_tr!("bedrock.store.notInstalled", "{name} isn't installed, so there's nothing to update. Install it first.", name = name)
        }
        // Gaming Services: the Xbox service that installs and runs Store games like Bedrock.
        0x87E0_0000..=0x87E0_FFFF | 0x8007_0424 | 0x8007_3D26 | 0x8007_3D27 | 0x8007_0BC9 => err_tr!(
            "bedrock.store.gamingServices",
            "Gaming Services, which Windows needs to install {name}, is missing or not working ({code}). Install or repair Gaming Services from the Microsoft Store, restart the PC, then try again.",
            name = name,
            code = code_hex
        ),
        // No internet: WinINet's errors (12000–12175), Delivery Optimization's and Windows Update's network errors.
        0x8007_2EE0..=0x8007_2F8F | 0x80D0_0000..=0x80D0_FFFF | 0x8024_4000..=0x8024_40FF | 0x8007_04CF | 0x8007_274C => err_tr!(
            "bedrock.store.offline",
            "The Microsoft Store couldn't download {name}: check the internet connection, then try again ({code}).",
            name = name,
            code = code_hex
        ),
        0x8007_0070 | 0x8007_0027 | 0x8007_3D19 => err_tr!(
            "bedrock.store.diskFull",
            "There isn't enough free disk space to install {name} ({code}). Free some space, then try again.",
            name = name,
            code = code_hex
        ),
        _ => err_tr!(
            "bedrock.store.failed",
            "The Microsoft Store couldn't install {name} ({code}): {error}",
            name = name,
            code = code_hex,
            error = if message.is_empty() { "-".to_string() } else { message.to_string() }
        ),
    }
}

/// The task dock's line for a state.
fn detail(state: State, done: u64, total: u64) -> Text {
    match state {
        State::Pending | State::ReadyToDownload | State::Starting => tr!("bedrock.store.queued", "Waiting for the Microsoft Store"),
        State::AcquiringLicense => tr!("bedrock.store.license", "Checking the license"),
        State::Downloading if total >= 1_000_000 => {
            tr!("bedrock.store.downloading", "Downloading ({done} of {total} MB)", done = (done / 1_000_000).to_string(), total = (total / 1_000_000).to_string())
        }
        State::Downloading => tr!("bedrock.store.downloadingStart", "Downloading"),
        State::RestoringData | State::Installing => tr!("bedrock.store.installing", "Installing"),
        State::Completed => tr!("bedrock.store.finished", "Finished"),
        State::Paused => tr!("bedrock.store.paused", "Paused in the Microsoft Store (it resumes on its own, or resume it in the Store's Downloads)"),
        State::PausedLowBattery => tr!("bedrock.store.pausedBattery", "Paused until the PC is plugged in"),
        State::PausedWiFi => tr!("bedrock.store.pausedWifi", "Paused: the Store waits for Wi-Fi or an unmetered connection"),
        State::Canceled | State::Error => tr!("bedrock.store.stopping", "Stopping"),
    }
}

/// Applies one line to the task. Some(result) when the install has ended.
pub fn step(task: &Task, edition: Edition, mode: Mode, line: Line) -> Option<Result<Outcome>> {
    match line {
        Line::Started(_) => task.update(tr!("bedrock.store.queued", "Waiting for the Microsoft Store"), None),
        Line::Status { state, percent, done, total } => {
            let p = (state == State::Downloading || state == State::Installing).then_some((percent / 100.0).clamp(0.0, 1.0));
            task.update(detail(state, done, total), p);
        }
        Line::Dependency { percent, .. } => task.update(
            tr!("bedrock.store.dependency", "Installing what Minecraft needs first ({percent}%)", percent = (percent.round() as i64).to_string()),
            None,
        ),
        Line::Done => return Some(Ok(Outcome::Done)),
        Line::UpToDate => return Some(Ok(Outcome::UpToDate)),
        Line::Canceled => return Some(Err(crate::core::events::cancelled_error())),
        Line::Blocked => {
            return Some(Err(err_tr!(
                "bedrock.store.blocked",
                "This PC doesn't allow installing {name} from the Microsoft Store (a policy or the Store's settings block it).",
                name = edition.name()
            )))
        }
        Line::NoStore => {
            return Some(Err(err_tr!(
                "bedrock.store.missing",
                "This Windows has no Microsoft Store install service, so {name} can't be installed from here.",
                name = edition.name()
            )))
        }
        Line::Error { code, message } => return Some(Err(failure(edition, mode, code, &message))),
    }
    None
}

/// Cancels the Store's install when the task is cancelled (or ResinCraft stops waiting for it any other way):
/// killing the helper alone would leave the Store installing.
struct CancelOnDrop {
    product: &'static str,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let script = format!(
            "Add-Type -AssemblyName System.Runtime.WindowsRuntime; \
             [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager, Windows.ApplicationModel.Store.Preview, ContentType = WindowsRuntime]::new().Cancel('{}')",
            self.product
        );
        let mut cmd = std::process::Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let _ = cmd.spawn();
    }
}

/// Installs, updates or repairs an edition as a cancellable task.
pub async fn run(edition: Edition, mode: Mode) -> Result<Outcome> {
    if !cfg!(windows) {
        crate::bail_tr!("bedrock.store.windowsOnly", "Bedrock Edition installs only on Windows.");
    }
    let name = edition.name();
    let title = match mode {
        Mode::Install => tr!("bedrock.task.install", "Installing {name}", name = name),
        Mode::Update => tr!("bedrock.task.update", "Updating {name}", name = name),
        Mode::Repair => tr!("bedrock.task.repair", "Repairing {name}", name = name),
    };
    // Tied to `bedrock:release` / `bedrock:preview` so the Bedrock page can show it on the edition's card.
    let owner = format!("bedrock:{}", match edition {
        Edition::Release => "release",
        Edition::Preview => "preview",
    });
    Task::run(title, Some(owner), move |task: Arc<Task>| async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        task.update(tr!("bedrock.store.queued", "Waiting for the Microsoft Store"), None);
        // The ids are fixed strings; the script has no double quotes, so it passes through -Command as it is.
        let script = format!("$Action = '{}'; $ProductId = '{}'\n{}", mode.arg(), edition.product_id(), include_str!("bedrock_store.ps1"));
        let mut child = crate::game::java::hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| err_tr!("bedrock.store.noPowershell", "Couldn't start Windows PowerShell to talk to the Microsoft Store: {error}", error = e.to_string()))?;
        let mut guard = CancelOnDrop { product: edition.product_id(), armed: true };
        let mut lines = BufReader::new(child.stdout.take().expect("piped")).lines();
        let mut result = None;
        while let Some(line) = lines.next_line().await? {
            if let Some(r) = parse_line(&line).and_then(|l| step(&task, edition, mode, l)) {
                result = Some(r);
                break;
            }
        }
        // Ended either way: nothing left to cancel.
        guard.armed = false;
        let _ = child.wait().await;
        result.unwrap_or_else(|| Err(failure(edition, mode, 0x8000_4005, "")))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text::text_of;

    fn key(e: &anyhow::Error) -> String {
        text_of(e).map(|t| t.key.clone()).unwrap_or_default()
    }

    #[test]
    fn reads_helper_lines() {
        assert_eq!(parse_line("started 2"), Some(Line::Started(2)));
        assert_eq!(
            parse_line("status Downloading 42 123456789 987654321\r"),
            Some(Line::Status { state: State::Downloading, percent: 42.0, done: 123_456_789, total: 987_654_321 })
        );
        assert_eq!(parse_line("status PausedWiFiRequired 3 0 0"), Some(Line::Status { state: State::PausedWiFi, percent: 3.0, done: 0, total: 0 }));
        assert_eq!(parse_line("dep Installing 60"), Some(Line::Dependency { state: State::Installing, percent: 60.0 }));
        assert_eq!(parse_line("uptodate"), Some(Line::UpToDate));
        assert_eq!(parse_line("done"), Some(Line::Done));
        assert_eq!(parse_line("canceled"), Some(Line::Canceled));
        assert_eq!(parse_line("blocked"), Some(Line::Blocked));
        assert_eq!(parse_line("nostore"), Some(Line::NoStore));
        assert_eq!(
            parse_line("error 0x80070490 Element not found. (Exception from HRESULT: 0x80070490)"),
            Some(Line::Error { code: 0x8007_0490, message: "Element not found. (Exception from HRESULT: 0x80070490)".into() })
        );
        assert_eq!(parse_line("error 0x80004005 "), Some(Line::Error { code: 0x8000_4005, message: String::new() }));
        // PowerShell noise and unknown states are skipped.
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("status Teleporting 1 2 3"), None);
        assert_eq!(parse_line("WARNING: something"), None);
    }

    #[test]
    fn store_errors_read_as_reasons() {
        let r = Edition::Release;
        let reason = |code: u32, mode: Mode| key(&failure(r, mode, code, "x"));
        assert_eq!(reason(0x803F_8001, Mode::Install), "bedrock.store.notOwned");
        assert_eq!(reason(0x8007_0520, Mode::Install), "bedrock.store.notOwned");
        assert_eq!(reason(0x8007_0490, Mode::Update), "bedrock.store.notInstalled");
        assert_eq!(reason(0x8007_0490, Mode::Install), "bedrock.store.failed");
        assert_eq!(reason(0x87E0_0017, Mode::Install), "bedrock.store.gamingServices");
        assert_eq!(reason(0x8007_3D26, Mode::Repair), "bedrock.store.gamingServices");
        assert_eq!(reason(0x8007_2EE7, Mode::Install), "bedrock.store.offline");
        assert_eq!(reason(0x80D0_3805, Mode::Update), "bedrock.store.offline");
        assert_eq!(reason(0x8007_0070, Mode::Install), "bedrock.store.diskFull");
        let other = failure(r, Mode::Install, 0x8000_FFFF, "Catastrophic failure");
        assert_eq!(key(&other), "bedrock.store.failed");
        assert_eq!(other.to_string(), "The Microsoft Store couldn't install Minecraft for Windows (0x8000FFFF): Catastrophic failure");
    }

    #[test]
    fn helper_lines_end_the_task() {
        let task = Task::new("test", None);
        let (r, m) = (Edition::Preview, Mode::Install);
        assert!(step(&task, r, m, Line::Status { state: State::Downloading, percent: 10.0, done: 1, total: 2 }).is_none());
        assert!(step(&task, r, m, Line::Dependency { state: State::Installing, percent: 5.0 }).is_none());
        assert_eq!(step(&task, r, m, Line::Done).unwrap().unwrap(), Outcome::Done);
        assert_eq!(step(&task, r, Mode::Update, Line::UpToDate).unwrap().unwrap(), Outcome::UpToDate);
        assert!(crate::core::events::is_cancelled(&step(&task, r, m, Line::Canceled).unwrap().unwrap_err()));
        assert_eq!(key(&step(&task, r, m, Line::Blocked).unwrap().unwrap_err()), "bedrock.store.blocked");
        assert_eq!(key(&step(&task, r, m, Line::NoStore).unwrap().unwrap_err()), "bedrock.store.missing");
        let e = step(&task, r, m, Line::Error { code: 0x803F_8001, message: String::new() }).unwrap().unwrap_err();
        assert!(e.to_string().contains("Minecraft Preview"));
    }

    /// The real Store on this PC: `BEDROCK_EDITION=release|preview BEDROCK_MODE=install|update|repair cargo test
    /// --lib real_store -- --ignored --nocapture` (prints each task event).
    #[tokio::test]
    #[ignore = "installs through the real Microsoft Store"]
    async fn real_store() {
        crate::core::events::set_sink(|channel, v| println!("{channel}: {v}"));
        let edition = if std::env::var("BEDROCK_EDITION").as_deref() == Ok("preview") { Edition::Preview } else { Edition::Release };
        let mode = match std::env::var("BEDROCK_MODE").as_deref() {
            Ok("install") => Mode::Install,
            Ok("repair") => Mode::Repair,
            _ => Mode::Update,
        };
        println!("result: {:?}", run(edition, mode).await.map_err(|e| e.to_string()));
    }

    #[test]
    fn helper_script_passes_through_the_command_line() {
        // Rust quotes double quotes as \" on Windows, which PowerShell's -Command doesn't read back the same way.
        assert!(!include_str!("bedrock_store.ps1").contains('"'));
    }
}

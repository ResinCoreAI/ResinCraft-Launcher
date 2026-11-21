//! When the game's own window appears. The launcher's launch screen stays up until then and folds away the moment
//! it does, so the game seems to open straight out of it (components/LaunchScreen.tsx). The game's log isn't a good
//! sign of this: Forge prints "LWJGL" well before its window, and some versions print nothing at all.
//!
//! On Windows a thread looks, a few times a second, for a visible top-level window of a real size that belongs to
//! the game's process or one it started (a wrapper command's Java). It sends `gameWindow` once, then stops; it also
//! stops when the process is gone or after ten minutes. Elsewhere nothing is sent and the UI falls back to the log.

use serde::Serialize;

use crate::core::events::emit;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameWindow {
    pub instance_id: String,
    pub run_id: String,
}

/// Whether a game log line says the game has finished loading: the sound engine starts last, just before the title
/// screen, on every version from 1.7 on (or says it couldn't). The launch screen folds then, and the launcher's
/// "When the game starts" action (minimise, hide) happens then, not when the process or its window first appears.
pub fn ready_line(text: &str) -> bool {
    text.contains("Sound engine started") || text.contains("Sound engine startup failed") || text.contains("Error starting SoundSystem")
}

/// Whether a game log line is the game shutting down because the player quit: Minecraft logs "Stopping!" when it
/// closes normally (every version from 1.7 on). Some mods' native libraries then make Java exit with an error code
/// while it unwinds; after this line, that is not a crash.
pub fn clean_stop_line(text: &str) -> bool {
    text.ends_with("Stopping!") && !text.contains("Stopping!!")
}

/// Whether a game log line says the game wrote a crash report: a crash, whatever the exit code.
pub fn crash_report_line(text: &str) -> bool {
    text.contains("#@!@# Game crashed!") || text.contains("---- Minecraft Crash Report ----")
}

/// Smaller than this is a splash or a tool window, not the game.
const MIN_W: i32 = 200;
const MIN_H: i32 = 150;

#[cfg(windows)]
pub fn watch(root: u32, instance_id: String, run_id: String) {
    use std::time::{Duration, Instant};
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut pids = vec![root];
        let mut looked = start;
        while start.elapsed() < Duration::from_secs(600) {
            // The process tree changes rarely: read it again every couple of seconds.
            if looked.elapsed() > Duration::from_secs(2) {
                pids = tree(root);
                looked = Instant::now();
                if pids.is_empty() {
                    return;
                }
            }
            if has_window(&pids) {
                emit("gameWindow", GameWindow { instance_id, run_id });
                return;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    });
}

#[cfg(not(windows))]
pub fn watch(_root: u32, _instance_id: String, _run_id: String) {}

/// `root` and every process under it; empty when `root` has ended.
#[cfg(windows)]
fn tree(root: u32) -> Vec<u32> {
    use std::collections::HashMap;
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
    let mut sys = System::new_with_specifics(RefreshKind::nothing());
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    if sys.process(Pid::from_u32(root)).is_none() {
        return Vec::new();
    }
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, p) in sys.processes() {
        if let Some(parent) = p.parent() {
            children.entry(parent.as_u32()).or_default().push(pid.as_u32());
        }
    }
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() {
        if let Some(kids) = children.get(&out[i]) {
            for k in kids {
                if !out.contains(k) {
                    out.push(*k);
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(windows)]
fn has_window(pids: &[u32]) -> bool {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible};

    struct Search<'a> {
        pids: &'a [u32],
        hit: bool,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // Safety: lparam is the &mut Search passed below, alive for the whole EnumWindows call.
        let s = unsafe { &mut *(lparam.0 as *mut Search) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if !s.pids.contains(&pid) {
            return BOOL(1);
        }
        let mut r = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut r) }.is_ok() && r.right - r.left >= MIN_W && r.bottom - r.top >= MIN_H {
            s.hit = true;
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut search = Search { pids, hit: false };
    // Safety: plain Win32 enumeration; the callback only reads window facts. Stopping early reports an error, which
    // means nothing here.
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut search as *mut Search as isize)) };
    search.hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_how_the_game_ended() {
        assert!(clean_stop_line("[11:45:29] [Render thread/INFO]: Stopping!"));
        assert!(clean_stop_line("[12:00:01] [Client thread/INFO]: Stopping!"));
        assert!(!clean_stop_line("[11:45:29] [Render thread/INFO]: Stopping [0] Placement Manager Daemon threads"));
        assert!(!clean_stop_line("[Server thread/INFO]: Stopping server"));
        assert!(crash_report_line("#@!@# Game crashed! Crash report saved to: crash-2026.txt"));
        assert!(crash_report_line("---- Minecraft Crash Report ----"));
        assert!(ready_line("[11:45:17] [Render thread/INFO]: Sound engine started"));
        assert!(!ready_line("[11:45:17] [Render thread/INFO]: Loaded 16 particles"));
    }
}

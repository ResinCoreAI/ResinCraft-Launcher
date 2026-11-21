//! A game starts at "above normal" priority and goes back to normal once it's up (its sound engine has started, about
//! when the title screen shows), so a busy computer doesn't stretch out the loading screen.
//!
//! Starting a game is mostly the CPU: Fabric applying mixins, the game's bootstrap, mods setting up, then loading
//! resources. With every core busy (a build, a compression, another game loading), Windows shares them out evenly and
//! the start takes two or three times as long: Vanilla Perfected 26.3 (Fabric, 78 mods) took 105-140 s to its title
//! screen with all 12 cores busy, against 53-61 s at above normal priority and 28-35 s on a quiet computer. Once the
//! game is up it is back at normal priority, so playing doesn't crowd out the rest of the computer.
//!
//! Windows only: elsewhere raising a priority needs privileges the launcher doesn't have.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// How long a game may stay above normal if it never says it's up (a version that logs differently, a crash at start).
const AT_MOST: Duration = Duration::from_secs(180);

/// Runs still starting: run id → their process, held open so the priority is put back on that process and no other.
static STARTING: Mutex<Option<HashMap<String, Process>>> = Mutex::new(None);

#[cfg(windows)]
struct Process(windows::Win32::Foundation::HANDLE);
#[cfg(not(windows))]
struct Process;

// A process handle may be used and closed from any thread.
#[cfg(windows)]
unsafe impl Send for Process {}

#[cfg(windows)]
impl Drop for Process {
    fn drop(&mut self) {
        use windows::Win32::System::Threading::{SetPriorityClass, NORMAL_PRIORITY_CLASS};
        unsafe {
            // Fails harmlessly when the game has ended meanwhile.
            let _ = SetPriorityClass(self.0, NORMAL_PRIORITY_CLASS);
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// The game's process is created at above normal priority (it doesn't wait for `started` to get there).
pub fn starting(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::ABOVE_NORMAL_PRIORITY_CLASS;
        cmd.creation_flags(super::java::CREATE_NO_WINDOW | ABOVE_NORMAL_PRIORITY_CLASS.0);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Run `run_id`'s process `pid` is running: back to normal priority when it says it's up (`ready`), or after `AT_MOST`.
pub fn started(run_id: &str, pid: Option<u32>) {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_INFORMATION};
        let Some(pid) = pid else { return };
        let Ok(handle) = (unsafe { OpenProcess(PROCESS_SET_INFORMATION, false, pid) }) else { return };
        STARTING.lock().unwrap().get_or_insert_with(HashMap::new).insert(run_id.to_string(), Process(handle));
        let run_id = run_id.to_string();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(AT_MOST).await;
            ready(&run_id);
        });
    }
    #[cfg(not(windows))]
    let _ = (run_id, pid);
}

/// Run `run_id` is up (or gave up): its process goes back to normal priority.
pub fn ready(run_id: &str) {
    // Dropping the process puts it back.
    let done = STARTING.lock().unwrap().as_mut().and_then(|m| m.remove(run_id));
    drop(done);
}

/// The line Minecraft logs once its sound engine is up, right before the title screen (every version since 1.6):
/// "[12:00:00] [Render thread/INFO]: Sound engine started" ("[Client thread/INFO]" before 1.13).
pub fn says_ready(line: &str) -> bool {
    line.contains("Sound engine started")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_when_the_game_is_up() {
        assert!(says_ready("[13:05:42] [Render thread/INFO]: Sound engine started"));
        assert!(says_ready("[13:05:42] [Client thread/INFO]: Sound engine started"));
        assert!(!says_ready("[13:05:42] [Render thread/INFO]: Starting sound engine"));
    }

    #[test]
    fn a_run_that_never_started_is_nothing_to_put_back() {
        ready("no-such-run");
        started("run-without-a-process", None);
        assert!(STARTING.lock().unwrap().as_ref().map_or(true, |m| !m.contains_key("run-without-a-process")));
    }

    /// The process really starts above normal and is back to normal once it's "ready".
    #[cfg(windows)]
    #[tokio::test]
    async fn a_game_starts_above_normal_and_goes_back() {
        use windows::Win32::System::Threading::{GetPriorityClass, OpenProcess, ABOVE_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION};
        let class = |pid: u32| unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).unwrap();
            let c = GetPriorityClass(h);
            let _ = windows::Win32::Foundation::CloseHandle(h);
            c
        };
        let mut cmd = super::super::java::hidden_command("cmd");
        starting(&mut cmd);
        let mut child = cmd.args(["/C", "ping -n 4 127.0.0.1 >NUL"]).spawn().unwrap();
        let pid = child.id().unwrap();
        assert_eq!(class(pid), ABOVE_NORMAL_PRIORITY_CLASS.0);
        started("test-run", Some(pid));
        ready("test-run");
        assert_eq!(class(pid), NORMAL_PRIORITY_CLASS.0);
        let _ = child.kill().await;
    }
}

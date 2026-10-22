//! How much RAM the launcher, each running game and each running server use (`app:memory`), for the instance page,
//! the server page and the optional Home card. A process counts with everything it started (a wrapper's Java,
//! WebView2's helpers); games and servers are left out of the launcher's own share even though it started them.
//!
//! Each process counts the memory that's really its own, as the system's monitor shows it: on Windows the private
//! working set (Task Manager's Memory column), on Linux the proportional set size (PSS, shared pages split between
//! the processes sharing them), on macOS the physical footprint (Activity Monitor's Memory column). Plain resident
//! sizes count shared libraries once per process, so a launcher with six processes looked half again as big.
//!
//! On Linux the process list also holds every thread, each reporting its whole process's memory; threads are left
//! out, or a Java server with hundreds of them reads hundreds of times too much ("397 GB of 4 GB").

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameMemory {
    pub instance_id: String,
    pub run_id: String,
    pub username: String,
    pub bytes: u64,
    /// The memory the game was given (-Xmx).
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerMemory {
    pub server_id: String,
    pub bytes: u64,
    /// The memory a Java server was given (-Xmx); none for Bedrock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUsage {
    /// The launcher itself: its own process and its WebView2 helpers.
    pub launcher: u64,
    pub launcher_processes: usize,
    pub games: Vec<GameMemory>,
    pub servers: Vec<ServerMemory>,
    pub system_total: u64,
    pub system_used: u64,
}

const MB: u64 = 1024 * 1024;

/// Every process under `root` (itself included), from a parent → children map.
fn tree(root: u32, children: &HashMap<u32, Vec<u32>>, skip: &HashSet<u32>) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    let mut seen = HashSet::new();
    while let Some(p) = stack.pop() {
        if !seen.insert(p) || (p != root && skip.contains(&p)) {
            continue;
        }
        out.push(p);
        if let Some(c) = children.get(&p) {
            stack.extend(c.iter().copied());
        }
    }
    out
}

/// Memory only this process uses (Task Manager's "Memory"); None when it can't be read.
#[cfg(windows)]
fn private_bytes(pid: u32) -> Option<u64> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut c = PROCESS_MEMORY_COUNTERS_EX2 { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32, ..Default::default() };
        let read = GetProcessMemoryInfo(handle, &mut c as *mut PROCESS_MEMORY_COUNTERS_EX2 as *mut PROCESS_MEMORY_COUNTERS, c.cb);
        let _ = CloseHandle(handle);
        read.ok().map(|_| c.PrivateWorkingSetSize as u64).filter(|b| *b > 0)
    }
}

/// Linux: PSS from /proc/<pid>/smaps_rollup (kB), the kernel's fair share of a process's memory.
#[cfg(target_os = "linux")]
fn private_bytes(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    parse_pss_kb(&text).map(|kb| kb * 1024)
}

#[cfg(any(target_os = "linux", test))]
fn parse_pss_kb(smaps_rollup: &str) -> Option<u64> {
    let line = smaps_rollup.lines().find(|l| l.starts_with("Pss:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// macOS: the physical footprint (what Activity Monitor shows as Memory).
#[cfg(target_os = "macos")]
fn private_bytes(pid: u32) -> Option<u64> {
    let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::proc_pid_rusage(pid as libc::c_int, libc::RUSAGE_INFO_V2, &mut info as *mut _ as *mut libc::rusage_info_t) };
    (ok == 0 && info.ri_phys_footprint > 0).then_some(info.ri_phys_footprint)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn private_bytes(_pid: u32) -> Option<u64> {
    None
}

/// Sums memory over process trees. `procs`: pid → (parent, bytes).
fn sum_tree(root: u32, procs: &HashMap<u32, (Option<u32>, u64)>, children: &HashMap<u32, Vec<u32>>, skip: &HashSet<u32>) -> (u64, usize) {
    let pids = tree(root, children, skip);
    let bytes = pids.iter().filter_map(|p| procs.get(p).map(|(_, b)| *b)).sum();
    (bytes, pids.iter().filter(|p| procs.contains_key(p)).count())
}

fn measure(games: Vec<crate::game::launch::GameProcess>, servers: Vec<(String, u32, Option<u32>)>) -> MemoryUsage {
    let mut sys = System::new_with_specifics(RefreshKind::nothing());
    sys.refresh_memory();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory());
    // Threads (Linux lists them as processes) would count their process's memory again each.
    let procs: HashMap<u32, (Option<u32>, u64)> = sys
        .processes()
        .iter()
        .filter(|(_, p)| p.thread_kind().is_none())
        .map(|(pid, p)| (pid.as_u32(), (p.parent().map(Pid::as_u32), p.memory())))
        .collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, (parent, _)) in &procs {
        if let Some(parent) = parent {
            children.entry(*parent).or_default().push(*pid);
        }
    }
    let roots: HashSet<u32> = games.iter().map(|g| g.pid).chain(servers.iter().map(|s| s.1)).collect();
    let me = std::process::id();
    // Only the processes shown get the slower, exact reading.
    let mut procs = procs;
    let shown: HashSet<u32> = std::iter::once(me).chain(roots.iter().copied()).flat_map(|r| tree(r, &children, &HashSet::new())).collect();
    for pid in shown {
        if let (Some(private), Some(entry)) = (private_bytes(pid), procs.get_mut(&pid)) {
            entry.1 = private;
        }
    }
    let (launcher, launcher_processes) = sum_tree(me, &procs, &children, &roots);
    MemoryUsage {
        launcher,
        launcher_processes,
        games: games
            .into_iter()
            .map(|g| GameMemory {
                bytes: sum_tree(g.pid, &procs, &children, &HashSet::new()).0,
                max_bytes: g.max_mb as u64 * MB,
                instance_id: g.instance_id,
                run_id: g.run_id,
                username: g.username,
            })
            .collect(),
        servers: servers
            .into_iter()
            .map(|(id, pid, max)| ServerMemory { server_id: id, bytes: sum_tree(pid, &procs, &children, &HashSet::new()).0, max_bytes: max.map(|m| m as u64 * MB) })
            .collect(),
        system_total: sys.total_memory(),
        system_used: sys.used_memory(),
    }
}

pub async fn usage() -> MemoryUsage {
    let games = crate::game::launch::game_processes();
    let servers = crate::server::runtime::server_processes();
    tokio::task::spawn_blocking(move || measure(games, servers)).await.unwrap_or(MemoryUsage {
        launcher: 0,
        launcher_processes: 0,
        games: vec![],
        servers: vec![],
        system_total: 0,
        system_used: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_are_not_counted_as_the_launcher() {
        // launcher 1 → webview 2, game 3 (→ java 4 behind a wrapper)
        let procs: HashMap<u32, (Option<u32>, u64)> =
            HashMap::from([(1, (None, 60)), (2, (Some(1), 300)), (3, (Some(1), 10)), (4, (Some(3), 2800)), (9, (None, 5000))]);
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (pid, (parent, _)) in &procs {
            if let Some(p) = parent {
                children.entry(*p).or_default().push(*pid);
            }
        }
        assert_eq!(sum_tree(1, &procs, &children, &HashSet::from([3])), (360, 2));
        assert_eq!(sum_tree(3, &procs, &children, &HashSet::new()), (2810, 2));
    }

    #[test]
    fn reads_linux_pss() {
        let rollup = "55d4c0a3e000-7ffd8a5f3000 ---p 00000000 00:00 0                          [rollup]\nRss:              209224 kB\nPss:              201337 kB\nPss_Anon:         190000 kB\n";
        assert_eq!(parse_pss_kb(rollup), Some(201337));
        assert_eq!(parse_pss_kb("Rss: 5 kB\n"), None);
    }

    #[test]
    fn measures_this_process() {
        let m = measure(vec![], vec![]);
        assert!(m.launcher > 0 && m.launcher_processes >= 1);
        assert!(m.system_total >= m.system_used && m.system_total > 0);
    }
}

//! How much RAM the launcher, each running game and each running server use (`app:memory`), for the instance page,
//! the server page and the optional Home card. A process counts with everything it started (a wrapper's Java,
//! WebView2's helpers); games and servers are left out of the launcher's own share even though it started them.

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

/// Sums memory over process trees. `procs`: pid → (parent, resident bytes).
fn sum_tree(root: u32, procs: &HashMap<u32, (Option<u32>, u64)>, children: &HashMap<u32, Vec<u32>>, skip: &HashSet<u32>) -> (u64, usize) {
    let pids = tree(root, children, skip);
    let bytes = pids.iter().filter_map(|p| procs.get(p).map(|(_, b)| *b)).sum();
    (bytes, pids.iter().filter(|p| procs.contains_key(p)).count())
}

fn measure(games: Vec<crate::game::launch::GameProcess>, servers: Vec<(String, u32, Option<u32>)>) -> MemoryUsage {
    let mut sys = System::new_with_specifics(RefreshKind::nothing());
    sys.refresh_memory();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory());
    let procs: HashMap<u32, (Option<u32>, u64)> = sys.processes().iter().map(|(pid, p)| (pid.as_u32(), (p.parent().map(Pid::as_u32), p.memory()))).collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, (parent, _)) in &procs {
        if let Some(parent) = parent {
            children.entry(*parent).or_default().push(*pid);
        }
    }
    let roots: HashSet<u32> = games.iter().map(|g| g.pid).chain(servers.iter().map(|s| s.1)).collect();
    let me = std::process::id();
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
    fn measures_this_process() {
        let m = measure(vec![], vec![]);
        assert!(m.launcher > 0 && m.launcher_processes >= 1);
        assert!(m.system_total >= m.system_used && m.system_total > 0);
    }
}

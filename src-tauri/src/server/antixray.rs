//! Paper's built-in anti-xray (Paper and the servers built on it: Purpur, Folia, Leaf), on or off from the server's
//! Settings. Since 1.19 it lives in config/paper-world-defaults.yml (`anticheat.anti-xray`), before that in paper.yml
//! (`world-settings.default.anti-xray`). Only the `enabled` and `engine-mode` lines are changed; the rest of the file,
//! comments included, stays as Paper wrote it. A file Paper hasn't made yet (the server never started) is started
//! with just these keys, and Paper fills in the rest on the next start.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::core::types::{ServerKind, ServerMeta};

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AntiXray {
    pub enabled: bool,
    /// 1: ores hidden as stone (light). 2: fake ores everywhere (stronger, more CPU and bandwidth).
    pub mode: u8,
}

pub fn supported(kind: ServerKind) -> bool {
    matches!(kind, ServerKind::Paper | ServerKind::Purpur | ServerKind::Folia | ServerKind::Leaf)
}

/// 1.19 and later: true. Snapshots and unknown versions count as new.
fn new_layout(mc: &str) -> bool {
    let mut parts = mc.split('.').map(|p| p.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(1), Some(minor)) => minor >= 19,
        _ => true,
    }
}

fn place(dir: &Path, mc: &str) -> (PathBuf, &'static [&'static str]) {
    if new_layout(mc) {
        (dir.join("config").join("paper-world-defaults.yml"), &["anticheat", "anti-xray"])
    } else {
        (dir.join("paper.yml"), &["world-settings", "default", "anti-xray"])
    }
}

pub async fn get(meta: &ServerMeta, dir: &Path) -> AntiXray {
    let (file, at) = place(dir, &meta.mc_version);
    let text = tokio::fs::read_to_string(file).await.unwrap_or_default();
    let key = |k: &str| get_value(&text, &[at, &[k]].concat());
    AntiXray { enabled: key("enabled").as_deref() == Some("true"), mode: key("engine-mode").and_then(|m| m.parse().ok()).unwrap_or(1) }
}

pub async fn set(meta: &ServerMeta, dir: &Path, want: &AntiXray) -> Result<()> {
    if !supported(meta.kind) {
        anyhow::bail!("{} has no built-in anti-xray; it comes with Paper and the servers built on it.", meta.kind.label());
    }
    let (file, at) = place(dir, &meta.mc_version);
    let mut text = tokio::fs::read_to_string(&file).await.unwrap_or_default();
    text = set_value(&text, &[at, &["enabled"]].concat(), if want.enabled { "true" } else { "false" });
    text = set_value(&text, &[at, &["engine-mode"]].concat(), &want.mode.clamp(1, 2).to_string());
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&file, text).await?;
    Ok(())
}

// ---------- A little YAML: nested keys with scalar values, by indentation ----------

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_content(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && !t.starts_with('#')
}

/// `key: value` or `key:` (also quoted keys) at the start of a trimmed line.
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let (k, _) = t.split_once(':')?;
    let k = k.trim().trim_matches(|c| c == '"' || c == '\'');
    (!k.is_empty() && !k.starts_with('-')).then_some(k)
}

/// The line holding `path` (a chain of nested keys), and where the block it would go into ends.
enum Found {
    At(usize),
    /// Missing from `depth` on: insert at `line`, indented `indent`.
    Missing { depth: usize, line: usize, indent: usize },
}

fn find(lines: &[&str], path: &[&str]) -> Found {
    let (mut start, mut end, mut parent): (usize, usize, Option<usize>) = (0, lines.len(), None);
    for (depth, key) in path.iter().enumerate() {
        // The indent of this block's children: that of its first content line (or two more than the parent).
        let child = lines[start..end].iter().find(|l| is_content(l)).map(|l| indent_of(l)).filter(|i| parent.map_or(true, |p| *i > p));
        let child = child.unwrap_or_else(|| parent.map_or(0, |p| p + 2));
        let hit = (start..end).find(|&i| is_content(lines[i]) && indent_of(lines[i]) == child && key_of(lines[i]) == Some(key));
        match hit {
            Some(i) if depth == path.len() - 1 => return Found::At(i),
            Some(i) => {
                let block_end = (i + 1..end).find(|&j| is_content(lines[j]) && indent_of(lines[j]) <= child).unwrap_or(end);
                start = i + 1;
                end = block_end;
                parent = Some(child);
            }
            None => {
                // After the block's last content line, so it doesn't land after the next block's comments.
                let line = (start..end).rev().find(|&i| is_content(lines[i])).map_or(start, |i| i + 1);
                return Found::Missing { depth, line, indent: child };
            }
        }
    }
    unreachable!("path is never empty")
}

pub(crate) fn get_value(text: &str, path: &[&str]) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    match find(&lines, path) {
        Found::At(i) => {
            let (_, v) = lines[i].split_once(':')?;
            let v = v.split(" #").next().unwrap_or("").trim().trim_matches(|c| c == '"' || c == '\'');
            Some(v.to_string())
        }
        Found::Missing { .. } => None,
    }
}

pub(crate) fn set_value(text: &str, path: &[&str], value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let borrowed: Vec<&str> = lines.iter().map(String::as_str).collect();
    match find(&borrowed, path) {
        Found::At(i) => {
            let indent = indent_of(&lines[i]);
            let key = path[path.len() - 1];
            lines[i] = format!("{}{key}: {value}", " ".repeat(indent));
        }
        Found::Missing { depth, line, indent } => {
            let mut add = Vec::new();
            for (n, key) in path[depth..].iter().enumerate() {
                let pad = " ".repeat(indent + 2 * n);
                add.push(if depth + n == path.len() - 1 { format!("{pad}{key}: {value}") } else { format!("{pad}{key}:") });
            }
            lines.splice(line..line, add);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAPER: &str = "# This is the world defaults configuration file for Paper.\n_version: 30\nanticheat:\n  anti-xray:\n    enabled: false\n    engine-mode: 1\n    hidden-blocks:\n    - copper_ore\n    - diamond_ore\n    lava-obscures: false\n  obfuscation:\n    items:\n      hide-durability: false\nchunks:\n  auto-save-interval: default\n";

    #[test]
    fn reads_and_changes_only_the_two_lines() {
        let path = ["anticheat", "anti-xray", "enabled"];
        assert_eq!(get_value(PAPER, &path).as_deref(), Some("false"));
        let on = set_value(PAPER, &path, "true");
        let on = set_value(&on, &["anticheat", "anti-xray", "engine-mode"], "2");
        assert_eq!(get_value(&on, &path).as_deref(), Some("true"));
        assert_eq!(get_value(&on, &["anticheat", "anti-xray", "engine-mode"]).as_deref(), Some("2"));
        // Everything else is untouched.
        assert_eq!(on, PAPER.replace("    enabled: false\n    engine-mode: 1", "    enabled: true\n    engine-mode: 2"));
    }

    #[test]
    fn starts_a_missing_file_or_block() {
        let fresh = set_value("", &["anticheat", "anti-xray", "enabled"], "true");
        assert_eq!(fresh, "anticheat:\n  anti-xray:\n    enabled: true\n");
        let partial = set_value("anticheat:\n  obfuscation:\n    items: {}\nchunks:\n  x: 1\n", &["anticheat", "anti-xray", "enabled"], "true");
        assert_eq!(partial, "anticheat:\n  obfuscation:\n    items: {}\n  anti-xray:\n    enabled: true\nchunks:\n  x: 1\n");
        assert_eq!(get_value(&partial, &["chunks", "x"]).as_deref(), Some("1"));
    }

    #[test]
    fn old_paper_yml_layout() {
        let old = "verbose: false\nworld-settings:\n  default:\n    anti-xray:\n      enabled: false\n      engine-mode: 1\n";
        let on = set_value(old, &["world-settings", "default", "anti-xray", "enabled"], "true");
        assert_eq!(get_value(&on, &["world-settings", "default", "anti-xray", "enabled"]).as_deref(), Some("true"));
        assert!(new_layout("1.21.1") && new_layout("26.2") && !new_layout("1.18.2"));
    }
}

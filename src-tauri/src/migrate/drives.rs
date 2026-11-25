//! Other launchers kept outside their default places: every fixed drive (Windows) or mounted disk (Linux,
//! macOS) is checked for the folders players usually put them in (`E:\PrismLauncher`, `D:\Games\CurseForge`,
//! `F:\.minecraft`, an old Windows install's `Users\<name>\AppData\Roaming\…`). It's a fixed list of likely
//! paths per drive, never a crawl, and each drive gets a time limit so a sleeping disk can't hold the list up.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::formats::Launcher;

/// Folders on a drive that often hold launchers ("" is the drive itself).
const PARENTS: [&str; 10] =
    ["", "Games", "Game", "Program Files", "Program Files (x86)", "Minecraft", "Launchers", "Apps", "Portable", "PortableApps"];

/// Launcher data folders, inside each of `PARENTS` (portable installs keep their data next to the program).
const FOLDERS: [&str; 18] = [
    "PrismLauncher",
    "Prism Launcher",
    "PolyMC",
    "MultiMC",
    "ModrinthApp",
    "Modrinth App",
    "CurseForge",
    "CurseForge/minecraft",
    "curseforge/minecraft",
    "Curse/Minecraft",
    "ATLauncher",
    "GDLauncher",
    "gdlauncher_next",
    "gdlauncher_carbon/data",
    ".minecraft",
    "minecraft",
    "Minecraft",
    "Minecraft Launcher",
];

/// Inside each user folder of a Windows install on another drive (`X:\Users\<name>`): the default places.
const USER_FOLDERS: [&str; 14] = [
    "AppData/Roaming/PrismLauncher",
    "AppData/Roaming/PolyMC",
    "AppData/Roaming/MultiMC",
    "AppData/Roaming/ModrinthApp",
    "AppData/Roaming/com.modrinth.theseus",
    "AppData/Roaming/ATLauncher",
    "AppData/Roaming/gdlauncher_next",
    "AppData/Roaming/gdlauncher_carbon/data",
    "AppData/Roaming/.minecraft",
    "curseforge/minecraft",
    "Documents/Curse/Minecraft",
    "MultiMC",
    "ATLauncher",
    "Desktop/MultiMC",
];

/// Windows' own accounts, never a player's.
const NOT_USERS: [&str; 5] = ["default", "default user", "public", "all users", "defaultapppool"];

/// Every place on `drive` worth a look, without touching the disk. On Windows (`case_blind`) names that differ
/// only in case are the same folder and listed once.
pub fn candidates(drive: &Path, case_blind: bool) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for parent in PARENTS {
        for folder in FOLDERS {
            let rel = if parent.is_empty() { folder.to_string() } else { format!("{parent}/{folder}") };
            if seen.insert(if case_blind { rel.to_lowercase() } else { rel.clone() }) {
                out.push(join(drive, &rel));
            }
        }
    }
    out
}

/// `drive` + a relative path written with `/`.
fn join(drive: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(drive.to_path_buf(), |p, part| p.join(part))
}

/// The default places inside each user folder of a Windows install on `drive` (one listing of `Users`).
pub fn user_candidates(drive: &Path) -> Vec<PathBuf> {
    let users = drive.join("Users");
    let Ok(entries) = std::fs::read_dir(&users) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !NOT_USERS.contains(&n.to_lowercase().as_str()))
        .collect();
    names.sort();
    names.iter().flat_map(|n| USER_FOLDERS.iter().map(|rel| join(&users.join(n), rel)).collect::<Vec<_>>()).collect()
}

/// What `drive` holds: each launcher data folder found in the usual places, and loose `.minecraft` folders
/// (no launcher, but mods or worlds). Blocking; run with a time limit.
pub fn probe(drive: &Path, with_users: bool, root_kind: impl Fn(&Path) -> Option<Launcher>) -> Vec<(Launcher, PathBuf)> {
    let mut places = candidates(drive, cfg!(windows));
    if with_users {
        places.extend(user_candidates(drive));
    }
    places
        .into_iter()
        .filter(|p| p.is_dir())
        .filter_map(|p| match root_kind(&p) {
            Some(l) => Some((l, p)),
            None if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(".minecraft")) => Some((Launcher::Folder, p)),
            None => None,
        })
        .collect()
}

/// Paths that are the same folder reached twice (a drive letter made with `subst`, a symlink, different
/// case on Windows) are kept once, the first time. `key` is what identifies a folder (its canonical path).
pub fn dedupe<T>(items: Vec<(T, PathBuf)>, seen: &mut HashSet<PathBuf>, key: impl Fn(&Path) -> PathBuf) -> Vec<(T, PathBuf)> {
    items.into_iter().filter(|(_, p)| seen.insert(key(p))).collect()
}

/// A folder's identity: its real path, so two ways to the same folder match.
pub fn identity(p: &Path) -> PathBuf {
    let real = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if cfg!(windows) {
        PathBuf::from(real.to_string_lossy().to_lowercase())
    } else {
        real
    }
}

/// What the player sees for a drive: "E:" on Windows, the mount point elsewhere.
pub fn label(drive: &Path) -> String {
    let s = drive.to_string_lossy();
    if cfg!(windows) {
        s.trim_end_matches(['\\', '/']).to_string()
    } else {
        s.into_owned()
    }
}

/// The drive a path is on, as `label` shows it (Windows only; elsewhere the default places have no drive).
pub fn drive_of(p: &Path) -> Option<String> {
    use std::path::{Component, Prefix};
    match p.components().next()? {
        Component::Prefix(pre) => match pre.kind() {
            Prefix::Disk(l) | Prefix::VerbatimDisk(l) => Some(format!("{}:", (l as char).to_ascii_uppercase())),
            _ => None,
        },
        _ => None,
    }
}

/// Fixed drives with a letter (removable, network and CD drives are left out).
#[cfg(windows)]
pub fn list() -> Vec<PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    const FIXED: u32 = 3;
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| PathBuf::from(format!("{}:\\", (b'A' + i) as char)))
        .filter(|root| {
            let wide: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) == FIXED }
        })
        .collect()
}

/// Mounted disks: `/mnt/*`, `/media/*`, `/media/<user>/*`, `/run/media/<user>/*` and `/Volumes/*`.
#[cfg(not(windows))]
pub fn list() -> Vec<PathBuf> {
    let mut out = mounts_under(Path::new("/mnt"), 1);
    out.extend(mounts_under(Path::new("/media"), 2));
    out.extend(mounts_under(Path::new("/run/media"), 2).into_iter().filter(|p| p.parent() != Some(Path::new("/run/media"))));
    out.extend(mounts_under(Path::new("/Volumes"), 1));
    out
}

/// Folders up to `depth` levels below `base` (a mount point's own folder), sorted. Only the listing's
/// file types are read, never the mounted disk itself, so a dead network mount can't block here.
pub fn mounts_under(base: &Path, depth: u8) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if depth == 0 {
        return out;
    }
    let Ok(entries) = std::fs::read_dir(base) else {
        return out;
    };
    let mut dirs: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        out.extend(mounts_under(&d, depth - 1));
        out.push(d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::formats::tests::Fixture;
    use super::*;

    #[test]
    fn likely_places_per_drive() {
        let drive = Path::new("X:/");
        let list = candidates(drive, true);
        for want in ["PrismLauncher", "MultiMC", ".minecraft", "ATLauncher", "Games/CurseForge", "CurseForge/minecraft", "Program Files/PrismLauncher", "Games/MultiMC"] {
            assert!(list.contains(&join(drive, want)), "{want} is checked");
        }
        assert!(!list.contains(&join(drive, "curseforge/minecraft")), "the lowercase twin is the same folder on Windows");
        let lower: Vec<String> = list.iter().map(|p| p.to_string_lossy().to_lowercase()).collect();
        assert_eq!(lower.len(), lower.iter().collect::<HashSet<_>>().len(), "on Windows, case-only twins are listed once");
        let exact = candidates(drive, false);
        assert!(exact.len() > list.len(), "elsewhere case matters, so both spellings are tried");
        assert!(exact.contains(&join(drive, "curseforge/minecraft")) && exact.contains(&join(drive, "CurseForge/minecraft")));
        assert!(list.len() < 200, "a short, fixed list, not a crawl");
        assert!(list.iter().all(|p| p.starts_with(drive)));
    }

    #[test]
    fn finds_launchers_on_a_drive() {
        let fx = Fixture::new();
        fx.file("Games/CurseForge/Instances/ATM/minecraftinstance.json", "{}")
            .file("PrismLauncher/prismlauncher.cfg", "")
            .file(".minecraft/saves/World/level.dat", "")
            .file("Users/Sam/AppData/Roaming/ATLauncher/configs/ATLauncher.json", "{}")
            .file("Users/Public/AppData/Roaming/MultiMC/multimc.cfg", "")
            .file("Games/Something/readme.txt", "");
        let kind = |p: &Path| {
            if p.join("prismlauncher.cfg").is_file() {
                Some(Launcher::Prism)
            } else if p.join("Instances").is_dir() {
                Some(Launcher::Curseforge)
            } else if p.join("configs/ATLauncher.json").is_file() {
                Some(Launcher::Atlauncher)
            } else if p.join("multimc.cfg").is_file() {
                Some(Launcher::Multimc)
            } else {
                None
            }
        };
        let mut found = probe(&fx.0, false, kind);
        found.sort_by(|a, b| a.1.cmp(&b.1));
        let rel: Vec<(Launcher, PathBuf)> = found.iter().map(|(l, p)| (*l, p.strip_prefix(&fx.0).unwrap().to_path_buf())).collect();
        assert!(rel.contains(&(Launcher::Prism, PathBuf::from("PrismLauncher"))));
        assert!(rel.contains(&(Launcher::Curseforge, join(Path::new("Games"), "CurseForge"))));
        assert!(rel.contains(&(Launcher::Folder, PathBuf::from(".minecraft"))), "a loose .minecraft is offered as a folder");
        assert!(!rel.iter().any(|(_, p)| p.starts_with("Users")), "user folders only when asked");
        let with_users = probe(&fx.0, true, kind);
        assert!(with_users.iter().any(|(l, p)| *l == Launcher::Atlauncher && p.ends_with(join(Path::new("Sam"), "AppData/Roaming/ATLauncher"))));
        assert!(!with_users.iter().any(|(_, p)| p.to_string_lossy().contains("Public")), "Windows' own accounts are skipped");
    }

    #[test]
    fn same_folder_twice_is_listed_once() {
        let fx = Fixture::new();
        fx.file("Prism/prismlauncher.cfg", "");
        let a = fx.path("Prism");
        let b = fx.path("Prism").join(".");
        let c = fx.path("Other");
        let mut seen = HashSet::new();
        let first = dedupe(vec![(1, a.clone()), (2, b), (3, c.clone())], &mut seen, identity);
        assert_eq!(first.iter().map(|(n, _)| *n).collect::<Vec<_>>(), vec![1, 3], "`Prism` and `Prism/.` are the same folder");
        let again = dedupe(vec![(4, a), (5, c)], &mut seen, identity);
        assert!(again.is_empty(), "folders already listed (from the default places) aren't listed again");
    }

    #[test]
    fn case_only_twins_match_on_windows() {
        if !cfg!(windows) {
            return;
        }
        let fx = Fixture::new();
        fx.file("CurseForge/minecraft/Instances/A/minecraftinstance.json", "{}");
        let mut seen = HashSet::new();
        let kept = dedupe(vec![((), fx.path("CurseForge/minecraft")), ((), fx.path("curseforge/Minecraft"))], &mut seen, identity);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn drive_names() {
        if cfg!(windows) {
            assert_eq!(label(Path::new("E:\\")), "E:");
            assert_eq!(drive_of(Path::new("e:\\Games\\PrismLauncher")).as_deref(), Some("E:"));
            assert_eq!(drive_of(Path::new("\\\\?\\D:\\x")).as_deref(), Some("D:"));
        } else {
            assert_eq!(label(Path::new("/mnt/data")), "/mnt/data");
            assert_eq!(drive_of(Path::new("/home/sam/.minecraft")), None);
        }
    }

    #[test]
    fn mount_points_two_levels_down() {
        let fx = Fixture::new();
        fx.file("sam/Disk A/x.txt", "").file("Disk B/y.txt", "");
        let found = mounts_under(&fx.0, 2);
        assert!(found.contains(&fx.path("sam/Disk A")) && found.contains(&fx.path("Disk B")) && found.contains(&fx.path("sam")));
        assert!(!found.iter().any(|p| p.ends_with("x.txt")));
        assert_eq!(mounts_under(&fx.path("missing"), 2), Vec::<PathBuf>::new());
    }
}

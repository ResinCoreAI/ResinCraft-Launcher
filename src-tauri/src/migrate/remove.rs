//! "Remove the originals after moving": takes an imported instance out of the launcher it came from, the way that
//! launcher would remove it itself. Only for launchers whose instance list is just their folders, only after a
//! clean copy, never while the launcher runs or a file is in use, and always to the Recycle Bin (never deleted).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::copy::Skipped;
use super::formats::{Found, Launcher};
use crate::core::text::Text;
use crate::tr;

/// What removing one original means.
#[derive(Debug, PartialEq)]
pub struct Plan {
    /// The instance folder, moved to the Recycle Bin.
    pub folder: PathBuf,
    /// Prism/MultiMC's group list, which names the folder (the entry is taken out).
    pub groups_file: Option<PathBuf>,
}

/// Whether this item's launcher can lose it cleanly; the reason when it can't.
pub fn plan_for(f: &Found) -> Result<Plan, Text> {
    if f.server {
        return Err(tr!("migrate.remove.serverKept", "Server folders stay; delete the old one yourself once the copy works"));
    }
    match f.launcher {
        Launcher::Prism | Launcher::Multimc => {
            let groups = f.path.parent().map(|d| d.join("instgroups.json")).filter(|p| p.is_file());
            Ok(Plan { folder: f.path.clone(), groups_file: groups })
        }
        Launcher::Atlauncher | Launcher::Curseforge => Ok(Plan { folder: f.path.clone(), groups_file: None }),
        // The classic app lists its instances folder; GDLauncher Carbon also keeps them in its database.
        Launcher::Gdlauncher if f.game_dir == f.path => Ok(Plan { folder: f.path.clone(), groups_file: None }),
        Launcher::Gdlauncher | Launcher::Modrinth => {
            Err(tr!("migrate.remove.database", "{launcher} keeps its instances in a database, so remove it there", launcher = f.launcher.label()))
        }
        Launcher::Official => Err(tr!("migrate.remove.shared", "The Minecraft Launcher's folder is shared by all its profiles, so it's never removed")),
        Launcher::Folder => Err(tr!("migrate.remove.folder", "Folders picked by hand stay where they are")),
    }
}

/// Only a copy that finished with nothing left behind, into an instance that exists, lets the original go.
pub fn after_copy(skipped: &[Skipped], copy_exists: bool) -> Result<(), Text> {
    if !skipped.is_empty() {
        return Err(tr!("migrate.remove.skippedFiles", "Some of its files couldn't be copied"));
    }
    if !copy_exists {
        return Err(tr!("migrate.remove.noCopy", "The copy isn't there"));
    }
    Ok(())
}

/// Process names (lowercase, without .exe) of each launcher.
fn process_names(l: Launcher) -> &'static [&'static str] {
    match l {
        Launcher::Prism => &["prismlauncher", "polymc"],
        Launcher::Multimc => &["multimc"],
        Launcher::Atlauncher => &["atlauncher"],
        Launcher::Gdlauncher => &["gdlauncher"],
        Launcher::Curseforge => &["curseforge"],
        _ => &[],
    }
}

/// Whether the launcher is open right now (ATLauncher may run as `java -jar ATLauncher.jar`).
pub fn launcher_running(l: Launcher) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let names = process_names(l);
    if names.is_empty() {
        return false;
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet));
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        names.contains(&name) || (l == Launcher::Atlauncher && p.cmd().iter().any(|a| a.to_string_lossy().to_lowercase().ends_with("atlauncher.jar")))
    })
}

/// Fails when something inside `folder` is in use: Windows refuses to rename a folder with an open file in it
/// (a running game, the launcher). Renamed straight back.
fn check_not_in_use(folder: &Path) -> std::io::Result<()> {
    let probe = PathBuf::from(format!("{}.resincraft-check", folder.display()));
    std::fs::rename(folder, &probe)?;
    std::fs::rename(&probe, folder)
}

/// Takes the folder's entry out of Prism/MultiMC's instgroups.json (`groups.<name>.instances`).
pub fn drop_from_groups(file: &Path, folder_name: &str) -> std::io::Result<()> {
    let text = std::fs::read_to_string(file)?;
    let Ok(mut j) = serde_json::from_str::<Value>(&text) else { return Ok(()) };
    let mut changed = false;
    if let Some(groups) = j.get_mut("groups").and_then(Value::as_object_mut) {
        for g in groups.values_mut() {
            if let Some(list) = g.get_mut("instances").and_then(Value::as_array_mut) {
                let before = list.len();
                list.retain(|v| v.as_str() != Some(folder_name));
                changed |= list.len() != before;
            }
        }
    }
    if changed {
        std::fs::write(file, serde_json::to_string_pretty(&j).unwrap_or(text))?;
    }
    Ok(())
}

/// Removes one original: checks the launcher is closed and nothing is in use, moves the folder away with `trash`,
/// then tidies the launcher's group list. On any problem nothing is removed and the reason is returned.
pub fn remove(f: &Found, plan: &Plan, running: impl Fn(Launcher) -> bool, trash: impl Fn(&Path) -> anyhow::Result<()>) -> Result<(), Text> {
    let label = f.launcher.label();
    if running(f.launcher) {
        return Err(tr!("migrate.remove.running", "Close {launcher} and try again", launcher = label));
    }
    if !plan.folder.is_dir() {
        return Err(tr!("migrate.remove.gone", "Its folder was already gone"));
    }
    if check_not_in_use(&plan.folder).is_err() {
        return Err(tr!("migrate.remove.inUse", "A file in it is in use. Close {launcher} and the game, then try again", launcher = label));
    }
    trash(&plan.folder).map_err(|e| tr!("migrate.remove.failed", "Couldn't move it to the Recycle Bin: {error}", error = e.to_string()))?;
    if let (Some(groups), Some(name)) = (&plan.groups_file, plan.folder.file_name()) {
        let _ = drop_from_groups(groups, &name.to_string_lossy());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::formats::{self, tests::Fixture};
    use super::super::server_dir;
    use super::*;

    fn delete(p: &Path) -> anyhow::Result<()> {
        Ok(std::fs::remove_dir_all(p)?)
    }

    #[test]
    fn which_launchers_can_lose_an_instance() {
        let fx = Fixture::new();
        fx.file("prism/instances/A/instance.cfg", "name=A\nIntendedVersion=1.20.1\n")
            .file("prism/instances/instgroups.json", r#"{"formatVersion":"1","groups":{"Fun":{"hidden":false,"instances":["A","B"]}}}"#)
            .file("cf/Instances/B/minecraftinstance.json", r#"{"name":"B","gameVersion":"1.20.1"}"#)
            .file("atl/instances/C/instance.json", r#"{"id":"1.20.1","launcher":{"name":"C"}}"#)
            .file("gd/instances/D/config.json", r#"{"loader":{"loaderType":"vanilla","mcVersion":"1.19"}}"#)
            .file("carbon/instances/E/instance.json", r#"{"name":"E","game_configuration":{"version":{"Standard":{"release":"1.20.4"}}}}"#)
            .file("mr/profiles/F/profile.json", r#"{"metadata":{"name":"F","game_version":"1.20"}}"#)
            .file("mc/launcher_profiles.json", r#"{"profiles":{"x":{"type":"custom","name":"G","lastVersionId":"1.20.1"}}}"#)
            .file("srv/server.properties", "");
        let prism = formats::mmc_instance(&fx.path("prism/instances/A"), None, Launcher::Prism).unwrap();
        assert_eq!(plan_for(&prism), Ok(Plan { folder: fx.path("prism/instances/A"), groups_file: Some(fx.path("prism/instances/instgroups.json")) }));
        assert!(plan_for(&formats::curseforge_instance(&fx.path("cf/Instances/B")).unwrap()).is_ok());
        assert!(plan_for(&formats::atlauncher_instance(&fx.path("atl/instances/C")).unwrap()).is_ok());
        assert!(plan_for(&formats::gdlauncher_instance(&fx.path("gd/instances/D")).unwrap()).is_ok());
        let carbon = plan_for(&formats::gdlauncher_instance(&fx.path("carbon/instances/E")).unwrap()).unwrap_err();
        assert_eq!(carbon.key, "migrate.remove.database");
        assert_eq!(plan_for(&formats::modrinth_profile_json(&fx.path("mr/profiles/F")).unwrap()).unwrap_err().key, "migrate.remove.database");
        assert_eq!(plan_for(&formats::official_profiles(&fx.path("mc"))[0]).unwrap_err().key, "migrate.remove.shared");
        assert_eq!(plan_for(&server_dir::server_folder(&fx.path("srv"), Launcher::Folder).unwrap()).unwrap_err().key, "migrate.remove.serverKept");
        let atl_server = server_dir::server_folder(&fx.path("srv"), Launcher::Atlauncher).unwrap();
        assert!(plan_for(&atl_server).is_err(), "servers are kept whatever launcher they came from");
    }

    #[test]
    fn only_after_a_clean_copy() {
        let skipped = vec![Skipped { path: "saves/w/session.lock".into(), reason: Text::plain("locked") }];
        assert_eq!(after_copy(&skipped, true).unwrap_err().key, "migrate.remove.skippedFiles");
        assert_eq!(after_copy(&[], false).unwrap_err().key, "migrate.remove.noCopy");
        assert!(after_copy(&[], true).is_ok());
    }

    #[test]
    fn removes_the_folder_and_its_group_entry() {
        let fx = Fixture::new();
        fx.file("instances/A/instance.cfg", "name=A\nIntendedVersion=1.20.1\n")
            .file("instances/A/.minecraft/saves/w/level.dat", "x")
            .file("instances/B/instance.cfg", "name=B\n")
            .file("instances/instgroups.json", r#"{"formatVersion":"1","groups":{"Fun":{"hidden":false,"instances":["A","B"]}}}"#);
        let f = formats::mmc_instance(&fx.path("instances/A"), None, Launcher::Prism).unwrap();
        let plan = plan_for(&f).unwrap();
        // The launcher is open: nothing happens.
        assert_eq!(remove(&f, &plan, |_| true, delete).unwrap_err().key, "migrate.remove.running");
        assert!(fx.path("instances/A").is_dir());
        // Trash fails: the original stays, and so does its group entry.
        assert_eq!(remove(&f, &plan, |_| false, |_| anyhow::bail!("no Recycle Bin")).unwrap_err().key, "migrate.remove.failed");
        assert!(std::fs::read_to_string(fx.path("instances/instgroups.json")).unwrap().contains("\"A\""));
        remove(&f, &plan, |_| false, delete).unwrap();
        assert!(!fx.path("instances/A").exists() && fx.path("instances/B").is_dir());
        let groups: Value = serde_json::from_str(&std::fs::read_to_string(fx.path("instances/instgroups.json")).unwrap()).unwrap();
        assert_eq!(groups.pointer("/groups/Fun/instances").unwrap(), &serde_json::json!(["B"]));
        assert_eq!(remove(&f, &plan, |_| false, delete).unwrap_err().key, "migrate.remove.gone");
    }

    #[cfg(windows)]
    #[test]
    fn a_file_in_use_keeps_the_original() {
        let fx = Fixture::new();
        fx.file("instances/A/instance.cfg", "name=A\nIntendedVersion=1.20.1\n").file("instances/A/.minecraft/mods/m.jar", "x");
        let f = formats::mmc_instance(&fx.path("instances/A"), None, Launcher::Prism).unwrap();
        let plan = plan_for(&f).unwrap();
        let open = std::fs::File::open(fx.path("instances/A/.minecraft/mods/m.jar")).unwrap();
        assert_eq!(remove(&f, &plan, |_| false, delete).unwrap_err().key, "migrate.remove.inUse");
        drop(open);
        assert!(fx.path("instances/A/.minecraft/mods/m.jar").is_file());
    }
}

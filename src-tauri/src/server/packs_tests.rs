//! Tests for packs.rs and the server side of packswitch.rs: what a server keeps, undoing a switch that also replaced
//! the server software, and which files a sync takes from an instance.

use std::io::Write as _;

use super::*;
use crate::content::packswitch::{apply, restore, snapshot_meta, work_dir, ALL_TOP_FILES};

struct Dirs {
    root: PathBuf,
}

impl Dirs {
    fn new(name: &str) -> Dirs {
        let root = std::env::temp_dir().join(format!("resin-serverpacks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("server")).unwrap();
        std::fs::create_dir_all(root.join("game")).unwrap();
        Dirs { root }
    }
    fn server(&self) -> PathBuf {
        self.root.join("server")
    }
    fn game(&self) -> PathBuf {
        self.root.join("game")
    }
    fn stage(&self) -> PathBuf {
        work_dir(&self.server()).join("stage")
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn put(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn sha1(text: &str) -> Option<Hash> {
    Some(Hash::sha1(packswitch::hash_reader(text.as_bytes(), HashAlgo::Sha1).unwrap()))
}

fn tree(dir: &Path) -> BTreeMap<String, Option<String>> {
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<String>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = norm(&p.strip_prefix(root).unwrap().to_string_lossy());
            if p.is_dir() {
                out.insert(rel + "/", None);
                go(root, &p, out);
            } else {
                out.insert(rel, Some(std::fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = BTreeMap::new();
    go(dir, dir, &mut out);
    out
}

#[test]
fn server_keeps_worlds_lists_and_software() {
    let d = Dirs::new("keep");
    let s = d.server();
    put(&s, "server.properties", "level-name=Survival\nmotd=mine");
    put(&s, "Survival/level.dat", "my world");
    put(&s, "whitelist.json", "[mine]");
    put(&s, "ops.json", "[me]");
    put(&s, "forge-1.20.1-installer.jar", "installer");
    put(&s, "run.bat", "java ...");
    put(&s, "libraries/net/x.jar", "lib");
    put(&s, "config/a.toml", "a-v1");
    put(&s, "mods/old.jar", "old");
    put(&s, "mods/hosts-own.jar", "own");
    for (p, t) in [("server.properties", "pack props"), ("Survival/region/r.mca", "pack"), ("whitelist.json", "[]"), ("config/a.toml", "a-v2"), ("mods/new.jar", "new")] {
        put(&d.stage(), p, t);
    }
    let old: BTreeMap<String, Option<Hash>> =
        [("config/a.toml", "a-v1"), ("mods/old.jar", "old"), ("run.bat", "java ..."), ("ops.json", "[me]")].iter().map(|(p, t)| (p.to_string(), sha1(t))).collect();
    let new: BTreeMap<String, NewEntry> =
        ["server.properties", "Survival/region/r.mca", "whitelist.json", "config/a.toml", "mods/new.jar"].iter().map(|p| (p.to_string(), NewEntry::Staged)).collect();
    let keep = Keep::server("Survival");
    let p = plan(&s, &d.stage(), Some(&old), &new, &HashMap::new(), &keep);
    assert_eq!(
        p.steps,
        vec![
            Step::Place { path: "config/a.toml".into(), to: "config/a.toml".into(), bak: false },
            Step::Place { path: "mods/new.jar".into(), to: "mods/new.jar".into(), bak: false },
            Step::Remove { path: "mods/old.jar".into() },
        ]
    );
    // What a pack never provides on a server is left out of a version, whatever the archive holds.
    assert!(keep.excluded("libraries/net/x.jar"));
    assert!(keep.excluded("forge-1.20.1-installer.jar"));
    assert!(keep.excluded("resourcepacks/x.zip"));
    assert!(!keep.excluded("config/a.toml"));
    assert!(!keep.excluded("mods/new.jar"));
}

#[test]
fn undo_puts_back_the_server_software_too() {
    let d = Dirs::new("software");
    let s = d.server();
    put(&s, "server.json", "{\"mc\":\"1.20.1\"}");
    put(&s, "server.jar", "old jar");
    put(&s, "config/a.toml", "a-v1");
    put(&s, "world/level.dat", "world");
    put(&d.stage(), "config/a.toml", "a-v2");
    let before = {
        let mut t = tree(&s);
        t.retain(|k, _| !k.starts_with(".pack-switch"));
        t
    };
    let work = work_dir(&s);
    snapshot_meta(&s, &work, &ALL_TOP_FILES).unwrap();
    apply(&s, &d.stage(), &work, &[Step::Place { path: "config/a.toml".into(), to: "config/a.toml".into(), bak: false }]).unwrap();
    // The loader installer then replaces the jar, adds a script, and the meta is saved.
    put(&s, "server.jar", "new jar");
    put(&s, "run.bat", "new script");
    put(&s, "server.json", "{\"mc\":\"1.21.1\"}");
    restore(&s, &s, &work, &ALL_TOP_FILES).unwrap();
    let _ = std::fs::remove_dir_all(&work);
    assert_eq!(tree(&s), before);
}

#[test]
fn sync_takes_enabled_mods_and_config_folders() {
    let d = Dirs::new("syncset");
    let g = d.game();
    put(&g, "mods/a.jar", "a");
    put(&g, "mods/b.jar.disabled", "b");
    put(&g, "mods/sub/c.jar", "c");
    put(&g, "config/x/y.toml", "y");
    put(&g, "kubejs/server_scripts/z.js", "z");
    put(&g, "options.txt", "opts");
    put(&g, "saves/W/level.dat", "w");
    let set = sync_set(&g);
    assert_eq!(set.keys().cloned().collect::<Vec<_>>(), vec!["config/x/y.toml", "kubejs/server_scripts/z.js", "mods/a.jar"]);
    assert_eq!(set["mods/a.jar"], packswitch::hash_reader("a".as_bytes(), HashAlgo::Sha1).unwrap());
}

fn jar(path: &Path, mod_id: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    z.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
    write!(z, "{{\"schemaVersion\":1,\"id\":\"{mod_id}\",\"version\":\"1\"}}").unwrap();
    z.finish().unwrap();
}

#[test]
fn first_sync_replaces_older_copies_by_mod_id() {
    let d = Dirs::new("older");
    jar(&d.game().join("mods/sodium-2.jar"), "sodium");
    jar(&d.server().join("mods/sodium-1.jar"), "sodium");
    jar(&d.server().join("mods/hosts-own.jar"), "lithium");
    let steps = older_copies(&d.server(), &d.game(), &["mods/sodium-2.jar".to_string()]);
    assert_eq!(steps, vec![Step::Remove { path: "mods/sodium-1.jar".into() }]);
}

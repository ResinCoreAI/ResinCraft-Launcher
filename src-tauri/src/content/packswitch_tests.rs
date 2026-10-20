//! Tests for packswitch.rs: the keep/replace/remove rules on fixture folders, and undoing an applied switch.

use super::*;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!("resin-packswitch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("inst").join("minecraft")).unwrap();
        std::fs::create_dir_all(root.join("inst").join(".pack-switch").join("stage")).unwrap();
        Fixture { root }
    }
    fn inst(&self) -> PathBuf {
        self.root.join("inst")
    }
    fn game(&self) -> PathBuf {
        self.inst().join("minecraft")
    }
    fn work(&self) -> PathBuf {
        work_dir(&self.inst())
    }
    fn stage(&self) -> PathBuf {
        self.work().join("stage")
    }
    fn put(&self, dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    /// A file in the instance as it is now.
    fn has(&self, rel: &str, text: &str) {
        self.put(&self.game(), rel, text);
    }
    /// A file of the new version, staged.
    fn staged(&self, rel: &str, text: &str) {
        self.put(&self.stage(), rel, text);
    }
    fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.game().join(rel)).ok()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn sha1(text: &str) -> Option<Hash> {
    Some(Hash::sha1(hash_reader(text.as_bytes(), HashAlgo::Sha1).unwrap()))
}

fn old(files: &[(&str, &str)]) -> BTreeMap<String, Option<Hash>> {
    files.iter().map(|(p, t)| (p.to_string(), sha1(t))).collect()
}

fn staged(paths: &[&str]) -> BTreeMap<String, NewEntry> {
    paths.iter().map(|p| (p.to_string(), NewEntry::Staged)).collect()
}

fn place(path: &str, to: &str, bak: bool) -> Step {
    Step::Place { path: path.into(), to: to.into(), bak }
}

fn remove(path: &str) -> Step {
    Step::Remove { path: path.into() }
}

#[test]
fn config_rules() {
    let f = Fixture::new("config");
    // Pack didn't change it; the player did: theirs stays.
    f.has("config/a.toml", "a-mine");
    f.staged("config/a.toml", "a-v1");
    // Pack changed it; the player didn't: replaced.
    f.has("config/b.toml", "b-v1");
    f.staged("config/b.toml", "b-v2");
    // Both changed it: the pack's wins, theirs kept as .bak.
    f.has("config/c.toml", "c-mine");
    f.staged("config/c.toml", "c-v2");
    // Gone from the pack: removed if untouched, kept if the player changed it.
    f.has("config/d.toml", "d-v1");
    f.has("config/e.toml", "e-mine");
    // The player's own file, never the pack's.
    f.has("config/mine.toml", "mine");
    let o = old(&[("config/a.toml", "a-v1"), ("config/b.toml", "b-v1"), ("config/c.toml", "c-v1"), ("config/d.toml", "d-v1"), ("config/e.toml", "e-v1")]);
    let n = staged(&["config/a.toml", "config/b.toml", "config/c.toml"]);
    let p = plan(&f.game(), &f.stage(), Some(&o), &n, &HashMap::new(), &Keep::client());
    assert_eq!(p.steps, vec![place("config/b.toml", "config/b.toml", false), place("config/c.toml", "config/c.toml", true), remove("config/d.toml")]);
    assert_eq!(p.kept, vec!["config/a.toml".to_string(), "config/e.toml".to_string()]);
    assert!(p.pending.is_empty());
}

#[test]
fn mod_rules() {
    let f = Fixture::new("mods");
    // Updated under a new name.
    f.has("mods/sodium-1.jar", "sodium1");
    f.staged("mods/sodium-2.jar", "sodium2");
    // Switched off by the player, updated under a new name: stays off.
    f.has("mods/jei-1.jar.disabled", "jei1");
    f.staged("mods/jei-2.jar", "jei2");
    // Switched off, same name, new content: the new one is placed switched off.
    f.has("mods/same.jar.disabled", "same1");
    f.staged("mods/same.jar", "same2");
    // Already as the new version has it.
    f.has("mods/lib.jar", "lib");
    f.staged("mods/lib.jar", "lib");
    // Added by the player.
    f.has("mods/extra.jar", "extra");
    let o = old(&[("mods/sodium-1.jar", "sodium1"), ("mods/jei-1.jar", "jei1"), ("mods/same.jar", "same1"), ("mods/lib.jar", "lib")]);
    let n = staged(&["mods/sodium-2.jar", "mods/jei-2.jar", "mods/same.jar", "mods/lib.jar"]);
    let projects: HashMap<String, String> = [("mods/jei-1.jar", "jei"), ("mods/jei-2.jar", "jei")].into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let p = plan(&f.game(), &f.stage(), Some(&o), &n, &projects, &Keep::client());
    assert_eq!(
        p.steps,
        vec![
            place("mods/jei-2.jar", "mods/jei-2.jar.disabled", false),
            place("mods/same.jar", "mods/same.jar.disabled", false),
            place("mods/sodium-2.jar", "mods/sodium-2.jar", false),
            remove("mods/jei-1.jar.disabled"),
            remove("mods/sodium-1.jar"),
        ]
    );
    assert!(p.kept.is_empty());
}

#[test]
fn player_files_are_never_touched() {
    let f = Fixture::new("player");
    f.has("options.txt", "my keys");
    f.staged("options.txt", "pack keys");
    f.has("saves/World/level.dat", "mine");
    f.staged("saves/World/region/r.0.0.mca", "pack region");
    f.staged("saves/Tutorial/level.dat", "pack world");
    f.has("servers.dat", "my servers");
    let o = old(&[("servers.dat", "pack servers")]);
    let n = staged(&["options.txt", "saves/World/region/r.0.0.mca", "saves/Tutorial/level.dat"]);
    let p = plan(&f.game(), &f.stage(), Some(&o), &n, &HashMap::new(), &Keep::client());
    // Only the world the player doesn't have yet arrives; servers.dat isn't removed.
    assert_eq!(p.steps, vec![place("saves/Tutorial/level.dat", "saves/Tutorial/level.dat", false)]);
}

#[test]
fn unknown_old_version_and_website_files() {
    let f = Fixture::new("unknown");
    f.has("config/x.toml", "mine");
    f.staged("config/x.toml", "pack");
    f.staged("config/new.toml", "pack");
    f.has("mods/blocked-1.jar", "b1");
    f.has("mods/have.jar", "have");
    let mut n = staged(&["config/x.toml", "config/new.toml"]);
    n.insert("mods/have.jar".into(), NewEntry::Pending(sha1("have")));
    n.insert("mods/blocked-2.jar".into(), NewEntry::Pending(sha1("b2")));
    // Without the old version's list, anything at a path the pack now uses is kept as .bak, and nothing is removed.
    let p = plan(&f.game(), &f.stage(), None, &n, &HashMap::new(), &Keep::client());
    assert_eq!(p.steps, vec![place("config/new.toml", "config/new.toml", false), place("config/x.toml", "config/x.toml", true)]);
    assert_eq!(p.pending, vec!["mods/blocked-2.jar".to_string()]);
    // With it, the outdated website-only file makes way for the new one.
    let o = old(&[("mods/blocked-1.jar", "b1"), ("mods/have.jar", "have")]);
    let p = plan(&f.game(), &f.stage(), Some(&o), &n, &HashMap::new(), &Keep::client());
    assert!(p.steps.contains(&remove("mods/blocked-1.jar")));
    assert_eq!(p.pending, vec!["mods/blocked-2.jar".to_string()]);
}

/// Every file and folder under `dir` with its contents.
fn tree(dir: &Path) -> BTreeMap<String, Option<String>> {
    let mut out = BTreeMap::new();
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<String>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = norm(&p.strip_prefix(root).unwrap().to_string_lossy());
            if p.is_dir() {
                out.insert(rel + "/", None);
                go(root, &p, out);
            } else {
                out.insert(rel, Some(std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    go(dir, dir, &mut out);
    out
}

fn switch_fixture(name: &str) -> (Fixture, Vec<Step>) {
    let f = Fixture::new(name);
    f.has("config/b.toml", "b-v1");
    f.has("config/c.toml", "c-mine");
    f.has("config/c.toml.bak", "older bak");
    f.has("config/d.toml", "d-v1");
    f.has("mods/same.jar.disabled", "same1");
    f.has("options.txt", "keys");
    f.staged("config/b.toml", "b-v2");
    f.staged("config/c.toml", "c-v2");
    f.staged("mods/same.jar", "same2");
    f.staged("kubejs/scripts/new.js", "js");
    std::fs::write(f.inst().join("instance.json"), "{\"v\":1}").unwrap();
    std::fs::write(f.inst().join("content-index.json"), "{}").unwrap();
    let steps = vec![
        place("config/b.toml", "config/b.toml", false),
        place("config/c.toml", "config/c.toml", true),
        place("mods/same.jar", "mods/same.jar.disabled", false),
        place("kubejs/scripts/new.js", "kubejs/scripts/new.js", false),
        remove("config/d.toml"),
    ];
    (f, steps)
}

#[test]
fn apply_then_restore_puts_everything_back() {
    let (f, steps) = switch_fixture("roundtrip");
    let before = tree(&f.game());
    snapshot_meta(&f.inst(), &f.work(), &INSTANCE_META).unwrap();
    apply(&f.game(), &f.stage(), &f.work(), &steps).unwrap();
    assert_eq!(f.read("config/b.toml").as_deref(), Some("b-v2"));
    assert_eq!(f.read("config/c.toml").as_deref(), Some("c-v2"));
    assert_eq!(f.read("config/c.toml.bak").as_deref(), Some("c-mine"));
    assert_eq!(f.read("mods/same.jar.disabled").as_deref(), Some("same2"));
    assert_eq!(f.read("kubejs/scripts/new.js").as_deref(), Some("js"));
    assert_eq!(f.read("config/d.toml"), None);
    // The switch then changes the instance's own files; undoing restores them too.
    std::fs::write(f.inst().join("instance.json"), "{\"v\":2}").unwrap();
    std::fs::write(f.inst().join("modpack-files.json"), "{}").unwrap();

    restore(&f.inst(), &f.game(), &f.work(), &INSTANCE_META).unwrap();
    assert_eq!(tree(&f.game()), before);
    assert_eq!(std::fs::read_to_string(f.inst().join("instance.json")).unwrap(), "{\"v\":1}");
    assert!(!f.inst().join("modpack-files.json").exists());
    // Running it again (an interrupted restore) changes nothing.
    restore(&f.inst(), &f.game(), &f.work(), &INSTANCE_META).unwrap();
    assert_eq!(tree(&f.game()), before);
}

#[test]
fn failed_apply_is_undone() {
    let (f, mut steps) = switch_fixture("failure");
    // A staged file that isn't there makes the apply fail halfway.
    steps.insert(3, place("mods/missing.jar", "mods/missing.jar", false));
    let before = tree(&f.game());
    snapshot_meta(&f.inst(), &f.work(), &INSTANCE_META).unwrap();
    assert!(apply(&f.game(), &f.stage(), &f.work(), &steps).is_err());
    assert_ne!(tree(&f.game()), before);
    restore(&f.inst(), &f.game(), &f.work(), &INSTANCE_META).unwrap();
    assert_eq!(tree(&f.game()), before);
}

#[test]
fn layers_map_entries() {
    let l = Layer::new(PathBuf::from("x.zip"), "overrides/");
    assert_eq!(l.map("overrides/config/a.toml").as_deref(), Some("config/a.toml"));
    assert_eq!(l.map("overrides/"), None);
    assert_eq!(l.map("manifest.json"), None);
    let t = Layer { zip: PathBuf::new(), prefix: String::new(), to: String::new(), skip_bin: true };
    assert_eq!(t.map("bin/modpack.jar"), None);
    assert_eq!(t.map("mods/a.jar").as_deref(), Some("mods/a.jar"));
    let e = Layer { zip: PathBuf::new(), prefix: "data/".into(), to: "resourcepacks/".into(), skip_bin: false };
    assert_eq!(e.map("data/pack.zip").as_deref(), Some("resourcepacks/pack.zip"));
    assert_eq!(norm(".\\mods//a.jar"), "mods/a.jar");
}

#[test]
fn record_hashes_round_trip() {
    let h = sha1("x");
    assert_eq!(parse_hash(&hash_text(&h)).map(|x| x.value), h.map(|x| x.value));
    assert!(parse_hash(&hash_text(&None)).is_none());
}

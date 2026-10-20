//! Recognises a Minecraft server folder (server.properties, a server jar, Forge's run files…) and works out what
//! runs it: the software (Vanilla, Paper, Purpur, Fabric, Forge, NeoForge…), its Minecraft version, and the
//! jar or argument file that starts it, so an imported server starts exactly as before. Blocking I/O.

use std::path::Path;

use super::formats::{folder_name, parse_mem, Found, Launcher};
use crate::content::index::{open_zip, read_zip_text};
use crate::core::types::{LoaderType, ServerKind, ServerLaunch};

/// Server folders have server.properties or eula.txt (written on first start), or a Bedrock server program.
pub fn looks_like_server(dir: &Path) -> bool {
    ["server.properties", "eula.txt", "bedrock_server.exe", "bedrock_server"].iter().any(|n| dir.join(n).is_file())
}

/// What a jar says about itself.
#[derive(Debug, Default, PartialEq)]
pub struct JarInfo {
    pub kind: Option<ServerKind>,
    pub mc: Option<String>,
}

/// Looks inside a server jar: Paperclip (Paper/Purpur/Folia/Leaf), the vanilla bundler, CraftBukkit/Spigot,
/// Arclight, plus `version.json` for the Minecraft version.
pub fn inspect_jar(path: &Path) -> JarInfo {
    let mut info = JarInfo::default();
    let Ok(mut zip) = open_zip(path) else {
        return info;
    };
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let has = |prefix: &str| names.iter().any(|n| n.starts_with(prefix));
    if let Ok(Some(v)) = read_zip_text(&mut zip, "version.json") {
        if let Ok(j) = serde_json::from_str::<serde_json::Value>(&v) {
            info.mc = j.get("id").and_then(|i| i.as_str()).map(|s| s.split(['/', ' ']).next().unwrap_or(s).to_string());
        }
    }
    // Bundlers list what they unpack: "<hash>\tpaper-1.20.4\tpaper-1.20.4.jar" or "<hash>\t1.20.4\t1.20.4/server-1.20.4.jar".
    let listed = read_zip_text(&mut zip, "META-INF/versions.list").ok().flatten().unwrap_or_default().to_lowercase();
    let listed_id = listed.lines().next().and_then(|l| l.split('\t').nth(1)).map(str::to_string);
    let brand = |b: &str| listed_id.as_deref().is_some_and(|id| id.starts_with(&format!("{b}-")));
    if has("io/izzel/arclight/") {
        info.kind = Some(ServerKind::Arclight);
    } else if has("io/papermc/paperclip/") || brand("paper") || brand("purpur") || brand("folia") || brand("leaf") {
        info.kind = Some(if brand("purpur") {
            ServerKind::Purpur
        } else if brand("folia") {
            ServerKind::Folia
        } else if brand("leaf") {
            ServerKind::Leaf
        } else {
            ServerKind::Paper
        });
    } else if has("org/bukkit/craftbukkit/") || brand("spigot") || brand("craftbukkit") {
        info.kind = Some(ServerKind::Spigot);
    } else if has("net/minecraft/bundler/") || has("net/minecraft/server/") {
        info.kind = Some(ServerKind::Vanilla);
    }
    if info.mc.is_none() {
        info.mc = listed_id.as_deref().map(|id| id.rsplit('-').next().unwrap_or(id).to_string()).filter(|v| v.starts_with(|c: char| c.is_ascii_digit()));
    }
    info
}

/// The first `x.y` or `x.y.z` version in a name ("paper-1.20.4-496.jar" → 1.20.4).
fn version_in(name: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:^|[^\d.])(1\.\d{1,2}(?:\.\d{1,2})?|[2-9]\d\.\d{1,2}(?:\.\d{1,2})?)(?:[^\d.]|$)").ok()?;
    let name = name.strip_suffix(".jar").unwrap_or(name);
    re.captures(name).map(|c| c[1].to_string())
}

/// The only sub-folder of `dir`, sorted, as a name ("libraries/net/fabricmc/fabric-loader/0.16.5" → "0.16.5").
fn versions_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort_by(|a, b| crate::content::index::natural_cmp(a, b));
    v
}

/// Forge/NeoForge's `@argfile` for this system (the other system's as a fallback), relative to the server folder.
fn args_file(dir: &Path, lib: &str) -> Option<String> {
    let (mine, other) = if cfg!(windows) { ("win_args.txt", "unix_args.txt") } else { ("unix_args.txt", "win_args.txt") };
    [mine, other].iter().map(|f| format!("{lib}/{f}")).find(|rel| dir.join(rel).is_file())
}

/// The Minecraft version a server last ran, from its log.
fn version_from_log(dir: &Path) -> Option<String> {
    let log = std::fs::read_to_string(dir.join("logs").join("latest.log")).ok()?;
    let line = log.lines().find(|l| l.contains("Starting minecraft server version"))?;
    line.rsplit("version").next().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// `-Xmx` from Forge's user_jvm_args.txt or a start script.
fn memory_from_scripts(dir: &Path) -> Option<u32> {
    for name in ["user_jvm_args.txt", "run.bat", "start.bat", "run.sh", "start.sh", "launch.bat", "launch.sh"] {
        let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
            continue;
        };
        for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
            if let Some(at) = line.find("-Xmx") {
                let v: String = line[at + 4..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                if let Some(mb) = parse_mem(&v) {
                    return Some(mb);
                }
            }
        }
    }
    None
}

fn property(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("server.properties")).ok()?;
    text.lines().filter_map(|l| l.split_once('=')).find(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string())
}

/// Reads a server folder. None when it doesn't look like one.
pub fn server_folder(dir: &Path, launcher: Launcher) -> Option<Found> {
    let jars: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_lowercase().ends_with(".jar") && !n.to_lowercase().contains("installer"))
        .collect();
    if !looks_like_server(dir) && jars.is_empty() {
        return None;
    }
    let mut f = Found::new(launcher, folder_name(dir), dir, dir.to_path_buf());
    f.server = true;
    f.skip = vec!["logs".into()];
    let jar = |name: &str| ServerLaunch { jar: Some(name.to_string()), ..Default::default() };
    let lib = |path: &str| dir.join("libraries").join(path);

    if dir.join("bedrock_server.exe").is_file() || dir.join("bedrock_server").is_file() {
        f.server_kind = Some(ServerKind::Bedrock);
        let exe = if cfg!(windows) { "bedrock_server.exe" } else { "bedrock_server" };
        f.launch = Some(ServerLaunch { exe: Some(exe.to_string()), ..Default::default() });
        let notes = std::fs::read_to_string(dir.join("release-notes.txt")).unwrap_or_default();
        f.mc_version = version_in(&notes).or_else(|| notes.lines().next().and_then(|l| l.split_whitespace().last()).map(str::to_string));
    } else if let Some(v) = versions_in(&lib("net/neoforged/neoforge")).pop() {
        f.server_kind = Some(ServerKind::Neoforge);
        f.launch = args_file(dir, &format!("libraries/net/neoforged/neoforge/{v}")).map(|a| ServerLaunch { args_file: Some(a), ..Default::default() });
        f.mc_version = versions_in(&lib("net/minecraft/server")).pop().map(|s| s.split('-').next().unwrap_or(&s).to_string());
        f.loader_version = Some(v);
    } else if let Some(v) = versions_in(&lib("net/minecraftforge/forge")).pop() {
        // "1.20.1-47.2.0"
        f.server_kind = Some(ServerKind::Forge);
        let (mc, forge) = v.split_once('-').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((String::new(), v.clone()));
        f.launch = args_file(dir, &format!("libraries/net/minecraftforge/forge/{v}"))
            .map(|a| ServerLaunch { args_file: Some(a), ..Default::default() })
            .or_else(|| jars.iter().find(|j| j.to_lowercase().starts_with("forge-")).map(|j| jar(j)));
        f.mc_version = Some(mc).filter(|m| !m.is_empty());
        f.loader_version = Some(forge);
    } else if let Some(j) = jars.iter().find(|j| j.to_lowercase().starts_with("forge-")) {
        // Old Forge: forge-1.12.2-14.23.5.2860.jar (or -universal).
        f.server_kind = Some(ServerKind::Forge);
        let rest = j.trim_end_matches(".jar").trim_start_matches("forge-").trim_end_matches("-universal");
        if let Some((mc, v)) = rest.split_once('-') {
            (f.mc_version, f.loader_version) = (Some(mc.to_string()), Some(v.to_string()));
        }
        f.launch = Some(jar(j));
    } else if let Some(j) = jars.iter().find(|j| j.starts_with("fabric-server-mc.")) {
        // fabric-server-mc.1.20.1-loader.0.14.21-launcher.0.11.2.jar
        f.server_kind = Some(ServerKind::Fabric);
        let parts: Vec<&str> = j.trim_end_matches(".jar").split('-').collect();
        f.mc_version = parts.iter().find_map(|p| p.strip_prefix("fabric-server-mc.").or_else(|| p.strip_prefix("mc."))).map(str::to_string);
        f.loader_version = parts.iter().find_map(|p| p.strip_prefix("loader.")).map(str::to_string);
        f.launch = Some(jar(j));
    } else if jars.iter().any(|j| j == "fabric-server-launch.jar") || dir.join(".fabric").is_dir() {
        f.server_kind = Some(ServerKind::Fabric);
        f.loader_version = versions_in(&lib("net/fabricmc/fabric-loader")).pop();
        f.launch = dir.join("fabric-server-launch.jar").is_file().then(|| jar("fabric-server-launch.jar"));
    } else if jars.iter().any(|j| j == "quilt-server-launch.jar") {
        f.server_kind = Some(ServerKind::Quilt);
        f.loader_version = versions_in(&lib("org/quiltmc/quilt-loader")).pop();
        f.launch = Some(jar("quilt-server-launch.jar"));
    }

    // Plain jars: look inside the most likely one (named after the software first, then the biggest).
    if f.server_kind.is_none() || f.mc_version.is_none() {
        let rank = |n: &str| {
            let n = n.to_lowercase();
            ["paper", "purpur", "folia", "leaf", "spigot", "craftbukkit", "arclight", "server", "minecraft_server"]
                .iter()
                .position(|p| n.starts_with(p))
                .unwrap_or(99)
        };
        let mut sorted = jars.clone();
        sorted.sort_by_key(|j| (rank(j), std::cmp::Reverse(std::fs::metadata(dir.join(j)).map(|m| m.len()).unwrap_or(0))));
        let main_jar = f.launch.as_ref().and_then(|l| l.jar.clone());
        for j in main_jar.iter().chain(sorted.iter()) {
            let info = inspect_jar(&dir.join(j));
            if f.server_kind.is_none() {
                if let Some(kind) = info.kind {
                    f.server_kind = Some(kind);
                    f.launch = Some(jar(j));
                }
            }
            if f.mc_version.is_none() {
                f.mc_version = info.mc.or_else(|| version_in(j));
            }
            if f.server_kind.is_some() && f.mc_version.is_some() {
                break;
            }
        }
    }
    if f.mc_version.is_none() {
        f.mc_version = version_from_log(dir).or_else(|| versions_in(&lib("net/minecraft/server")).pop());
    }
    // Unknown software: judge by its folders (a plugin server runs plugins, a mod server mods).
    if f.server_kind.is_none() {
        f.server_kind = Some(if dir.join("plugins").is_dir() {
            ServerKind::Paper
        } else if dir.join("mods").is_dir() {
            ServerKind::Fabric
        } else {
            ServerKind::Vanilla
        });
        if let Some(j) = jars.first() {
            f.launch = Some(jar(j));
        }
    }
    f.loader = f.server_kind.and_then(ServerKind::loader).unwrap_or(LoaderType::Vanilla);
    f.memory_mb = memory_from_scripts(dir);
    f.port = property(dir, "server-port").and_then(|p| p.parse().ok()).filter(|p| *p > 0);
    Some(f.check())
}

#[cfg(test)]
mod tests {
    use super::super::formats::tests::Fixture;
    use super::*;
    use std::io::Write;

    /// A jar (zip) with the given entries.
    fn jar(path: &Path, entries: &[(&str, &str)]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, text) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn vanilla_and_paper_jars() {
        let fx = Fixture::new();
        fx.file("vanilla/server.properties", "server-port=25570\nmotd=hi\n").file("vanilla/eula.txt", "eula=true\n");
        jar(
            &fx.path("vanilla/server.jar"),
            &[
                ("version.json", r#"{"id":"1.21.4","name":"1.21.4"}"#),
                ("META-INF/versions.list", "abc\t1.21.4\t1.21.4/server-1.21.4.jar"),
                ("net/minecraft/bundler/Main.class", ""),
            ],
        );
        let v = server_folder(&fx.path("vanilla"), Launcher::Folder).unwrap();
        assert!(v.server);
        assert_eq!((v.server_kind, v.mc_version.as_deref()), (Some(ServerKind::Vanilla), Some("1.21.4")));
        assert_eq!(v.launch.and_then(|l| l.jar).as_deref(), Some("server.jar"));
        assert_eq!(v.port, Some(25570));

        fx.file("paper/server.properties", "").file("paper/plugins/x.jar", "").file("paper/start.bat", "java -Xmx6G -jar mc.jar nogui\n");
        jar(&fx.path("paper/mc.jar"), &[("io/papermc/paperclip/Main.class", ""), ("META-INF/versions.list", "abc\tpurpur-1.20.4\tpurpur-1.20.4.jar")]);
        let p = server_folder(&fx.path("paper"), Launcher::Folder).unwrap();
        assert_eq!((p.server_kind, p.mc_version.as_deref()), (Some(ServerKind::Purpur), Some("1.20.4")));
        assert_eq!(p.launch.and_then(|l| l.jar).as_deref(), Some("mc.jar"));
        assert_eq!(p.memory_mb, Some(6144));
    }

    #[test]
    fn modded_servers() {
        let fx = Fixture::new();
        fx.file("neo/server.properties", "")
            .file("neo/libraries/net/neoforged/neoforge/21.1.77/win_args.txt", "")
            .file("neo/libraries/net/neoforged/neoforge/21.1.77/unix_args.txt", "")
            .file("neo/libraries/net/minecraft/server/1.21.1-20240808.144430/x", "")
            .file("neo/user_jvm_args.txt", "# comment -Xmx1G\n-Xmx8G\n");
        let n = server_folder(&fx.path("neo"), Launcher::Folder).unwrap();
        assert_eq!((n.server_kind, n.mc_version.as_deref(), n.loader_version.as_deref()), (Some(ServerKind::Neoforge), Some("1.21.1"), Some("21.1.77")));
        let wanted = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
        assert_eq!(n.launch.and_then(|l| l.args_file), Some(format!("libraries/net/neoforged/neoforge/21.1.77/{wanted}")));
        assert_eq!(n.memory_mb, Some(8192));
        assert_eq!(n.loader, LoaderType::Neoforge);

        fx.file("forge/eula.txt", "").file("forge/libraries/net/minecraftforge/forge/1.20.1-47.2.0/unix_args.txt", "");
        let f = server_folder(&fx.path("forge"), Launcher::Folder).unwrap();
        assert_eq!((f.server_kind, f.mc_version.as_deref(), f.loader_version.as_deref()), (Some(ServerKind::Forge), Some("1.20.1"), Some("47.2.0")));
        assert!(f.launch.and_then(|l| l.args_file).is_some_and(|a| a.ends_with("unix_args.txt")), "falls back to the other system's file");

        fx.file("oldforge/server.properties", "").file("oldforge/forge-1.12.2-14.23.5.2860.jar", "");
        let o = server_folder(&fx.path("oldforge"), Launcher::Folder).unwrap();
        assert_eq!((o.mc_version.as_deref(), o.loader_version.as_deref()), (Some("1.12.2"), Some("14.23.5.2860")));

        fx.file("fabric/server.properties", "").file("fabric/fabric-server-mc.1.20.1-loader.0.14.21-launcher.0.11.2.jar", "");
        let fa = server_folder(&fx.path("fabric"), Launcher::Folder).unwrap();
        assert_eq!((fa.server_kind, fa.mc_version.as_deref(), fa.loader_version.as_deref()), (Some(ServerKind::Fabric), Some("1.20.1"), Some("0.14.21")));

        fx.file("fabric2/server.properties", "")
            .file("fabric2/fabric-server-launch.jar", "")
            .file("fabric2/libraries/net/fabricmc/fabric-loader/0.16.5/x.jar", "");
        fx.file("fabric2/logs/latest.log", "[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.21.1\n");
        let fb = server_folder(&fx.path("fabric2"), Launcher::Folder).unwrap();
        assert_eq!((fb.server_kind, fb.mc_version.as_deref(), fb.loader_version.as_deref()), (Some(ServerKind::Fabric), Some("1.21.1"), Some("0.16.5")));
        assert!(is_skipped_logs(&fb));
    }

    fn is_skipped_logs(f: &Found) -> bool {
        super::super::formats::is_skipped("logs", &f.skip)
    }

    #[test]
    fn not_a_server() {
        let fx = Fixture::new();
        fx.file("world/level.dat", "");
        assert!(server_folder(&fx.path("world"), Launcher::Folder).is_none());
        fx.file("mystery/server.properties", "");
        let m = server_folder(&fx.path("mystery"), Launcher::Folder).unwrap();
        assert!(m.problem.is_some(), "no version anywhere");
        assert_eq!(version_in("paper-1.20.4-496.jar").as_deref(), Some("1.20.4"));
        assert_eq!(version_in("server-26.1.jar").as_deref(), Some("26.1"));
    }
}

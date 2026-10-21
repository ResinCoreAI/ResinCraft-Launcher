//! The version an instance starts with: its loader's version JSON, LiteLoader next to Forge, and the player's
//! own changes from instance settings → Version (main class, extra libraries, tweakers, JVM/game arguments),
//! like Prism's custom components. Jar mods are merged into the game jar separately (jarmods.rs).

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::core::net::{hash_file, HashAlgo};
use crate::core::types::{Instance, LoaderType, PatchLibrary};
use crate::core::paths;
use crate::bail_tr;

use super::loaders::{resolve_version, ResolveContext};
use super::mojang::{ArgValue, Argument, FileRef, LibDownloads, Library, VersionJson};

/// Jars the player added from disk are kept in the launcher's libraries folder under this.
const LOCAL: &str = "local";

/// The version an instance starts with (before OptiFine and jar mods, which work on the game jar).
pub async fn resolve_instance(inst: &Instance, ctx: &ResolveContext) -> Result<VersionJson> {
    let mut v = resolve_version(&inst.mc_version, inst.loader, inst.loader_version.as_deref(), ctx).await?;
    if inst.loader == LoaderType::Forge {
        if let Some(ll) = inst.liteloader.as_deref().filter(|v| !v.is_empty()) {
            super::liteloader::apply(&mut v, &inst.mc_version, ll, ctx).await?;
        }
    }
    apply(inst, &mut v);
    super::jarmods::old_game_flags(&mut v, inst);
    Ok(v)
}

fn library(l: &PatchLibrary) -> Library {
    match l.file.as_deref().filter(|f| !f.is_empty()) {
        // A jar from disk: already in place, nothing to download.
        Some(file) => Library {
            name: l.name.clone(),
            downloads: Some(LibDownloads { artifact: Some(FileRef { url: String::new(), path: Some(format!("{LOCAL}/{file}")), ..Default::default() }), classifiers: None }),
            ..Default::default()
        },
        None => Library { name: l.name.trim().to_string(), url: l.url.clone().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()), ..Default::default() },
    }
}

/// Applies the instance's own changes to `v`.
pub fn apply(inst: &Instance, v: &mut VersionJson) {
    let Some(p) = &inst.version_patch else { return };
    if let Some(m) = p.main_class.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        v.main_class = Some(m.to_string());
    }
    let mut libs: Vec<Library> = p.libraries.iter().filter(|l| !l.name.trim().is_empty()).map(library).collect();
    libs.append(&mut v.libraries);
    v.libraries = libs;
    let plain = |s: &String| Argument::Plain(s.trim().to_string());
    let args = v.arguments.get_or_insert_with(Default::default);
    for t in p.tweakers.iter().filter(|t| !t.trim().is_empty()) {
        args.game.extend([Argument::Plain("--tweakClass".into()), plain(t)]);
    }
    args.game.extend(p.game_args.iter().filter(|a| !a.trim().is_empty()).map(plain));
    args.jvm.extend(p.jvm_args.iter().filter(|a| !a.trim().is_empty()).map(plain));
}

/// Copies a jar from disk into the shared libraries folder for an instance's version changes.
pub async fn add_local_library(src: &Path) -> Result<PatchLibrary> {
    let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let check = src.to_path_buf();
    let is_jar = tokio::task::spawn_blocking(move || std::fs::File::open(&check).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some()).await?;
    if name.is_empty() || !is_jar {
        bail_tr!("versionpatch.notJar", "{file} isn't a jar file.", file = name);
    }
    // Under its checksum, so two different jars with the same name don't collide.
    let sha = hash_file(src, HashAlgo::Sha1).await?;
    let file = format!("{}/{name}", &sha[..12]);
    let dest = paths::libraries().join(LOCAL).join(&sha[..12]).join(&name);
    if tokio::fs::metadata(&dest).await.is_err() {
        tokio::fs::create_dir_all(dest.parent().unwrap()).await?;
        tokio::fs::copy(src, &dest).await?;
    }
    Ok(PatchLibrary { name: format!("local:{}", name.trim_end_matches(".jar")), url: None, file: Some(file) })
}

/// What the game starts with, for the Version tab.
#[derive(Debug, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedVersion {
    pub main_class: String,
    pub libraries: Vec<String>,
    pub tweakers: Vec<String>,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub jar_mods: Vec<String>,
}

fn flatten(args: &[Argument]) -> Vec<String> {
    args.iter()
        .flat_map(|a| match a {
            Argument::Plain(s) => vec![s.clone()],
            Argument::Ruled { value: ArgValue::One(s), .. } => vec![s.clone()],
            Argument::Ruled { value: ArgValue::Many(v), .. } => v.clone(),
        })
        .collect()
}

pub fn describe(v: &VersionJson, jar_mods: Vec<String>) -> ResolvedVersion {
    let mut game: Vec<String> = v.minecraft_arguments.as_deref().unwrap_or_default().split(' ').filter(|s| !s.is_empty()).map(str::to_string).collect();
    game.extend(flatten(v.arguments.as_ref().map(|a| a.game.as_slice()).unwrap_or(&[])));
    let mut tweakers = Vec::new();
    let mut rest = Vec::new();
    let mut it = game.into_iter();
    while let Some(a) = it.next() {
        if a == "--tweakClass" {
            tweakers.extend(it.next());
        } else {
            rest.push(a);
        }
    }
    ResolvedVersion {
        main_class: v.main_class.clone().unwrap_or_default(),
        libraries: v.libraries.iter().map(|l| l.name.clone()).collect(),
        tweakers,
        jvm_args: flatten(v.arguments.as_ref().map(|a| a.jvm.as_slice()).unwrap_or(&[])),
        game_args: rest,
        jar_mods,
    }
}

/// Resolves an instance's version (installing its loader if needed) and describes it.
pub async fn resolved(id: &str) -> Result<ResolvedVersion> {
    let inst = crate::core::instances::get(id).await?;
    let v = resolve_instance(&inst, &ResolveContext::default()).await?;
    let mut jars: Vec<String> = super::jarmods::loader_jar_mods(&v).iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
    jars.extend(inst.jar_mods.iter().filter(|m| !m.disabled).map(|m| m.name.clone()));
    Ok(describe(&v, jars))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::VersionPatch;

    #[test]
    fn applies_the_players_changes() {
        let mut v: VersionJson = serde_json::from_value(serde_json::json!({
            "id": "1.12.2", "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name} --tweakClass a.B", "libraries": [{ "name": "x:y:1" }]
        }))
        .unwrap();
        let mut inst: Instance = serde_json::from_value(serde_json::json!({
            "id": "i", "name": "I", "mcVersion": "1.12.2", "loader": "vanilla", "color": "#fff", "createdAt": 0
        }))
        .unwrap();
        inst.version_patch = Some(VersionPatch {
            main_class: Some(" net.minecraft.launchwrapper.Launch ".into()),
            libraries: vec![
                PatchLibrary { name: "org.example:thing:2".into(), url: Some("https://maven.example.org/".into()), file: None },
                PatchLibrary { name: "local:mine".into(), url: None, file: Some("abc/mine.jar".into()) },
            ],
            tweakers: vec!["c.D".into(), " ".into()],
            jvm_args: vec!["-Dfoo=1".into()],
            game_args: vec!["--demo".into()],
        });
        apply(&inst, &mut v);
        assert_eq!(v.main_class.as_deref(), Some("net.minecraft.launchwrapper.Launch"));
        assert_eq!(v.libraries.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["org.example:thing:2", "local:mine", "x:y:1"]);
        assert_eq!(v.libraries[1].downloads.as_ref().unwrap().artifact.as_ref().unwrap().path.as_deref(), Some("local/abc/mine.jar"));
        let d = describe(&v, vec![]);
        assert_eq!(d.tweakers, ["a.B", "c.D"]);
        assert_eq!(d.game_args, ["--username", "${auth_player_name}", "--demo"]);
        assert_eq!(d.jvm_args, ["-Dfoo=1"]);
    }
}

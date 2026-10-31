//! Which game versions can join a hosted server, read from its own jars (the server list shows it, so it's never
//! typed by hand and never comes from a table that goes out of date):
//!
//! - ViaVersion, ViaBackwards and ViaRewind ship one package per protocol step (`protocols/v1_21to1_21_2`, or
//!   `protocol1_9to1_8` in old builds), so a jar itself says which versions that build reaches.
//! - Geyser's `GameProtocol` class names the Bedrock versions that build accepts.
//!
//! Anything that can't be read leaves the range out: a card showing less beats one promising a version that fails.

use std::io::Read;
use std::path::PathBuf;

use crate::content::index::{open_zip, read_zip_text, Zip};

/// A version as numbers, so 1.21.10 sorts after 1.21.9 and 26.1 after 1.21.11.
type Version = Vec<u32>;

/// The oldest and newest version named by a jar's protocol packages.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    min: Version,
    max: Version,
}

impl Span {
    fn covers(&self, v: &Version) -> bool {
        self.min <= *v && *v <= self.max
    }

    fn widen(&mut self, v: Version) {
        if v < self.min {
            self.min = v;
        } else if v > self.max {
            self.max = v;
        }
    }

    fn of(all: impl IntoIterator<Item = Version>) -> Option<Span> {
        let mut all = all.into_iter();
        let first = all.next()?;
        let mut span = Span { min: first.clone(), max: first };
        all.for_each(|v| span.widen(v));
        Some(span)
    }

    fn text(&self) -> (String, String) {
        (text(&self.min), text(&self.max))
    }
}

fn text(v: &[u32]) -> String {
    v.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
}

/// "1.21.4" as numbers; None for snapshots and anything else that isn't only numbers and dots.
fn parse(v: &str) -> Option<Version> {
    let parts: Option<Version> = v.trim().split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| p.len() >= 2)
}

/// The two versions a protocol package joins: "v1_21_11to26_1", or "protocol1_9to1_8" in old builds. Via names a
/// group of versions by writing them together ("1_7_6_10" is 1.7.6 to 1.7.10), so only three numbers count.
fn step(package: &str) -> Option<[Version; 2]> {
    let name = package.strip_prefix('v').or_else(|| package.strip_prefix("protocol"))?;
    let (a, b) = name.split_once("to")?;
    let side = |s: &str| -> Option<Version> {
        let parts: Option<Version> = s.split('_').take(3).map(|p| p.parse().ok()).collect();
        parts.filter(|p| p.len() >= 2)
    };
    Some([side(a)?, side(b)?])
}

/// What a Via jar's protocol packages reach, from the names of the jar's entries.
fn protocol_span<'a>(entries: impl IntoIterator<Item = &'a str>) -> Option<Span> {
    let packages = entries.into_iter().filter_map(|e| {
        let (_, rest) = e.split_once("/protocols/").or_else(|| e.split_once("/protocol/"))?;
        // Files sit inside the package; `protocol/SomeClass.class` beside the packages isn't one.
        rest.split_once('/').map(|(package, _)| package)
    });
    Span::of(packages.filter_map(step).flatten())
}

/// The Java versions players can join with. Each plugin only adds to the range while the chain to the server holds:
/// ViaBackwards and ViaRewind need ViaVersion, ViaRewind picks up where ViaBackwards ends, and a plugin too old for
/// the server's version adds nothing. None when the server only takes its own version.
fn java_range(server: &str, via: Option<&Span>, backwards: Option<&Span>, rewind: Option<&Span>) -> Option<Span> {
    let server = parse(server)?;
    let via = via.filter(|v| v.covers(&server))?;
    let mut range = Span { min: server.clone(), max: server.clone() };
    range.widen(via.max.clone());
    if let Some(backwards) = backwards.filter(|b| b.covers(&server)) {
        range.widen(backwards.min.clone());
        if let Some(rewind) = rewind.filter(|r| r.covers(&range.min)) {
            range.widen(rewind.min.clone());
        }
    }
    (range.min != server || range.max != server).then_some(range)
}

/// The UTF-8 strings in a class file's constant pool.
fn class_strings(class: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if class.len() < 10 || class[..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return out;
    }
    let count = u16::from_be_bytes([class[8], class[9]]);
    let (mut at, mut slot) = (10usize, 1u16);
    while slot < count {
        let Some(&tag) = class.get(at) else { break };
        at += 1;
        match tag {
            1 => {
                let Some(len) = class.get(at..at + 2) else { break };
                let len = u16::from_be_bytes([len[0], len[1]]) as usize;
                let Some(bytes) = class.get(at + 2..at + 2 + len) else { break };
                out.push(String::from_utf8_lossy(bytes).into_owned());
                at += 2 + len;
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => at += 4,
            // Longs and doubles take two slots.
            5 | 6 => {
                at += 8;
                slot += 1;
            }
            7 | 8 | 16 | 19 | 20 => at += 2,
            15 => at += 3,
            _ => break,
        }
        slot += 1;
    }
    out
}

fn entry_bytes(zip: &mut Zip, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// The versions written in a Geyser or protocol class: "26.30", or "1.21.70 - 1.21.73" in some builds. Protocol classes
/// still write 26.30 as "1.26.30", so that's put the way the game and `GameProtocol` say it.
fn bedrock_versions(strings: &[String]) -> Vec<Version> {
    let versions = |s: &String| s.split([' ', '-', '/']).filter(|p| !p.is_empty()).map(parse).collect::<Option<Vec<_>>>().unwrap_or_default();
    strings
        .iter()
        .flat_map(versions)
        .map(|v| match v.as_slice() {
            [1, major, rest @ ..] if *major >= 26 => [&[*major], rest].concat(),
            _ => v,
        })
        .collect()
}

/// The Bedrock versions a jar with Geyser in it accepts: the versions `GameProtocol` names, and those of the protocol
/// classes it uses (a build doesn't have to name the version a protocol class already carries).
fn geyser_span(zip: &mut Zip) -> Option<Span> {
    let class = zip.file_names().find(|n| n.starts_with("org/geysermc/geyser/") && n.ends_with("/GameProtocol.class"))?.to_string();
    let strings = class_strings(&entry_bytes(zip, &class)?);
    let mut versions = bedrock_versions(&strings);
    for s in strings.iter().filter(|s| s.starts_with("org/cloudburstmc/protocol/bedrock/codec/v") && !s.contains(';')) {
        if let Some(codec) = entry_bytes(zip, &format!("{s}.class")) {
            versions.extend(bedrock_versions(&class_strings(&codec)));
        }
    }
    Span::of(versions)
}

/// The `name` a plugin declares.
fn plugin_name(zip: &mut Zip) -> Option<String> {
    let text = ["paper-plugin.yml", "plugin.yml"].iter().find_map(|n| read_zip_text(zip, n).ok().flatten())?;
    text.lines().find_map(|l| Some(l.strip_prefix("name")?.trim_start().strip_prefix(':')?.trim().trim_matches(['"', '\'']).to_string()))
}

/// What a server's jars say: `(oldest, newest)` versions, each None when there's nothing to add to the listing.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reach {
    pub java: Option<(String, String)>,
    pub bedrock: Option<(String, String)>,
}

/// Reads the jars (enabled ones only). `plugins` is false on a mod server, where only Geyser counts: the Via plugins
/// there are other projects with their own rules.
pub fn read(jars: &[PathBuf], mc_version: &str, plugins: bool) -> Reach {
    let (mut via, mut backwards, mut rewind, mut bedrock) = (None, None, None, None);
    for jar in jars {
        let Ok(mut zip) = open_zip(jar) else { continue };
        bedrock = bedrock.or_else(|| geyser_span(&mut zip));
        if !plugins {
            continue;
        }
        let slot = match plugin_name(&mut zip).as_deref() {
            Some("ViaVersion") => &mut via,
            Some("ViaBackwards") => &mut backwards,
            Some("ViaRewind") => &mut rewind,
            _ => continue,
        };
        *slot = protocol_span(zip.file_names());
    }
    Reach {
        java: java_range(mc_version, via.as_ref(), backwards.as_ref(), rewind.as_ref()).map(|r| r.text()),
        bedrock: bedrock.map(|b: Span| b.text()),
    }
}

/// `read`, off the async threads: jars can be large.
pub async fn read_jars(jars: Vec<PathBuf>, mc_version: String, plugins: bool) -> Reach {
    tokio::task::spawn_blocking(move || read(&jars, &mc_version, plugins)).await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn v(s: &str) -> Version {
        parse(s).unwrap()
    }
    fn span(min: &str, max: &str) -> Span {
        Span { min: v(min), max: v(max) }
    }
    fn range(server: &str, via: Option<Span>, back: Option<Span>, rew: Option<Span>) -> Option<(String, String)> {
        java_range(server, via.as_ref(), back.as_ref(), rew.as_ref()).map(|r| r.text())
    }
    fn pair(a: &str, b: &str) -> Option<(String, String)> {
        Some((a.to_string(), b.to_string()))
    }

    #[test]
    fn versions_sort_as_numbers() {
        assert!(v("1.21.10") > v("1.21.9") && v("26.1") > v("1.21.11") && v("1.9") < v("1.9.1"));
        assert_eq!(parse("26w14a"), None);
        assert_eq!(parse("1.21.1-pre1"), None);
        assert_eq!(parse("26"), None);
    }

    #[test]
    fn protocol_packages_new_and_old() {
        assert_eq!(step("v1_21_11to26_1"), Some([v("1.21.11"), v("26.1")]));
        assert_eq!(step("v26_3to26_2"), Some([v("26.3"), v("26.2")]));
        assert_eq!(step("protocol1_9to1_8"), Some([v("1.9"), v("1.8")]));
        // Groups of versions keep their first three numbers.
        assert_eq!(step("v1_8to1_7_6_10"), Some([v("1.8"), v("1.7.6")]));
        assert_eq!(step("v1_7_6_10to1_7_2_5"), Some([v("1.7.6"), v("1.7.2")]));
        for not_one in ["template", "base", "registration", "version"] {
            assert_eq!(step(not_one), None, "{not_one}");
        }
        let entries = [
            "com/viaversion/viaversion/protocols/base/Thing.class",
            "com/viaversion/viaversion/protocols/v1_8to1_9/Protocol1_8To1_9.class",
            "com/viaversion/viaversion/protocols/v26_2to26_3/Protocol26_2To26_3.class",
            "com/viaversion/viaversion/api/protocol/version/ProtocolVersion.class",
            "com/viaversion/viaversion/protocols/v1_20to1_20_2/",
        ];
        assert_eq!(protocol_span(entries), Some(span("1.8", "26.3")));
        assert_eq!(protocol_span(["plugin.yml", "a/protocol/Loose.class"]), None);
    }

    #[test]
    fn the_range_follows_the_chain() {
        let (via, back, rew) = (span("1.8", "26.3"), span("1.9", "26.3"), span("1.7.2", "1.9"));
        // No Via plugins, or Via without a reach past the server: as today.
        assert_eq!(range("26.3", None, None, None), None);
        assert_eq!(range("26.3", Some(via.clone()), None, None), None);
        assert_eq!(range("26.3", None, Some(back.clone()), Some(rew.clone())), None);
        // Newer players only, older players, and the whole chain.
        assert_eq!(range("1.21.1", Some(via.clone()), None, None), pair("1.21.1", "26.3"));
        assert_eq!(range("26.2", Some(via.clone()), Some(back.clone()), None), pair("1.9", "26.3"));
        assert_eq!(range("26.2", Some(via.clone()), Some(back.clone()), Some(rew.clone())), pair("1.7.2", "26.3"));
        // ViaRewind does nothing without ViaBackwards.
        assert_eq!(range("26.2", Some(via.clone()), None, Some(rew.clone())), pair("26.2", "26.3"));
        // ViaBackwards older than the server adds nothing, and neither does ViaRewind behind it.
        assert_eq!(range("26.2", Some(via.clone()), Some(span("1.9", "1.21.11")), Some(rew.clone())), pair("26.2", "26.3"));
        // ViaRewind that doesn't meet where ViaBackwards ends.
        assert_eq!(range("26.2", Some(via.clone()), Some(span("1.12", "26.3")), Some(rew)), pair("1.12", "26.3"));
        // ViaVersion older than the server keeps everything out, and so does a version that can't be read.
        assert_eq!(range("26.3", Some(span("1.8", "26.2")), Some(back.clone()), None), None);
        assert_eq!(range("26w14a", Some(via), Some(back), None), None);
    }

    /// A class file whose constant pool has these strings, between a long (two slots) and an int.
    fn class_with(strings: &[&str]) -> Vec<u8> {
        let mut b = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
        b.extend(((strings.len() + 4) as u16).to_be_bytes());
        b.push(5);
        b.extend([0u8; 8]);
        for s in strings {
            b.push(1);
            b.extend((s.len() as u16).to_be_bytes());
            b.extend(s.as_bytes());
        }
        b.push(3);
        b.extend([0u8; 4]);
        b
    }

    fn jar(path: &Path, files: &[(&str, Vec<u8>)]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, bytes) in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut z, bytes).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn strings_come_out_of_a_class() {
        assert_eq!(class_strings(&class_with(&["26.30", "codec"])), ["26.30", "codec"]);
        assert!(class_strings(b"not a class").is_empty());
        assert_eq!(bedrock_versions(&["26.30".into(), "1.21.70 - 1.21.73".into(), "codec".into(), "26".into()]), [v("26.30"), v("1.21.70"), v("1.21.73")]);
        assert_eq!(bedrock_versions(&["1.26.30".into(), "1.21.130".into()]), [v("26.30"), v("1.21.130")]);
    }

    #[test]
    fn reads_a_server_with_these_plugins() {
        let dir = std::env::temp_dir().join(format!("rc-reach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = |n: &str| dir.join(n);
        let yml = |n: &str| ("plugin.yml", format!("name: {n}\nversion: 5.0\n").into_bytes());
        // Names are from plugin.yml, whatever the file is called.
        jar(&path("a.jar"), &[yml("ViaVersion"), ("com/viaversion/viaversion/protocols/v1_8to1_9/P.class", vec![]), ("com/viaversion/viaversion/protocols/v26_2to26_3/P.class", vec![])]);
        jar(&path("b.jar"), &[yml("ViaBackwards"), ("com/viaversion/viabackwards/protocol/v1_9_1to1_9/P.class", vec![]), ("com/viaversion/viabackwards/protocol/v26_3to26_2/P.class", vec![])]);
        jar(&path("c.jar"), &[yml("ViaRewind"), ("com/viaversion/viarewind/protocol/v1_9to1_8/P.class", vec![]), ("com/viaversion/viarewind/protocol/v1_7_6_10to1_7_2_5/P.class", vec![])]);
        jar(&path("ViaVersionStatus.jar"), &[yml("ViaVersionStatus"), ("x/protocol/v1_2to1_3/P.class", vec![])]);
        jar(
            &path("d.jar"),
            &[
                yml("Geyser-Spigot"),
                ("org/geysermc/geyser/network/bedrock/GameProtocol.class", class_with(&["26.40", "26.52", "org/cloudburstmc/protocol/bedrock/codec/v1/Bedrock_v1"])),
                ("org/cloudburstmc/protocol/bedrock/codec/v1/Bedrock_v1.class", class_with(&["1.26.30"])),
            ],
        );
        std::fs::write(path("broken.jar"), b"nope").unwrap();
        let all: Vec<PathBuf> = ["a.jar", "b.jar", "c.jar", "ViaVersionStatus.jar", "d.jar", "broken.jar"].iter().map(|n| path(n)).collect();
        assert_eq!(read(&all, "26.2", true), Reach { java: pair("1.7.2", "26.3"), bedrock: pair("26.30", "26.52") });
        // A mod server: only Geyser counts.
        assert_eq!(read(&all, "26.2", false), Reach { java: None, bedrock: pair("26.30", "26.52") });
        // No plugins that matter: nothing to show.
        assert_eq!(read(&all[3..4], "26.2", true), Reach::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// On real jars: `REACH_PLUGINS=<plugins folder> REACH_MC=26.2 cargo test real_jars -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_jars() {
        let dir = std::env::var("REACH_PLUGINS").unwrap();
        let jars: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "jar")).collect();
        println!("{:?}", read(&jars, &std::env::var("REACH_MC").unwrap(), true));
    }
}

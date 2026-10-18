//! Telling when two listings are the same mod on different sites (Iris on Modrinth and on CurseForge), so a
//! selection or an instance doesn't end up with two copies of one mod.
//!
//! Evidence, strongest first:
//! - the same file on both sites: CurseForge's SHA-1s looked up on Modrinth (`version_files`), or a local
//!   Modrinth file's CurseForge fingerprint (`/v1/fingerprints`). Pairs proven this way are remembered in
//!   `<cache>/mod-twins.json`;
//! - the same source repository, backed by the same name, author or slug;
//! - the same name and author;
//! - the same slug, backed by the same name or author.
//!
//! Names, authors and slugs are compared loosely ("Entity Culling Fabric/Forge" = "Entity Culling",
//! "JellySquid" = "jellysquid3"). A name alone is never enough.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::index::{index_key, load_index, natural_cmp, save_index, ContentIndex};
use super::jarmeta::{self, JarMeta};
use super::{curseforge, modrinth, sources};
use crate::core::events::Task;
use crate::core::fsutil::{read_json, write_json};
use crate::core::instances;
use crate::core::paths;
use crate::core::settings;
use crate::core::shell;
use crate::core::text::error_payload;
use crate::core::types::{source_of, ContentKind, ContentSource, Instance, LoaderType};
use crate::{bail_tr, err_tr, tr};

// ---------- What the UI sends and gets back ----------

/// A project picked in Discover.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPick {
    pub id: String,
    pub kind: ContentKind,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub slug: String,
    /// A specific version (file id on CurseForge); latest compatible when missing.
    #[serde(default)]
    pub version_id: Option<String>,
}

/// Why two listings are thought to be the same mod (weakest first, so `max` picks the strongest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Evidence {
    Slug,
    Name,
    Link,
    /// The same mod id inside both jars (serialized "modid").
    ModId,
    Hash,
}

/// Picks that are the same mod from different sites.
#[derive(Debug, Clone, Serialize)]
pub struct PickGroup {
    pub ids: Vec<String>,
    pub reason: Evidence,
}

/// A pick the instance already has from another site, or as a jar neither site knows (added by hand).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCopy {
    /// The pick.
    pub id: String,
    pub kind: ContentKind,
    pub filename: String,
    /// Empty for a jar added by hand.
    pub project_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_number: Option<String>,
    pub reason: Evidence,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DuplicateCheck {
    pub groups: Vec<PickGroup>,
    pub installed: Vec<InstalledCopy>,
}

/// An installed file to move to the Recycle Bin before installing (the user chose another site's copy).
#[derive(Debug, Clone, Deserialize)]
pub struct Replace {
    pub kind: ContentKind,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Failed {
    pub id: String,
    /// `{message, key?, params?}`, like a rejected engine call.
    pub error: Value,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct InstallManyResult {
    pub installed: Vec<String>,
    pub failed: Vec<Failed>,
}

// ---------- Facts and matching (pure) ----------

/// What we know about one project, for comparing it with projects on other sites.
#[derive(Debug, Clone, Default)]
pub struct ModFacts {
    /// Project id with its source prefix ("cf:455508", or a bare Modrinth id).
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub slug: String,
    /// Project links (source code, issues…).
    pub links: Vec<String>,
    /// SHA-1s of files known to belong to the project.
    pub sha1s: Vec<String>,
    /// Ids of the same project on other sites, proven by a file hash.
    pub twins: BTreeSet<String>,
    /// The installed file (in the kind's folder), for copies already in an instance.
    pub file: Option<String>,
    pub version_number: Option<String>,
    /// Details were fetched from the project's site.
    pub fetched: bool,
    /// `sha1s` were looked up on Modrinth.
    pub looked_up: bool,
}

impl ModFacts {
    pub fn from_pick(p: &ModPick) -> ModFacts {
        ModFacts {
            id: p.id.clone(),
            title: p.title.clone(),
            authors: if p.author.is_empty() { vec![] } else { vec![p.author.clone()] },
            slug: p.slug.clone(),
            ..Default::default()
        }
    }

    fn source(&self) -> ContentSource {
        source_of(&self.id)
    }

    /// Fills in what `other` knows and this doesn't.
    fn merge(&mut self, other: ModFacts) {
        if self.title.is_empty() {
            self.title = other.title;
        }
        if self.slug.is_empty() {
            self.slug = other.slug;
        }
        for a in other.authors {
            if !self.authors.contains(&a) {
                self.authors.push(a);
            }
        }
        for l in other.links {
            if !self.links.contains(&l) {
                self.links.push(l);
            }
        }
        for h in other.sha1s {
            if !self.sha1s.contains(&h) {
                self.sha1s.push(h);
            }
        }
        self.twins.extend(other.twins);
        self.fetched |= other.fetched;
    }
}

/// Only Modrinth and CurseForge publish single mods.
fn comparable(s: ContentSource) -> bool {
    matches!(s, ContentSource::Modrinth | ContentSource::Curseforge)
}

/// Lowercase letters and digits only: "Iris Shaders" → "irisshaders".
fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

const LOADER_WORDS: [&str; 4] = ["fabric", "forge", "neoforge", "quilt"];

/// A mod name without bracketed tags or trailing loader names: "Entity Culling Fabric/Forge" and "[EC] Entity
/// Culling" both become "entityculling". "Fabric API" stays "fabricapi".
pub fn norm_title(s: &str) -> String {
    let mut plain = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let mut words: Vec<String> = plain.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    while words.len() > 1 && words.last().is_some_and(|w| LOADER_WORDS.contains(&w.as_str())) {
        words.pop();
    }
    let out = words.concat();
    if out.is_empty() {
        squash(s)
    } else {
        out
    }
}

/// An author name without case, punctuation or trailing digits: "JellySquid" = "jellysquid3" = "jellysquid3_".
pub fn norm_author(s: &str) -> String {
    squash(s).trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
}

/// The code repository a link points into ("https://github.com/IrisShaders/Iris/issues" → "github.com/irisshaders/iris").
/// Links to anything but a code host don't count: a shared wiki or Discord says nothing.
pub fn repo_of(url: &str) -> Option<String> {
    let u = url.trim().to_lowercase();
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(&u);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let mut parts = rest.split(['/', '?', '#']).filter(|p| !p.is_empty());
    let host = parts.next()?;
    if !["github.com", "gitlab.com", "codeberg.org", "bitbucket.org", "git.sr.ht"].contains(&host) {
        return None;
    }
    let owner = parts.next()?;
    let repo = parts.next()?.trim_end_matches(".git");
    if repo.is_empty() {
        return None;
    }
    Some(format!("{host}/{owner}/{repo}"))
}

/// Whether `a` and `b` are the same mod on two different sites, and the strongest reason to think so.
/// Listings on the same site are never duplicates of each other (that's one project, or two different ones).
pub fn same_mod(a: &ModFacts, b: &ModFacts) -> Option<Evidence> {
    let (sa, sb) = (a.source(), b.source());
    if sa == sb || !comparable(sa) || !comparable(sb) {
        return None;
    }
    if a.twins.contains(&b.id) || b.twins.contains(&a.id) || a.sha1s.iter().any(|h| !h.is_empty() && b.sha1s.contains(h)) {
        return Some(Evidence::Hash);
    }
    let title = {
        let (x, y) = (norm_title(&a.title), norm_title(&b.title));
        !x.is_empty() && x == y
    };
    let author = a.authors.iter().map(|x| norm_author(x)).filter(|x| !x.is_empty()).any(|x| b.authors.iter().any(|y| norm_author(y) == x));
    let slug = {
        let (x, y) = (squash(&a.slug), squash(&b.slug));
        !x.is_empty() && x == y
    };
    let repo = a.links.iter().filter_map(|l| repo_of(l)).any(|x| b.links.iter().filter_map(|l| repo_of(l)).any(|y| y == x));
    if repo && (title || author || slug) {
        Some(Evidence::Link)
    } else if title && author {
        Some(Evidence::Name)
    } else if slug && (title || author) {
        Some(Evidence::Slug)
    } else {
        None
    }
}

/// Groups picks that are the same mod from different sites (only picks of the same kind are compared).
fn group_picks(facts: &[ModFacts], kinds: &[ContentKind]) -> Vec<PickGroup> {
    // Union-find over the picks, remembering the weakest evidence that joined each group.
    let mut parent: Vec<usize> = (0..facts.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut reason: HashMap<usize, Evidence> = HashMap::new();
    for i in 0..facts.len() {
        for j in i + 1..facts.len() {
            if kinds[i] != kinds[j] {
                continue;
            }
            if let Some(ev) = same_mod(&facts[i], &facts[j]) {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                let weakest = [reason.get(&ri), reason.get(&rj), Some(&ev)].into_iter().flatten().min().copied().unwrap_or(ev);
                if ri != rj {
                    parent[rj] = ri;
                }
                reason.insert(ri, weakest);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for i in 0..facts.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(facts[i].id.clone());
    }
    groups
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(r, ids)| PickGroup { ids, reason: reason.get(&r).copied().unwrap_or(Evidence::Slug) })
        .collect()
}

// ---------- Remembered pairs ----------

type Twins = BTreeMap<String, BTreeSet<String>>;

fn twins_file() -> PathBuf {
    paths::cache().join("mod-twins.json")
}

async fn load_twins() -> Twins {
    read_json(&twins_file()).await.unwrap_or_default()
}

fn pair(map: &mut Twins, a: &str, b: &str) -> bool {
    let new = map.entry(a.to_string()).or_default().insert(b.to_string());
    map.entry(b.to_string()).or_default().insert(a.to_string());
    new
}

// ---------- Fetching facts (best effort: failures leave the facts as they were) ----------

/// Fills in slugs, authors, links and recent file hashes from each site, and pairs CurseForge projects with
/// Modrinth projects that publish the same files.
async fn enrich(facts: &mut [&mut ModFacts], twins: &mut Twins) -> bool {
    let mut changed = false;
    let mr: Vec<String> = facts.iter().filter(|f| !f.fetched && f.source() == ContentSource::Modrinth).map(|f| f.id.clone()).collect();
    let cf: Vec<String> = facts.iter().filter(|f| !f.fetched && f.source() == ContentSource::Curseforge).map(|f| f.id.clone()).collect();
    let mut found: HashMap<String, ModFacts> = HashMap::new();
    if !mr.is_empty() {
        if let Ok(list) = modrinth::facts(&mr).await {
            found.extend(list.into_iter().map(|f| (f.id.clone(), f)));
        }
    }
    if !cf.is_empty() {
        if let Ok(list) = curseforge::facts(&cf).await {
            found.extend(list.into_iter().map(|f| (f.id.clone(), f)));
        }
    }
    for f in facts.iter_mut() {
        if let Some(more) = found.remove(&f.id) {
            f.merge(more);
        }
    }
    changed |= look_up_cf_hashes(facts, twins).await;
    changed
}

/// Asks Modrinth which of its projects publish the files CurseForge lists for these projects.
async fn look_up_cf_hashes(facts: &mut [&mut ModFacts], twins: &mut Twins) -> bool {
    let todo: Vec<usize> = (0..facts.len()).filter(|&i| facts[i].source() == ContentSource::Curseforge && !facts[i].looked_up && !facts[i].sha1s.is_empty()).collect();
    if todo.is_empty() {
        return false;
    }
    let hashes: Vec<String> = todo.iter().flat_map(|&i| facts[i].sha1s.iter().filter(|h| !h.is_empty()).cloned()).collect();
    let Ok(owners) = modrinth::projects_by_sha1(&hashes).await else { return false };
    let mut changed = false;
    for &i in &todo {
        let f = &mut facts[i];
        f.looked_up = true;
        let ids: BTreeSet<String> = f.sha1s.iter().filter_map(|h| owners.get(h).cloned()).collect();
        for id in ids {
            changed |= pair(twins, &f.id, &id);
            f.twins.insert(id);
        }
    }
    changed
}

/// Applies remembered pairs.
fn recall(f: &mut ModFacts, twins: &Twins) {
    if let Some(t) = twins.get(&f.id) {
        f.twins.extend(t.iter().cloned());
    }
}

// ---------- What an instance already has ----------

/// The mods of one kind already in an instance (plus ones about to be installed in the same batch), for spotting
/// a newcomer that is one of them from another site.
pub struct Guard {
    instance_id: String,
    kind: ContentKind,
    present: Vec<ModFacts>,
    twins: Twins,
    /// Installed CurseForge files were looked up on Modrinth.
    cf_looked_up: bool,
    changed: bool,
}

impl Guard {
    /// Everything of `kind` in the instance's content index.
    pub async fn open(inst: &Instance, kind: ContentKind, idx: &ContentIndex) -> Guard {
        Guard::with_twins(inst, kind, idx, load_twins().await)
    }

    fn with_twins(inst: &Instance, kind: ContentKind, idx: &ContentIndex, twins: Twins) -> Guard {
        let prefix = format!("{}/", kind.as_str());
        let present = idx
            .iter()
            .filter(|(k, e)| k.starts_with(&prefix) && !e.project_id.is_empty())
            .map(|(k, e)| {
                let mut f = ModFacts {
                    id: e.project_id.clone(),
                    title: e.title.clone(),
                    sha1s: if e.sha1.is_empty() { vec![] } else { vec![e.sha1.clone()] },
                    file: Some(k[prefix.len()..].to_string()),
                    version_number: Some(e.version_number.clone()).filter(|v| !v.is_empty()),
                    ..Default::default()
                };
                recall(&mut f, &twins);
                f
            })
            .collect();
        Guard { instance_id: inst.id.clone(), kind, present, twins, cf_looked_up: false, changed: false }
    }

    /// Counts `f` as present from now on (installed, or about to be in this batch).
    pub fn add(&mut self, mut f: ModFacts) {
        recall(&mut f, &self.twins);
        self.present.retain(|p| p.id != f.id || p.file.is_some());
        self.present.push(f);
    }

    /// The copy of `cand` this instance already has from another site, and why we think so.
    pub async fn copy_of(&mut self, cand: &mut ModFacts) -> Option<(ModFacts, Evidence)> {
        if !comparable(cand.source()) {
            return None;
        }
        let others: Vec<usize> = (0..self.present.len())
            .filter(|&i| {
                let p = &self.present[i];
                p.id != cand.id && p.source() != cand.source() && comparable(p.source())
            })
            .collect();
        if others.is_empty() {
            return None; // nothing from another site: no lookups needed
        }
        recall(cand, &self.twins);
        if let Some(hit) = self.best(cand, &others) {
            return Some(hit);
        }

        // The same files on both sites. A CurseForge project's recent files come with its details.
        if cand.source() == ContentSource::Curseforge {
            if cand.fetched {
                self.changed |= look_up_cf_hashes(&mut [&mut *cand], &mut self.twins).await;
            } else {
                self.changed |= enrich(&mut [&mut *cand], &mut self.twins).await;
            }
        } else if !self.cf_looked_up {
            self.cf_looked_up = true;
            let mut cf: Vec<&mut ModFacts> = self.present.iter_mut().filter(|p| p.source() == ContentSource::Curseforge).collect();
            self.changed |= look_up_cf_hashes(&mut cf, &mut self.twins).await;
        }
        if let Some(hit) = self.best(cand, &others) {
            return Some(hit);
        }

        // Same name: fetch both sides' details, and fingerprint local Modrinth files for a CurseForge newcomer.
        let name = norm_title(&cand.title);
        let suspects: Vec<usize> = others.iter().copied().filter(|&i| !name.is_empty() && norm_title(&self.present[i].title) == name).collect();
        if suspects.is_empty() {
            return None;
        }
        {
            let mut batch: Vec<&mut ModFacts> = vec![&mut *cand];
            batch.extend(self.present.iter_mut().enumerate().filter(|(i, p)| suspects.contains(i) && !p.fetched).map(|(_, p)| p));
            self.changed |= enrich(&mut batch, &mut self.twins).await;
        }
        if cand.source() == ContentSource::Curseforge {
            let folder = paths::game_dir(&self.instance_id).join(self.kind.folder());
            let files: Vec<(usize, PathBuf)> = suspects
                .iter()
                .filter_map(|&i| {
                    let p = &self.present[i];
                    p.file.as_ref().filter(|_| p.source() == ContentSource::Modrinth).map(|f| (i, folder.join(f)))
                })
                .collect();
            if let Ok(prints) = curseforge::lookup_files(files.iter().map(|(_, p)| p.clone()).collect()).await {
                for (i, path) in files {
                    if let Some((mod_id, _)) = prints.get(&path) {
                        let id = format!("cf:{mod_id}");
                        self.changed |= pair(&mut self.twins, &self.present[i].id, &id);
                        self.present[i].twins.insert(id);
                    }
                }
            }
        }
        self.best(cand, &others)
    }

    fn best(&self, cand: &ModFacts, others: &[usize]) -> Option<(ModFacts, Evidence)> {
        others.iter().filter_map(|&i| same_mod(cand, &self.present[i]).map(|ev| (i, ev))).max_by_key(|(_, ev)| *ev).map(|(i, ev)| (self.present[i].clone(), ev))
    }

    /// Remembers newly proven pairs.
    pub async fn save(&self) {
        if self.changed {
            let _ = write_json(&twins_file(), &self.twins).await;
        }
    }
}

/// Facts for a dependency about to be installed, to check it against the instance.
pub fn dep_facts(id: String, title: &str, slug: &str, authors: Vec<String>, links: Vec<String>, sha1s: Vec<String>) -> ModFacts {
    ModFacts { id, title: title.to_string(), slug: slug.to_string(), authors, links, sha1s, fetched: true, ..Default::default() }
}

/// The task detail shown when a dependency is skipped because the instance already has it from another site.
pub fn skipped_text(name: &str, copy: &ModFacts) -> crate::core::text::Text {
    let source = match copy.source() {
        ContentSource::Curseforge => "CurseForge",
        _ => "Modrinth",
    };
    tr!("dupes.depAlreadyInstalled", "{name} is already installed from {source}", name = name, source = source)
}

// ---------- Channels ----------

/// Before installing picks: which are the same mod from different sites, and which the instance already has
/// from another site (installed by ResinCraft or dropped in by hand).
pub async fn check(instance_id: &str, picks: Vec<ModPick>) -> Result<DuplicateCheck> {
    let inst = instances::get(instance_id).await?;
    let mut out = DuplicateCheck::default();
    let picks: Vec<ModPick> = picks.into_iter().filter(|p| comparable(source_of(&p.id)) && p.kind != ContentKind::Modpack).collect();
    if picks.is_empty() {
        return Ok(out);
    }
    let mut twins = load_twins().await;
    let mut changed = false;
    let mut facts: Vec<ModFacts> = picks.iter().map(ModFacts::from_pick).collect();
    for f in facts.iter_mut() {
        recall(f, &twins);
    }
    let kinds: Vec<ContentKind> = picks.iter().map(|p| p.kind).collect();

    // The same mod picked from two sites.
    let both = facts.iter().any(|f| f.source() == ContentSource::Modrinth) && facts.iter().any(|f| f.source() == ContentSource::Curseforge);
    if both {
        let mut all: Vec<&mut ModFacts> = facts.iter_mut().collect();
        changed |= enrich(&mut all, &mut twins).await;
        out.groups = group_picks(&facts, &kinds);
    }

    // Already in the instance from another site.
    let mut done: Vec<ContentKind> = Vec::new();
    for kind in kinds.iter().copied() {
        if done.contains(&kind) {
            continue;
        }
        done.push(kind);
        // Recognise hand-added files first (by hash, on Modrinth then CurseForge).
        let items = sources::list_content(&inst.id, kind).await.unwrap_or_default();
        let idx = load_index(&inst.id).await;
        let mut guard = Guard::with_twins(&inst, kind, &idx, std::mem::take(&mut twins));
        for (i, f) in facts.iter_mut().enumerate() {
            if kinds[i] != kind {
                continue;
            }
            if let Some((copy, reason)) = guard.copy_of(f).await {
                out.installed.push(InstalledCopy {
                    id: f.id.clone(),
                    kind,
                    filename: copy.file.clone().unwrap_or_default(),
                    project_id: copy.id.clone(),
                    title: copy.title.clone(),
                    version_number: copy.version_number.clone(),
                    reason,
                });
            }
        }
        changed |= guard.changed;
        twins = guard.twins;

        // Jars neither site knows: the same mod id inside as the file a pick would install.
        if kind == ContentKind::Mod {
            let unknown: Vec<String> = items.iter().filter(|i| i.project_id.is_none()).map(|i| i.filename.clone()).collect();
            let open: Vec<&ModPick> = picks.iter().filter(|p| p.kind == kind && !out.installed.iter().any(|c| c.id == p.id)).collect();
            let found = hand_added_copies(&inst, &open, unknown).await;
            out.installed.extend(found);
        }
    }
    if changed {
        let _ = write_json(&twins_file(), &twins).await;
    }
    Ok(out)
}

/// Picks whose file has the same mod id inside as one of `unknown`, jars in the mods folder neither site knows
/// (added by hand). Each pick's file is read from its site in small parts, only when there are such jars.
async fn hand_added_copies(inst: &Instance, picks: &[&ModPick], unknown: Vec<String>) -> Vec<InstalledCopy> {
    if picks.is_empty() || unknown.is_empty() {
        return vec![];
    }
    let folder = paths::game_dir(&inst.id).join(ContentKind::Mod.folder());
    let jars: Vec<(String, JarMeta)> = tokio::task::spawn_blocking(move || {
        unknown.into_iter().map(|n| (jarmeta::read_jar(&folder.join(&n)), n)).filter(|(m, _)| !m.ids.is_empty()).map(|(m, n)| (n, m)).collect()
    })
    .await
    .unwrap_or_default();
    if jars.is_empty() {
        return vec![];
    }
    let mut metas: Vec<Option<JarMeta>> = Vec::new();
    for chunk in picks.chunks(4) {
        metas.extend(futures::future::join_all(chunk.iter().map(|&p| pick_meta(inst, p))).await);
    }
    picks
        .iter()
        .zip(metas)
        .filter_map(|(p, meta)| {
            let meta = meta?;
            let (file, jar) = jars.iter().find(|(_, j)| jarmeta::same_mod_id(&meta, j))?;
            Some(InstalledCopy {
                id: p.id.clone(),
                kind: ContentKind::Mod,
                filename: file.clone(),
                project_id: String::new(),
                title: jar.name.clone().unwrap_or_else(|| file.clone()),
                version_number: jar.version.clone(),
                reason: Evidence::ModId,
            })
        })
        .collect()
}

/// The metadata inside the file installing a pick would download.
async fn pick_meta(inst: &Instance, p: &ModPick) -> Option<JarMeta> {
    let file = match source_of(&p.id) {
        ContentSource::Modrinth => modrinth::planned_file(inst, &p.id, p.kind, p.version_id.as_deref()).await,
        ContentSource::Curseforge => curseforge::planned_file(inst, &p.id, p.kind, p.version_id.as_deref()).await,
        _ => return None,
    };
    jarmeta::remote_meta(&file.ok()??).await
}

/// Installs several projects in one task, each from its own site, after moving `replace` to the Recycle Bin.
/// Dependencies the instance (or the batch) already has from another site are skipped. One failure doesn't stop
/// the rest; failures are listed in the result.
pub async fn install_many(instance_id: &str, picks: Vec<ModPick>, replace: Vec<Replace>) -> Result<InstallManyResult> {
    let inst = instances::get(instance_id).await?;
    if picks.iter().any(|p| p.kind == ContentKind::Mod) && inst.loader == LoaderType::Vanilla {
        bail_tr!(
            "modrinth.vanillaInstance",
            "{name} is a vanilla instance. Mods need a mod loader such as Fabric or NeoForge; create a modded instance first.",
            name = inst.name
        );
    }
    let title = tr!("dupes.installingInto", "Installing into {name}", name = inst.name);
    Task::run(title, Some(inst.id.clone()), move |task| async move {
        for r in &replace {
            super::modrinth::remove_content(&inst.id, r.kind, &r.filename).await?;
        }
        let mut result = InstallManyResult::default();
        let mut blocked = Vec::new();
        let mut kinds: Vec<ContentKind> = Vec::new();
        for p in &picks {
            if !kinds.contains(&p.kind) {
                kinds.push(p.kind);
            }
        }
        let total = picks.len().max(1) as f64;
        let mut n = 0.0;
        for kind in kinds {
            let mut idx = load_index(&inst.id).await;
            let mut guard = Guard::open(&inst, kind, &idx).await;
            let batch: Vec<&ModPick> = picks.iter().filter(|p| p.kind == kind).collect();
            // A dependency of one pick shouldn't bring in another site's copy of a later pick.
            for p in &batch {
                guard.add(ModFacts::from_pick(p));
            }
            for p in batch {
                task.update(tr!("content.installingItem", "Installing {name}…", name = p.title), Some(n / total));
                n += 1.0;
                let r = match source_of(&p.id) {
                    ContentSource::Modrinth => modrinth::install_into(&inst, &task, &mut idx, &mut guard, &p.id, kind, p.version_id.clone()).await,
                    ContentSource::Curseforge => {
                        curseforge::install_into(&inst, &task, &mut idx, &mut guard, &p.id, kind, p.version_id.clone(), &mut blocked).await
                    }
                    _ => Err(err_tr!("dupes.notSingle", "{name} can't be installed into an instance", name = p.title)),
                };
                match r {
                    Ok(()) => result.installed.push(p.id.clone()),
                    Err(e) => result.failed.push(Failed { id: p.id.clone(), error: error_payload(&e) }),
                }
                save_index(&inst.id, &idx).await?;
            }
            guard.save().await;
        }
        if !blocked.is_empty() {
            curseforge::add_pending(&inst.id, blocked).await?;
        }
        Ok(result)
    })
    .await
}

// ---------- Modpacks ----------

/// A mod a modpack brought twice; the extra copy was removed after the install.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedCopy {
    /// The removed file (in mods/).
    pub filename: String,
    /// The copy that stays.
    pub kept: String,
    /// The mod's name.
    pub title: String,
}

/// One jar in a freshly installed pack's mods folder.
#[derive(Debug, Clone, Default)]
struct PackJar {
    file: String,
    /// The pack lists it (the tracked copy) rather than shipping it in its overrides.
    listed: bool,
    /// What a site knows about it (recognised by hash or fingerprint).
    site: Option<ModFacts>,
    meta: JarMeta,
}

impl PackJar {
    fn title(&self) -> String {
        self.site.as_ref().map(|f| f.title.clone()).filter(|t| !t.is_empty()).or_else(|| self.meta.name.clone()).unwrap_or_else(|| self.file.clone())
    }

    fn version(&self) -> &str {
        self.meta.version.as_deref().or(self.site.as_ref().and_then(|f| f.version_number.as_deref())).unwrap_or_default()
    }
}

/// Whether two of a pack's jars are the same mod: one project twice, one mod from both sites, or the same mod id
/// inside (the safety net for files neither site knows).
fn same_jar(a: &PackJar, b: &PackJar) -> bool {
    let by_site = match (&a.site, &b.site) {
        (Some(x), Some(y)) => x.id == y.id || same_mod(x, y).is_some(),
        _ => false,
    };
    by_site || jarmeta::same_mod_id(&a.meta, &b.meta)
}

/// The jars to remove, each with the copy that stays instead. Of each set of jars that are one mod, the copy from
/// the preferred site stays (`pref` "modrinth" | "curseforge"; "ask" prefers neither), then the one the pack lists,
/// then one a site knows, then the newest version.
fn pack_removals(jars: &[PackJar], pref: &str) -> Vec<(usize, usize)> {
    let mut parent: Vec<usize> = (0..jars.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..jars.len() {
        for j in i + 1..jars.len() {
            if same_jar(&jars[i], &jars[j]) {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[rj] = ri;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..jars.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let preferred = match pref {
        "modrinth" => Some(ContentSource::Modrinth),
        "curseforge" => Some(ContentSource::Curseforge),
        _ => None,
    };
    let rank = |j: &PackJar| (preferred.is_some() && j.site.as_ref().map(|f| f.source()) == preferred, j.listed, j.site.is_some());
    let mut out = Vec::new();
    for members in groups.values().filter(|g| g.len() > 1) {
        let keep = members
            .iter()
            .copied()
            .max_by(|&a, &b| {
                let (x, y) = (&jars[a], &jars[b]);
                rank(x).cmp(&rank(y)).then_with(|| natural_cmp(x.version(), y.version())).then_with(|| y.file.cmp(&x.file))
            })
            .unwrap_or(members[0]);
        out.extend(members.iter().filter(|&&i| i != keep).map(|&i| (i, keep)));
    }
    out
}

/// Pairs a pack's Modrinth and CurseForge files that are one mod, with as few lookups as possible: CurseForge files
/// looked up on Modrinth by hash, then, for same-name pairs still unproven, both projects' details and the Modrinth
/// file's CurseForge fingerprint. Returns whether new pairs were proven.
async fn link_sites(facts: &mut [&mut ModFacts], folder: &Path, twins: &mut Twins) -> bool {
    let has = |s: ContentSource| facts.iter().any(|f| f.source() == s);
    if !has(ContentSource::Modrinth) || !has(ContentSource::Curseforge) {
        return false;
    }
    let mut changed = {
        let mut cf: Vec<&mut ModFacts> = facts.iter_mut().filter(|f| f.source() == ContentSource::Curseforge).map(|f| &mut **f).collect();
        look_up_cf_hashes(&mut cf, twins).await
    };
    let mut suspects: BTreeSet<usize> = BTreeSet::new();
    for i in 0..facts.len() {
        for j in i + 1..facts.len() {
            let (a, b) = (&*facts[i], &*facts[j]);
            if a.source() != b.source() && same_mod(a, b).is_none() {
                let name = norm_title(&a.title);
                if !name.is_empty() && name == norm_title(&b.title) {
                    suspects.extend([i, j]);
                }
            }
        }
    }
    if suspects.is_empty() {
        return changed;
    }
    {
        let mut batch: Vec<&mut ModFacts> = facts.iter_mut().enumerate().filter(|(i, f)| suspects.contains(i) && !f.fetched).map(|(_, f)| &mut **f).collect();
        changed |= enrich(&mut batch, twins).await;
    }
    let files: Vec<(usize, PathBuf)> = suspects
        .iter()
        .filter(|&&i| facts[i].source() == ContentSource::Modrinth)
        .filter_map(|&i| facts[i].file.as_ref().map(|f| (i, folder.join(f))))
        .collect();
    if let Ok(prints) = curseforge::lookup_files(files.iter().map(|(_, p)| p.clone()).collect()).await {
        for (i, path) in files {
            if let Some((mod_id, _)) = prints.get(&path) {
                let id = format!("cf:{mod_id}");
                changed |= pair(twins, &facts[i].id, &id);
                facts[i].twins.insert(id);
            }
        }
    }
    changed
}

/// What each new pack instance's `dedupe_pack` removed, until the UI is told (`pack_result`).
static PACK_REPORTS: Mutex<BTreeMap<String, Vec<RemovedCopy>>> = Mutex::new(BTreeMap::new());

/// After a modpack's files and overrides are in place: removes the extra copy of each mod that is in mods/ twice
/// (a jar in the overrides that is also a listed file, one project twice, or one mod from both sites; see
/// `pack_removals` for which copy stays). `listed` are the files in mods/ the pack lists. Removed copies go to the
/// Recycle Bin and are reported with the new instance. Best effort: lookups that fail just find less.
pub async fn dedupe_pack(inst: &Instance, task: &Task, listed: &[String]) {
    let removed = remove_pack_twins(inst, task, listed).await;
    if !removed.is_empty() {
        PACK_REPORTS.lock().unwrap().insert(inst.id.clone(), removed);
    }
}

async fn remove_pack_twins(inst: &Instance, task: &Task, listed: &[String]) -> Vec<RemovedCopy> {
    let folder = paths::game_dir(&inst.id).join(ContentKind::Mod.folder());
    let mut files = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&folder).await {
        while let Ok(Some(e)) = dir.next_entry().await {
            let name = e.file_name().to_string_lossy().into_owned();
            // Disabled jars don't load, so they can't clash.
            if name.to_lowercase().ends_with(".jar") && e.file_type().await.is_ok_and(|t| t.is_file()) {
                files.push(name);
            }
        }
    }
    if files.len() < 2 {
        return vec![];
    }
    task.update(tr!("dupes.checkingPack", "Checking for mods the pack has twice…"), None);
    // Which site each file is from: by hash on Modrinth, then by fingerprint on CurseForge.
    let _ = sources::list_content(&inst.id, ContentKind::Mod).await;
    let mut idx = load_index(&inst.id).await;
    let mut twins = load_twins().await;
    let metas = {
        let (folder, files) = (folder.clone(), files.clone());
        tokio::task::spawn_blocking(move || files.iter().map(|f| jarmeta::read_jar(&folder.join(f))).collect::<Vec<_>>()).await.unwrap_or_default()
    };
    let mut jars: Vec<PackJar> = files
        .into_iter()
        .zip(metas.into_iter().chain(std::iter::repeat_with(JarMeta::default)))
        .map(|(file, meta)| {
            let site = idx.get(&index_key(ContentKind::Mod, &file)).filter(|e| !e.project_id.is_empty()).map(|e| {
                let mut f = ModFacts {
                    id: e.project_id.clone(),
                    title: e.title.clone(),
                    sha1s: if e.sha1.is_empty() { vec![] } else { vec![e.sha1.clone()] },
                    file: Some(file.clone()),
                    version_number: Some(e.version_number.clone()).filter(|v| !v.is_empty()),
                    ..Default::default()
                };
                recall(&mut f, &twins);
                f
            });
            PackJar { listed: listed.contains(&file), file, site, meta }
        })
        .collect();
    let changed = {
        let mut sites: Vec<&mut ModFacts> = jars.iter_mut().filter_map(|j| j.site.as_mut()).collect();
        link_sites(&mut sites, &folder, &mut twins).await
    };
    if changed {
        let _ = write_json(&twins_file(), &twins).await;
    }

    let pref = settings::get().await.preferred_mod_source;
    let mut removed = Vec::new();
    for (i, keep) in pack_removals(&jars, &pref) {
        let path = folder.join(&jars[i].file);
        // The file came with the pack, not from the player: if the Recycle Bin won't take it, delete it, since
        // two copies stop the game from starting.
        if shell::trash(&path).await.is_err() && tokio::fs::remove_file(&path).await.is_err() {
            continue;
        }
        idx.remove(&index_key(ContentKind::Mod, &jars[i].file));
        removed.push(RemovedCopy { filename: jars[i].file.clone(), kept: jars[keep].file.clone(), title: jars[keep].title() });
    }
    if !removed.is_empty() {
        let _ = save_index(&inst.id, &idx).await;
    }
    removed
}

/// A new modpack instance as the UI gets it: the instance, plus `duplicatesRemoved` when its pack had mods twice.
pub fn pack_result(inst: Instance) -> Result<Value> {
    let mut v = serde_json::to_value(&inst)?;
    let removed = PACK_REPORTS.lock().unwrap().remove(&inst.id);
    if let (Some(removed), Value::Object(map)) = (removed, &mut v) {
        map.insert("duplicatesRemoved".into(), serde_json::to_value(removed)?);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mr(id: &str, title: &str, author: &str, slug: &str, links: &[&str]) -> ModFacts {
        ModFacts {
            id: id.into(),
            title: title.into(),
            authors: vec![author.into()],
            slug: slug.into(),
            links: links.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn cf(id: u64, title: &str, authors: &[&str], slug: &str, links: &[&str]) -> ModFacts {
        ModFacts {
            id: format!("cf:{id}"),
            title: title.into(),
            authors: authors.iter().map(|s| s.to_string()).collect(),
            slug: slug.into(),
            links: links.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn normalises_names() {
        assert_eq!(norm_title("Iris Shaders"), "irisshaders");
        assert_eq!(norm_title("Entity Culling Fabric/Forge"), "entityculling");
        assert_eq!(norm_title("Entity Culling"), "entityculling");
        assert_eq!(norm_title("[EMF] Entity Model Features"), "entitymodelfeatures");
        assert_eq!(norm_title("Sodium (Fabric)"), "sodium");
        assert_eq!(norm_title("Fabric API"), "fabricapi");
        // A name that is only a loader word stays itself.
        assert_eq!(norm_title("Forge"), "forge");
        assert_eq!(norm_author("JellySquid"), norm_author("jellysquid3"));
        assert_eq!(norm_author("jellysquid3_"), "jellysquid");
        assert_eq!(norm_author("tr7zw"), "tr7zw");
    }

    #[test]
    fn finds_repositories() {
        assert_eq!(repo_of("https://github.com/IrisShaders/Iris/issues").as_deref(), Some("github.com/irisshaders/iris"));
        assert_eq!(repo_of("https://www.github.com/IrisShaders/Iris.git/").as_deref(), Some("github.com/irisshaders/iris"));
        assert_eq!(repo_of("http://gitlab.com/a/b?tab=readme").as_deref(), Some("gitlab.com/a/b"));
        assert_eq!(repo_of("https://github.com/IrisShaders"), None);
        assert_eq!(repo_of("https://discord.gg/abc"), None);
        assert_eq!(repo_of("https://www.curseforge.com/minecraft/mc-mods/irisshaders"), None);
    }

    #[test]
    fn matches_real_listings() {
        // Live data (2026-10): each mod differs between the sites in some way.
        let iris_mr = mr("YL57xq9U", "Iris Shaders", "coderbot", "iris", &["https://github.com/IrisShaders/Iris"]);
        let iris_cf = cf(455508, "Iris Shaders", &["coderbot", "IMS21"], "irisshaders", &["https://github.com/IrisShaders/Iris"]);
        assert_eq!(same_mod(&iris_mr, &iris_cf), Some(Evidence::Link));

        let sodium_mr = mr("AANobbMI", "Sodium", "jellysquid3", "sodium", &[]);
        let sodium_cf = cf(394468, "Sodium", &["JellySquid"], "sodium", &[]);
        assert_eq!(same_mod(&sodium_mr, &sodium_cf), Some(Evidence::Name));

        // Fabric API's CurseForge page links the repository's old name.
        let api_mr = mr("P7dR8mSH", "Fabric API", "modmuss50", "fabric-api", &["https://github.com/FabricMC/fabric-api"]);
        let api_cf = cf(306612, "Fabric API", &["modmuss50", "sfPlayer1"], "fabric-api", &["https://github.com/FabricMC/fabric"]);
        assert_eq!(same_mod(&api_mr, &api_cf), Some(Evidence::Name));

        let ec_mr = mr("NNAgCjsB", "Entity Culling", "tr7zw", "entityculling", &[]);
        let ec_cf = cf(448233, "Entity Culling Fabric/Forge", &["tr7zw"], "entityculling", &[]);
        assert_eq!(same_mod(&ec_mr, &ec_cf), Some(Evidence::Name));

        // Same slug and author, different name: still the same project.
        let a = mr("x1", "Cool Mod", "someone", "coolmod", &[]);
        let b = cf(1, "Cool Mod Reloaded", &["someone"], "coolmod", &[]);
        assert_eq!(same_mod(&a, &b), Some(Evidence::Slug));
    }

    #[test]
    fn hashes_beat_everything() {
        let mut a = mr("YL57xq9U", "Iris", "", "", &[]);
        let b = cf(455508, "Something else", &[], "", &[]);
        assert_eq!(same_mod(&a, &b), None);
        a.twins.insert("cf:455508".into());
        assert_eq!(same_mod(&a, &b), Some(Evidence::Hash));
        assert_eq!(same_mod(&b, &a), Some(Evidence::Hash));

        let mut c = mr("abc", "One", "", "", &[]);
        let mut d = cf(7, "Two", &[], "", &[]);
        c.sha1s = vec!["deadbeef".into()];
        d.sha1s = vec!["cafe".into(), "deadbeef".into()];
        assert_eq!(same_mod(&c, &d), Some(Evidence::Hash));
        // Empty hashes never match.
        c.sha1s = vec![String::new()];
        d.sha1s = vec![String::new()];
        assert_eq!(same_mod(&c, &d), None);
    }

    #[test]
    fn needs_two_signals() {
        // Same name, different authors and slugs: different mods.
        let a = mr("x1", "Backpacks", "alice", "backpacks", &[]);
        let b = cf(2, "Backpacks", &["bob"], "simple-backpacks", &[]);
        assert_eq!(same_mod(&a, &b), None);
        // A shared repository alone isn't enough (one repository can hold several mods).
        let c = mr("x2", "Alpha", "alice", "alpha", &["https://github.com/alice/mods"]);
        let d = cf(3, "Beta", &["bob"], "beta", &["https://github.com/alice/mods"]);
        assert_eq!(same_mod(&c, &d), None);
        // Same slug alone isn't either.
        let e = mr("x3", "Iris", "someone", "iris", &[]);
        let f = cf(4, "Iris Flowers", &["gardener"], "iris", &[]);
        assert_eq!(same_mod(&e, &f), None);
    }

    #[test]
    fn same_site_is_never_a_duplicate() {
        let a = mr("AANobbMI", "Sodium", "jellysquid3", "sodium", &[]);
        let b = mr("other", "Sodium", "jellysquid3", "sodium", &[]);
        assert_eq!(same_mod(&a, &b), None);
        let c = cf(1, "Sodium", &["jellysquid3"], "sodium", &[]);
        let d = ModFacts { id: "ftb:1".into(), ..c.clone() };
        assert_eq!(same_mod(&c, &d), None);
    }

    #[test]
    fn groups_picks_across_sites() {
        let facts = vec![
            mr("YL57xq9U", "Iris Shaders", "coderbot", "iris", &[]),
            mr("AANobbMI", "Sodium", "jellysquid3", "sodium", &[]),
            cf(455508, "Iris Shaders", &["coderbot"], "irisshaders", &[]),
            cf(238222, "Just Enough Items (JEI)", &["mezz"], "jei", &[]),
            cf(394468, "Sodium", &["JellySquid"], "sodium", &[]),
        ];
        let kinds = vec![ContentKind::Mod; 5];
        let groups = group_picks(&facts, &kinds);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].ids, vec!["YL57xq9U".to_string(), "cf:455508".to_string()]);
        assert_eq!(groups[1].ids, vec!["AANobbMI".to_string(), "cf:394468".to_string()]);
        assert!(groups.iter().all(|g| g.reason == Evidence::Name));

        // Different kinds are never grouped.
        let kinds = vec![ContentKind::Mod, ContentKind::Mod, ContentKind::Shader, ContentKind::Mod, ContentKind::Mod];
        let groups = group_picks(&facts, &kinds);
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn guard_spots_copies_without_network() {
        // A remembered pair, an installed file's hash and a planned pick all answer without asking any site.
        let mut twins = Twins::new();
        pair(&mut twins, "P7dR8mSH", "cf:306612");
        let entry = |id: &str, title: &str, sha1: &str| IndexEntry { project_id: id.into(), title: title.into(), sha1: sha1.into(), ..Default::default() };
        let mut idx = ContentIndex::new();
        idx.insert("mod/fabric-api-0.110.jar".into(), entry("P7dR8mSH", "Fabric API", "aaa"));
        idx.insert("mod/iris-1.8.jar".into(), entry("cf:455508", "Iris Shaders", "bbb"));
        idx.insert("shader/complementary.zip".into(), entry("cf:627557", "Complementary Shaders", "ddd"));
        let mut guard = Guard::with_twins(&test_instance(), ContentKind::Mod, &idx, twins);
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();

        let mut api = dep_facts("cf:306612".into(), "Fabric API", "fabric-api", vec![], vec![], vec![]);
        let (copy, ev) = rt.block_on(guard.copy_of(&mut api)).expect("remembered pair");
        assert_eq!((copy.file.as_deref(), ev), (Some("fabric-api-0.110.jar"), Evidence::Hash));

        let mut iris = dep_facts("YL57xq9U".into(), "Iris Shaders", "iris", vec![], vec![], vec!["bbb".into()]);
        let (copy, ev) = rt.block_on(guard.copy_of(&mut iris)).expect("same file");
        assert_eq!((copy.id.as_str(), ev), ("cf:455508", Evidence::Hash));

        // Planned picks count as present.
        guard.add(ModFacts { id: "AANobbMI".into(), title: "Sodium".into(), sha1s: vec!["ccc".into()], ..Default::default() });
        let mut sodium = dep_facts("cf:394468".into(), "Sodium", "sodium", vec![], vec![], vec!["ccc".into()]);
        assert!(rt.block_on(guard.copy_of(&mut sodium)).is_some());

        // Only the guard's kind counts, and nothing from another site means no lookups at all.
        let mut only_mr = ContentIndex::new();
        only_mr.insert("mod/fabric-api-0.110.jar".into(), entry("P7dR8mSH", "Fabric API", "aaa"));
        only_mr.insert("shader/complementary.zip".into(), entry("cf:627557", "Complementary Shaders", "ddd"));
        let mut guard = Guard::with_twins(&test_instance(), ContentKind::Mod, &only_mr, Twins::new());
        let mut same = dep_facts("P7dR8mSH".into(), "Fabric API", "fabric-api", vec![], vec![], vec![]);
        assert!(rt.block_on(guard.copy_of(&mut same)).is_none());
        let mut shader = dep_facts("abc".into(), "Complementary Shaders", "complementary", vec![], vec![], vec!["ddd".into()]);
        assert!(rt.block_on(guard.copy_of(&mut shader)).is_none());
    }

    fn jar(file: &str, listed: bool, site: Option<ModFacts>, ids: &[&str], version: &str) -> PackJar {
        let meta = JarMeta { ids: ids.iter().map(|s| s.to_string()).collect(), name: None, version: Some(version.into()).filter(|v: &String| !v.is_empty()) };
        PackJar { file: file.into(), listed, site: site.map(|mut f| {
            f.file = Some(file.into());
            f
        }), meta }
    }

    /// (removed, kept) file names.
    fn removals(jars: &[PackJar], pref: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = pack_removals(jars, pref).into_iter().map(|(i, k)| (jars[i].file.clone(), jars[k].file.clone())).collect();
        out.sort();
        out
    }

    #[test]
    fn pack_keeps_the_listed_copy() {
        // A CurseForge pack lists Sodium; its overrides ship the Modrinth jar of the same mod, and an unknown copy
        // of JEI next to the listed one. Different mods are left alone.
        let jars = vec![
            jar("sodium-cf.jar", true, Some(cf(394468, "Sodium", &["JellySquid"], "", &[])), &["sodium"], "0.5.11"),
            jar("sodium-mr.jar", false, Some(mr("AANobbMI", "Sodium", "jellysquid3", "", &[])), &["sodium"], "0.5.13"),
            jar("jei-listed.jar", true, Some(cf(238222, "Just Enough Items", &["mezz"], "", &[])), &["jei"], "19.21"),
            jar("jei-old-build.jar", false, None, &["jei"], "19.19"),
            jar("lithium.jar", true, Some(mr("gvQqBUqZ", "Lithium", "jellysquid3", "", &[])), &["lithium"], "0.14"),
            jar("nometa.jar", false, None, &[], ""),
        ];
        let listed = vec![("jei-old-build.jar".to_string(), "jei-listed.jar".to_string()), ("sodium-mr.jar".to_string(), "sodium-cf.jar".to_string())];
        assert_eq!(removals(&jars, "ask"), listed);
        // A preferred site beats the pack's list; with no copy from that site, the listed one still stays.
        assert_eq!(
            removals(&jars, "modrinth"),
            vec![("jei-old-build.jar".to_string(), "jei-listed.jar".to_string()), ("sodium-cf.jar".to_string(), "sodium-mr.jar".to_string())]
        );
        assert_eq!(removals(&jars, "curseforge"), listed);
    }

    #[test]
    fn pack_pairs_without_mod_ids() {
        // No metadata: one project twice (two versions), and one mod on both sites proven by a remembered pair.
        let mut api_mr = mr("P7dR8mSH", "Fabric API", "modmuss50", "", &[]);
        api_mr.twins.insert("cf:306612".into());
        let jars = vec![
            jar("iris-1.7.jar", true, Some(mr("YL57xq9U", "Iris", "coderbot", "", &[])), &[], "1.7.0"),
            jar("iris-1.8.jar", true, Some(mr("YL57xq9U", "Iris", "coderbot", "", &[])), &[], "1.8.0"),
            jar("fabric-api-mr.jar", false, Some(api_mr), &[], ""),
            jar("fabric-api-cf.jar", true, Some(cf(306612, "Fabric API", &["modmuss50"], "", &[])), &[], ""),
            // Same name on both sites, nothing else in common: not paired.
            jar("backpacks-a.jar", true, Some(mr("x1", "Backpacks", "alice", "", &[])), &[], ""),
            jar("backpacks-b.jar", true, Some(cf(2, "Backpacks", &["bob"], "", &[])), &[], ""),
        ];
        assert_eq!(
            removals(&jars, "ask"),
            vec![("fabric-api-mr.jar".to_string(), "fabric-api-cf.jar".to_string()), ("iris-1.7.jar".to_string(), "iris-1.8.jar".to_string())]
        );
        // Three copies by mod id, none listed or known: the newest stays.
        let jars = vec![jar("a.jar", false, None, &["create"], "0.5.1"), jar("b.jar", false, None, &["create"], "0.5.10"), jar("c.jar", false, None, &["create"], "0.5.9")];
        assert_eq!(removals(&jars, "ask"), vec![("a.jar".to_string(), "b.jar".to_string()), ("c.jar".to_string(), "b.jar".to_string())]);
        assert_eq!(jars[1].title(), "b.jar");
    }

    #[test]
    fn pack_report_rides_on_the_instance() {
        let inst = test_instance();
        let removed = RemovedCopy { filename: "a.jar".into(), kept: "b.jar".into(), title: "Create".into() };
        PACK_REPORTS.lock().unwrap().insert(inst.id.clone(), vec![removed]);
        let v = pack_result(inst.clone()).unwrap();
        assert_eq!(v["duplicatesRemoved"][0]["kept"], "b.jar");
        assert_eq!(v["name"], "Test");
        // Told once.
        assert!(pack_result(inst).unwrap().get("duplicatesRemoved").is_none());
        assert_eq!(serde_json::to_value(Evidence::ModId).unwrap(), "modid");
    }

    use super::super::index::IndexEntry;

    fn test_instance() -> Instance {
        serde_json::from_value(serde_json::json!({
            "id": "test", "name": "Test", "mcVersion": "1.21.4", "loader": "fabric", "color": "#fff", "createdAt": 0, "playtime": 0
        }))
        .unwrap()
    }
}

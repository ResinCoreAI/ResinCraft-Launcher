//! Discover's details page: a project's description, gallery and links, and per version its changelog and
//! dependencies, from whichever source it's on. Each source fills in what it has and says what it can't
//! (`changelogs` / `dependencies`). HTML is passed on as the source wrote it; the UI sanitizes it.

use std::sync::OnceLock;

use anyhow::Result;
use pulldown_cmark::{html, Event, Options, Parser};
use regex::{Captures, Regex};

use super::{atlauncher, curseforge, ftb, github, modrinth, technic};
use crate::core::types::{source_of, ContentSource, ProjectDetails, VersionInfo};

pub async fn details(project_id: &str) -> Result<ProjectDetails> {
    match source_of(project_id) {
        ContentSource::Modrinth => modrinth::details(project_id).await,
        ContentSource::Curseforge => curseforge::details(project_id).await,
        ContentSource::Ftb => ftb::details(project_id).await,
        ContentSource::Atlauncher => atlauncher::details(project_id).await,
        ContentSource::Technic => technic::details(project_id).await,
        ContentSource::Github => github::details(project_id).await,
    }
}

pub async fn version_info(project_id: &str, version_id: &str) -> Result<VersionInfo> {
    match source_of(project_id) {
        ContentSource::Modrinth => modrinth::version_info(version_id).await,
        ContentSource::Curseforge => curseforge::version_info(project_id, version_id).await,
        ContentSource::Ftb => ftb::version_info(project_id, version_id).await,
        ContentSource::Atlauncher => atlauncher::version_info(project_id, version_id).await,
        ContentSource::Github => github::version_info(project_id, version_id).await,
        // Technic has no changelogs and packs carry no dependencies.
        ContentSource::Technic => Ok(VersionInfo::default()),
    }
}

/// The mods in version `version_id` of modpack `project_id`, sorted by name. Packs whose list lives inside their
/// file have only that file's index read (ranged requests), not the whole download.
pub async fn pack_mods(project_id: &str, version_id: &str) -> Result<Vec<crate::core::types::PackMod>> {
    let mut list = match source_of(project_id) {
        ContentSource::Modrinth => modrinth::pack_mods(version_id).await?,
        ContentSource::Curseforge => curseforge::pack_mods(project_id, version_id).await?,
        ContentSource::Ftb => ftb::pack_mods(project_id, version_id).await?,
        ContentSource::Atlauncher => atlauncher::pack_mods(project_id, version_id).await?,
        ContentSource::Technic => technic::pack_mods(project_id, version_id).await?,
        ContentSource::Github => Vec::new(),
    };
    list.sort_by_key(|m| m.title.to_lowercase());
    list.dedup_by(|a, b| a.title == b.title && a.id == b.id);
    Ok(list)
}

/// "mods/Sodium-0.5.11.jar" → ("Sodium-0.5.11", "Sodium-0.5.11.jar").
pub(crate) fn title_of_file(path: &str) -> (String, String) {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
    let title = file.strip_suffix(".jar").or_else(|| file.strip_suffix(".zip")).unwrap_or(&file).to_string();
    (title, file)
}

/// Whether a pack file path is a mod (in mods/, a jar).
pub(crate) fn is_mod_path(path: &str) -> bool {
    let p = path.trim_start_matches("./").replace('\\', "/");
    p.starts_with("mods/") && (p.ends_with(".jar") || p.ends_with(".jar.disabled"))
}

/// Sort order for a version's dependencies: what it needs first, what it can't run with last.
pub fn dependency_rank(kind: &str) -> u8 {
    match kind {
        "required" => 0,
        "optional" => 1,
        "embedded" => 2,
        _ => 3,
    }
}

/// Markdown (GitHub-flavoured: tables, strikethrough, task lists; raw HTML kept) to HTML. `hard_breaks` keeps
/// single newlines, for plain text that was never written as Markdown.
pub fn markdown(text: &str, hard_breaks: bool) -> String {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_GFM;
    let events = Parser::new_ext(text, opts).map(|e| match e {
        Event::SoftBreak if hard_breaks => Event::HardBreak,
        e => e,
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

/// Makes relative `src` and `href` attributes absolute: images against `raw` (where the files can be
/// downloaded), links against `page`. Both end in "/". Anchors ("#usage") stay as they are.
pub fn absolutize(html: &str, page: &str, raw: &str) -> String {
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let re = ATTR.get_or_init(|| Regex::new(r#"(?i)\b(src|href)\s*=\s*(["'])([^"']*)(["'])"#).unwrap());
    let scheme = |u: &str| u.split_once(':').is_some_and(|(s, _)| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)));
    re.replace_all(html, |c: &Captures| {
        let url = c[3].trim();
        let fixed = if url.is_empty() || url.starts_with('#') || scheme(url) {
            url.to_string()
        } else if let Some(rest) = url.strip_prefix("//") {
            format!("https://{rest}")
        } else {
            let base = if c[1].eq_ignore_ascii_case("src") { raw } else { page };
            format!("{base}{}", url.trim_start_matches("./").trim_start_matches('/'))
        };
        format!("{}={}{}{}", &c[1], &c[2], fixed, &c[4])
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_renders_gfm_and_keeps_html() {
        let html = markdown("# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n~~old~~ <center>hi</center>", false);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<table>"));
        assert!(html.contains("<del>old</del>"));
        assert!(html.contains("<center>hi</center>"));
    }

    #[test]
    fn plain_text_keeps_its_line_breaks() {
        assert!(markdown("one\r\ntwo", true).contains("one<br />"));
        assert!(!markdown("one\ntwo", false).contains("<br"));
    }

    #[test]
    fn relative_urls_become_absolute() {
        let page = "https://github.com/o/r/blob/HEAD/";
        let raw = "https://raw.githubusercontent.com/o/r/HEAD/";
        let html = absolutize(r##"<img src="docs/a.png"><a href="./LICENSE">l</a><a href="#top">t</a><img src='/b.gif'>"##, page, raw);
        assert!(html.contains(r#"src="https://raw.githubusercontent.com/o/r/HEAD/docs/a.png""#));
        assert!(html.contains(r#"href="https://github.com/o/r/blob/HEAD/LICENSE""#));
        assert!(html.contains(r##"href="#top""##));
        assert!(html.contains("src='https://raw.githubusercontent.com/o/r/HEAD/b.gif'"));
        let kept = r#"<a href="https://x.dev/a">a</a><a href="mailto:me@x.dev">m</a><img src="//cdn.x.dev/i.png">"#;
        let out = absolutize(kept, page, raw);
        assert!(out.contains(r#"href="https://x.dev/a""#) && out.contains("mailto:me@x.dev") && out.contains(r#"src="https://cdn.x.dev/i.png""#));
    }
}

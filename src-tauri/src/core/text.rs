//! Player-facing text the UI can translate.
//!
//! `tr!("launch.needJava", "Minecraft {version} needs Java {major}.", version = v, major = 21)` builds a [`Text`]:
//! a stable key, the English wording (shown in logs, and the fallback) and the named parameters. Use
//! `bail_tr!(…)` to return one as an error and `err_tr!(…)` to make one; task titles and details accept a Text too.
//! An error that *is* a Text reaches the UI as `{message, key, params}` and is shown in the player's language.
//!
//! The UI's English source for these keys (src/renderer/src/i18n/en/engine.ts, keys prefixed `engine.`) is
//! generated from the `tr!` calls: run `npx tsx tools/engine-strings.ts` after adding or changing one.
//! Placeholders must be named (`{name}`), never positional, so translators can reorder them.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Text {
    /// Stable id without the UI's `engine.` prefix; empty for text that isn't translatable.
    pub key: String,
    /// The English wording, with parameters filled in.
    pub text: String,
    /// Values for the `{placeholders}`. Numbers stay numbers (the UI formats them); a nested Text is translated too.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
}

impl Text {
    pub fn new(key: &str, text: String, params: Vec<(&str, Value)>) -> Text {
        Text { key: key.to_string(), text, params: params.into_iter().map(|(k, v)| (k.to_string(), v)).collect() }
    }

    /// Text with no translation (names, paths, messages from other programs).
    pub fn plain(text: impl Into<String>) -> Text {
        Text { key: String::new(), text: text.into(), params: Map::new() }
    }

    pub fn is_plain(&self) -> bool {
        self.key.is_empty()
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl std::error::Error for Text {}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text::plain(s)
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        Text::plain(s)
    }
}

impl From<&String> for Text {
    fn from(s: &String) -> Text {
        Text::plain(s.as_str())
    }
}

/// A parameter value for `tr!`.
pub fn param<T: Serialize + ?Sized>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// The translatable Text an error was made from, if the error is exactly that (not wrapped in more context).
pub fn text_of(err: &anyhow::Error) -> Option<&Text> {
    // downcast_ref also looks under `.context(…)`; only use the Text when it is what the error displays.
    err.downcast_ref::<Text>().filter(|t| !t.is_plain() && err.to_string() == t.text)
}

/// How an error is sent to the UI: always `message` (English), plus `key`/`params` when it can be translated.
pub fn error_payload(err: &anyhow::Error) -> Value {
    match text_of(err) {
        Some(t) => json!({ "message": t.text, "key": t.key, "params": t.params }),
        None => json!({ "message": err.to_string() }),
    }
}

/// Builds a translatable [`Text`]: `tr!("key", "English {name}", name = value)`.
#[macro_export]
macro_rules! tr {
    ($key:literal, $fmt:literal $(, $name:ident = $val:expr)* $(,)?) => {{
        $( let $name = &$val; )*
        $crate::core::text::Text::new(
            $key,
            format!($fmt $(, $name = $name)*),
            vec![$((stringify!($name), $crate::core::text::param($name))),*],
        )
    }};
}

/// A translatable error: `err_tr!("key", "English {name}", name = value)`.
#[macro_export]
macro_rules! err_tr {
    ($($t:tt)*) => {
        ::anyhow::Error::from($crate::tr!($($t)*))
    };
}

/// Returns a translatable error: `bail_tr!("key", "English {name}", name = value)`.
#[macro_export]
macro_rules! bail_tr {
    ($($t:tt)*) => {
        return Err($crate::err_tr!($($t)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_text_with_params() {
        let name = String::from("Fabric");
        let t = tr!("test.hello", "{name} needs Java {major}.", name = name, major = 21);
        assert_eq!(t.key, "test.hello");
        assert_eq!(t.text, "Fabric needs Java 21.");
        assert_eq!(t.params["name"], json!("Fabric"));
        assert_eq!(t.params["major"], json!(21));
        assert_eq!(tr!("test.plain", "No params").params.len(), 0);
    }

    #[test]
    fn errors_carry_their_key() {
        fn fails() -> anyhow::Result<()> {
            bail_tr!("test.fail", "Couldn't find {what}", what = "level.dat");
        }
        let e = fails().unwrap_err();
        assert_eq!(e.to_string(), "Couldn't find level.dat");
        assert_eq!(error_payload(&e), json!({ "message": "Couldn't find level.dat", "key": "test.fail", "params": { "what": "level.dat" } }));
        // Wrapped in more context, or plain: only the English message.
        let wrapped = fails().map_err(|e| e.context("Import failed")).unwrap_err();
        assert_eq!(error_payload(&wrapped), json!({ "message": "Import failed" }));
        assert_eq!(error_payload(&anyhow::anyhow!("boom")), json!({ "message": "boom" }));
    }

    #[test]
    fn nests_texts() {
        let label = tr!("test.label", "Downloading libraries");
        let t = tr!("test.progress", "{label} ({done}/{total})", label = label, done = 3, total = 10);
        assert_eq!(t.text, "Downloading libraries (3/10)");
        assert_eq!(t.params["label"]["key"], json!("test.label"));
    }
}

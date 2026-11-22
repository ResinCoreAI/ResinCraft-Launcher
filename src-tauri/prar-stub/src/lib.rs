//! Stand-in for PuirkleRAR, whose repository is private. The official builds use the real crate; a build from
//! source without access to it runs `node tools/use-prar-stub.mjs`, which points the engine here. The engine then
//! offers zip backups only (`AVAILABLE` is false) and says so when it meets a PRAR file.
//! Only what `src/core/backup_prar.rs` uses, with the same signatures.

/// False: this is the stand-in.
pub const AVAILABLE: bool = false;

fn missing() -> anyhow::Error {
    anyhow::anyhow!("this build of ResinCraft has no PRAR support")
}

pub mod progress {
    pub trait Progress: Send + Sync {
        fn start(&self, total: u64, msg: &str);
        fn inc(&self, n: u64);
        fn log(&self, line: &str);
        fn detail(&self, _text: &str) {}
        fn grow(&self, _extra: u64) {}
        fn cancelled(&self) -> bool {
            false
        }
    }
}

pub mod util {
    use std::path::PathBuf;

    pub const MAX_COMMENT: usize = 65535;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Level {
        Fast,
        Normal,
        Best,
        Ultra,
        Max,
    }

    #[derive(Clone, Debug)]
    pub struct Entry {
        pub name: String,
        pub path: PathBuf,
        pub is_dir: bool,
        pub size: u64,
        pub mtime_ns: u64,
    }

    pub fn mtime_ns(md: &std::fs::Metadata) -> u64 {
        md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as u64).unwrap_or(0)
    }

    #[derive(Clone, Default, Debug)]
    pub struct Selection {
        pub names: Vec<String>,
        pub base: String,
        pub skip: Vec<String>,
    }

    impl Selection {
        pub fn all() -> Self {
            Self::default()
        }
    }
}

pub mod prar {
    use crate::progress::Progress;
    use crate::util::{Entry, Level, Selection};
    use anyhow::Result;
    use std::path::Path;

    pub fn pack(_archive: &Path, _entries: Vec<Entry>, _level: Level, _password: Option<&str>, _comment: Option<&str>, _p: &dyn Progress) -> Result<()> {
        Err(crate::missing())
    }

    pub fn extract(_archive: &Path, _out_dir: &Path, _password: Option<&str>, _overwrite: bool, _sel: &Selection, _p: &dyn Progress) -> Result<()> {
        Err(crate::missing())
    }

    pub fn comment(_archive: &Path, _password: Option<&str>) -> Result<Option<String>> {
        Err(crate::missing())
    }
}

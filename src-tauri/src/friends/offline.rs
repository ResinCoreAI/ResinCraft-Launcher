//! Offline players in a friend's world. Minecraft always runs an Open to LAN world in online mode, so an offline profile
//! is turned away. With Settings `friends_offline`, a game starts with a small Java agent (java/lan-offline) that turns
//! the check off on its own server and says so in the log; only then does the invite code tell guests offline is fine.
//! It needs Minecraft's real names (26.1 and newer). Nobody's name is checked then, so it's the host's choice.

use std::path::PathBuf;

use anyhow::Result;

const AGENT: &[u8] = include_bytes!("../../java/lan-offline/lan-offline.jar");
/// What the agent prints once a world's server lets offline players in, and when this version can't.
pub const ON_LINE: &str = "[ResinCraft] LAN offline on";
pub const UNSUPPORTED_LINE: &str = "[ResinCraft] LAN offline unsupported";

/// The agent's jar in the cache, written when missing or changed.
fn agent_jar() -> Result<PathBuf> {
    let path = crate::core::paths::cache().join("lan-offline.jar");
    if std::fs::read(&path).ok().as_deref() != Some(AGENT) {
        std::fs::create_dir_all(crate::core::paths::cache())?;
        std::fs::write(&path, AGENT)?;
    }
    Ok(path)
}

/// The JVM argument that loads the agent.
pub fn agent_argument() -> Result<String> {
    Ok(format!("-javaagent:{}", crate::game::mojang::plain(&agent_jar()?)))
}

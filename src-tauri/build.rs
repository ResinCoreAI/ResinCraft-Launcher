fn main() {
    // The headless test programs in examples/ link the same Tauri code as the app. On Windows that code
    // imports TaskDialogIndirect from Common Controls v6, which only loads when the exe's manifest asks
    // for it — tauri_build adds that manifest to the app binary only, so embed it for examples too.
    #[cfg(windows)]
    {
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-examples=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    embed_keys();
    tauri_build::build()
}

/// Compiles the app's service keys in (Microsoft Client ID, CurseForge API key, the ResinCraft Relay address,
/// the optional MineSkin API key for the online skin search, the optional Discord app ID for Rich Presence)
/// so players never have to set them up. They come from environment variables or the git-ignored
/// ../keys.local.env file.
fn embed_keys() {
    const KEYS: [&str; 6] = [
        "RESINCRAFT_MS_CLIENT_ID",
        "RESINCRAFT_CURSEFORGE_KEY",
        "RESINCRAFT_RELAY_URL",
        "RESINCRAFT_RELAY_DOMAIN",
        "RESINCRAFT_MINESKIN_KEY",
        "RESINCRAFT_DISCORD_APP_ID",
    ];
    println!("cargo:rerun-if-changed=../keys.local.env");
    let file = std::fs::read_to_string("../keys.local.env").unwrap_or_default();
    for key in KEYS {
        println!("cargo:rerun-if-env-changed={key}");
        let from_env = std::env::var(key).ok().filter(|v| !v.trim().is_empty());
        let from_file = file
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .find_map(|l| l.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if let Some(value) = from_env.or(from_file) {
            println!("cargo:rustc-env={key}={value}");
        }
    }
}

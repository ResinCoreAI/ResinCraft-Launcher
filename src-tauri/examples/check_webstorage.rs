//! Reads the old http origin's localStorage from a WebView2 `Local Storage/leveldb` folder, as the first start on
//! https does (webstorage.rs).
//!
//!   cargo run --example check_webstorage "<LOCALAPPDATA>/ai.resincore.resincraft/EBWebView/Default/Local Storage/leveldb"
fn main() {
    let dir = std::env::args().nth(1).expect("usage: check_webstorage <leveldb folder>");
    match resincraft_lib::webstorage::read_old(std::path::Path::new(&dir)) {
        Ok(entries) => {
            println!("{} entries on http://tauri.localhost", entries.len());
            for (k, v) in entries {
                println!("  {k} = {}", v.chars().take(80).collect::<String>());
            }
        }
        Err(e) => println!("FAIL {e:#}"),
    }
}

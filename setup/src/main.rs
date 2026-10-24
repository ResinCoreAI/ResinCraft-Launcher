//! ResinCraft Setup: a small installer that always installs the newest ResinCraft Launcher.
//!
//! It reads latest.json from the repository's `updates` branch (the file installed launchers update from), downloads
//! the Windows installer it names, checks it against the update signing key (the same check the launcher's updater
//! does) and runs it. The program itself never changes between releases, so one download link works for every
//! version and Windows SmartScreen keeps the reputation the file has built.
#![windows_subsystem = "windows"]

use std::io::Read;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, Ordering};

use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CreateFontW, GetSysColorBrush, COLOR_WINDOW, DEFAULT_CHARSET, FW_NORMAL};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{PBM_SETPOS, PBM_SETRANGE32, PROGRESS_CLASSW};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const LATEST: &str = "https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/updates/latest.json";
const RELEASES: &str = "https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest";
/// The update signing key (tauri.conf.json, `plugins.updater.pubkey`).
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFEMDk0MTk2RDAzM0UwQUYKUldTdjREUFFsa0VKSFRFa1NFK0dtK3p4TXd0bTRMQlExMmFQK0Jlc3FTUm9NWWtzWW1BSlR4a2kK";

const WM_DONE: u32 = WM_APP + 1;
const WM_FAILED: u32 = WM_APP + 2;

static LABEL: AtomicIsize = AtomicIsize::new(0);
static BAR: AtomicIsize = AtomicIsize::new(0);
static ERROR: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_label(text: &str) {
    let w = wide(text);
    unsafe { SetWindowTextW(LABEL.load(Ordering::Relaxed) as HWND, w.as_ptr()) };
}

fn set_progress(per_mille: u32) {
    unsafe { SendMessageW(BAR.load(Ordering::Relaxed) as HWND, PBM_SETPOS, per_mille as WPARAM, 0) };
}

/// Downloads, checks and saves the newest installer; returns where it is.
fn fetch() -> Result<PathBuf, String> {
    set_label("Looking for the newest version…");
    let latest: serde_json::Value = ureq::get(LATEST)
        .call()
        .map_err(|e| format!("Couldn't reach the download server ({e})."))?
        .into_json()
        .map_err(|_| "The download server sent something unexpected.".to_string())?;
    let version = latest["version"].as_str().unwrap_or("").to_string();
    let platforms = &latest["platforms"];
    let entry = if platforms["windows-x86_64-nsis"].is_object() { &platforms["windows-x86_64-nsis"] } else { &platforms["windows-x86_64"] };
    let (Some(url), Some(signature)) = (entry["url"].as_str(), entry["signature"].as_str()) else {
        return Err("No Windows version is available right now.".into());
    };

    set_label(&format!("Downloading ResinCraft Launcher {version}…"));
    let response = ureq::get(url).call().map_err(|e| format!("The download failed ({e})."))?;
    let total: u64 = response.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut data = Vec::with_capacity(total as usize);
    let mut reader = response.into_reader();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("The download stopped ({e})."))?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if total > 0 {
            set_progress((data.len() as u64 * 1000 / total).min(1000) as u32);
        }
    }

    set_label("Checking the download…");
    verify(&data, signature).map_err(|_| "The download didn't pass the signature check, so it wasn't run. Try again, or download from the website.".to_string())?;

    let name = url.rsplit('/').next().filter(|n| n.to_lowercase().ends_with(".exe")).unwrap_or("ResinCraft-Launcher-setup.exe");
    let dir = std::env::temp_dir().join("ResinCraft Setup");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't save the installer ({e})."))?;
    let path = dir.join(name);
    std::fs::write(&path, &data).map_err(|e| format!("Couldn't save the installer ({e})."))?;
    Ok(path)
}

/// The launcher updater's check: the file must be signed by ResinCraft's update key.
fn verify(data: &[u8], signature_b64: &str) -> Result<(), ()> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let key = String::from_utf8(b64.decode(PUBKEY).map_err(|_| ())?).map_err(|_| ())?;
    let sig = String::from_utf8(b64.decode(signature_b64).map_err(|_| ())?).map_err(|_| ())?;
    let key = PublicKey::decode(&key).map_err(|_| ())?;
    let sig = Signature::decode(&sig).map_err(|_| ())?;
    key.verify(data, &sig, true).map_err(|_| ())
}

fn open(path: &str) {
    let (verb, file) = (wide("open"), wide(path));
    unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DONE => {
            DestroyWindow(hwnd);
            0
        }
        WM_FAILED => {
            let text = format!("{}\n\nOpen the download page instead?", ERROR.lock().unwrap());
            let (t, c) = (wide(&text), wide("ResinCraft Setup"));
            if MessageBoxW(hwnd, t.as_ptr(), c.as_ptr(), MB_YESNO | MB_ICONWARNING) == IDYES {
                open(RELEASES);
            }
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ResinCraftSetup");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: LoadIconW(instance, 1 as _),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);

        // Sized for the screen's scale (the manifest makes the window DPI aware).
        let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForSystem().max(96) as i32;
        let px = |v: i32| v * dpi / 96;
        let (w, h) = (px(440), px(150));
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let title = wide("ResinCraft Setup");
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            (sw - w) / 2,
            (sh - h) / 2,
            w,
            h,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );

        let face = wide("Segoe UI");
        let font = CreateFontW(-px(14), 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, 5, 0, face.as_ptr());
        let empty = wide("");
        let static_class = wide("STATIC");
        let label = CreateWindowExW(0, static_class.as_ptr(), empty.as_ptr(), WS_CHILD | WS_VISIBLE, px(20), px(22), px(390), px(22), hwnd, null_mut(), instance, std::ptr::null());
        SendMessageW(label, WM_SETFONT, font as WPARAM, 1);
        let bar = CreateWindowExW(0, PROGRESS_CLASSW, empty.as_ptr(), WS_CHILD | WS_VISIBLE, px(20), px(56), px(390), px(16), hwnd, null_mut(), instance, std::ptr::null());
        SendMessageW(bar, PBM_SETRANGE32, 0, 1000);
        LABEL.store(label as isize, Ordering::Relaxed);
        BAR.store(bar as isize, Ordering::Relaxed);

        let target = hwnd as isize;
        std::thread::spawn(move || {
            let hwnd = target as HWND;
            match fetch() {
                Ok(path) => {
                    set_label("Starting the installer…");
                    open(&path.to_string_lossy());
                    PostMessageW(hwnd, WM_DONE, 0, 0);
                }
                Err(e) => {
                    *ERROR.lock().unwrap() = e;
                    PostMessageW(hwnd, WM_FAILED, 0, 0);
                }
            }
        });

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against the live update server: the newest Windows installer downloads and passes the signature check, and
    /// a changed byte fails it. Run with `cargo test -- --ignored` (it downloads the installer).
    #[test]
    #[ignore]
    fn newest_installer_passes_and_a_changed_one_fails() {
        let latest: serde_json::Value = ureq::get(LATEST).call().unwrap().into_json().unwrap();
        let entry = &latest["platforms"]["windows-x86_64-nsis"];
        let mut data = Vec::new();
        ureq::get(entry["url"].as_str().unwrap()).call().unwrap().into_reader().read_to_end(&mut data).unwrap();
        let sig = entry["signature"].as_str().unwrap();
        assert!(verify(&data, sig).is_ok());
        data[1000] ^= 1;
        assert!(verify(&data, sig).is_err());
    }
}

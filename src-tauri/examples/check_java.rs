//! Prints which Java each Minecraft Java version would get on this PC, without downloading anything:
//!   cargo run --example check_java
use resincraft_lib::core::ctx;
use resincraft_lib::core::types::JavaInstall;
use resincraft_lib::game::java::{detect_java, find_java_for, host_cpu, resolve_java};

#[tokio::main]
async fn main() {
    ctx::init(std::env::temp_dir().join(format!("resin-java-{}", std::process::id())), None);
    for j in detect_java().await.unwrap() {
        println!("installed: Java {} at {}", j.major, j.path);
    }
    for required in [8, 17, 21, 25] {
        let none: Option<fn() -> std::future::Ready<anyhow::Result<JavaInstall>>> = None;
        let new = resolve_java(required, host_cpu(), none).await.unwrap().map(|j| j.major);
        let old = find_java_for(required, host_cpu()).await.unwrap().map(|j| j.major);
        println!("needs Java {required}: before {old:?}, now (no download) {new:?}");
    }
}

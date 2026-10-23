//! Changes a throwaway instance's Minecraft version and loader, as the instance's Version settings do:
//!   cargo run --example check_setversion

use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, instances};

#[tokio::main]
async fn main() {
    let data = std::env::temp_dir().join(format!("resin-setversion-{}", std::process::id()));
    ctx::init(data.clone(), None);
    let new = NewInstance { name: "Switchy".into(), mc_version: "1.20.1".into(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None };
    let inst = instances::create(new, |_| {}).await.unwrap();

    // Vanilla → Fabric without a version: the newest Fabric for 1.21.1.
    let fabric = instances::set_version(&inst.id, "1.21.1", LoaderType::Fabric, None).await.unwrap();
    println!("fabric: {} {:?} {:?}", fabric.mc_version, fabric.loader, fabric.loader_version);
    // A chosen Forge version for 1.20.1.
    let forge = instances::set_version(&inst.id, "1.20.1", LoaderType::Forge, Some("47.4.10".into())).await.unwrap();
    println!("forge: {} {:?} {:?}", forge.mc_version, forge.loader, forge.loader_version);
    // Back to Vanilla: no loader version left.
    let vanilla = instances::set_version(&inst.id, "1.20.1", LoaderType::Vanilla, Some("47.4.10".into())).await.unwrap();
    println!("vanilla: {} {:?} {:?}", vanilla.mc_version, vanilla.loader, vanilla.loader_version);

    let ok = fabric.mc_version == "1.21.1"
        && fabric.loader == LoaderType::Fabric
        && fabric.loader_version.as_deref().is_some_and(|v| v.starts_with("0."))
        && forge.loader_version.as_deref().is_some_and(|v| v.contains("47.4.10"))
        && vanilla.loader_version.is_none()
        && instances::get(&inst.id).await.unwrap().loader == LoaderType::Vanilla;
    let _ = std::fs::remove_dir_all(&data);
    println!("{}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}

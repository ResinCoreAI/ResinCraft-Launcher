// Puts the icon, the version info and the manifest (modern controls, sharp on high-DPI screens) into the exe.
fn main() {
    println!("cargo:rerun-if-changed=setup.rc");
    println!("cargo:rerun-if-changed=setup.manifest");
    embed_resource::compile("setup.rc", embed_resource::NONE).manifest_required().unwrap();
}

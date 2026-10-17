//! Parity check, Rust side: the same read-only requests as scripts/parity-ts.ts, sent through the real
//! IPC router, printed as JSON.   cargo run --example parity -- <instanceId> > rust.json
use resincraft_lib::core::{ctx, paths, router, Args};
use serde_json::{json, Map, Value};

#[tokio::main]
async fn main() {
    ctx::init(paths::default_data_dir(None), None);
    let id = std::env::args().nth(1).expect("instance id");
    let calls: Vec<(&str, &str, Vec<Value>, Option<usize>)> = vec![
        ("settings:get", "settings:get", vec![], None),
        ("instances:list", "instances:list", vec![], None),
        ("versions:list", "versions:list", vec![], Some(3)),
        ("versions:loaders fabric", "versions:loaders", vec![json!("fabric"), json!("1.21.1")], Some(3)),
        ("versions:loaders neoforge", "versions:loaders", vec![json!("neoforge"), json!("1.21.1")], Some(3)),
        ("java:list", "java:list", vec![], None),
        ("accounts:list", "accounts:list", vec![], None),
        ("content:search modrinth", "content:search", vec![json!({"query": "sodium", "kind": "mod", "limit": 3})], None),
        ("content:search ftb", "content:search", vec![json!({"query": "", "kind": "modpack", "source": "ftb", "limit": 2})], None),
        ("content:versions", "content:versions", vec![json!("AANobbMI")], Some(2)),
        ("content:list", "content:list", vec![json!(id), json!("mod")], None),
        ("content:exportPreview", "content:exportPreview", vec![json!(id)], None),
        ("worlds:list", "worlds:list", vec![json!(id)], None),
        ("servers:list", "servers:list", vec![json!(id)], None),
        ("screenshots:list", "screenshots:list", vec![json!(id)], None),
        ("notes:get", "notes:get", vec![json!(id)], None),
    ];
    let mut out = Map::new();
    for (name, channel, args, take) in calls {
        let v = match router::dispatch(channel, Args::new(args)).await {
            Ok(Value::Array(a)) if take.is_some() => Value::Array(a.into_iter().take(take.unwrap()).collect()),
            Ok(v) => v,
            Err(e) => json!({ "__error": e.to_string() }),
        };
        out.insert(name.to_string(), v);
    }
    print!("{}", Value::Object(out));
}

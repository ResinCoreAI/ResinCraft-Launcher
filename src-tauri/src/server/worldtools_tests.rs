//! Generated region and player files, nothing from a real server except in `real_world_copy` (see there).
use std::io::Write as _;

use flate2::write::{GzEncoder, ZlibEncoder};

use super::*;
use crate::player::nbt::Compound;

struct Temp(PathBuf);

impl Temp {
    fn new() -> Temp {
        let p = std::env::temp_dir().join(format!("resin-worldtools-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }
    fn file(&self, rel: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, data).unwrap();
        p
    }
    fn dir(&self, rel: &str) {
        std::fs::create_dir_all(self.0.join(rel)).unwrap();
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A region with its chunks stored out of order, gaps between them and a recognisable pattern in each.
fn region(chunks: &[(usize, usize, usize)]) -> Vec<u8> {
    let mut out = vec![0u8; HEADER];
    let order = chunks.iter().rev().enumerate();
    for (n, &(lx, lz, sectors)) in order {
        let i = lx + lz * 32;
        if n % 3 == 2 {
            out.extend(vec![0xee; SECTOR]);
        }
        let offset = out.len() / SECTOR;
        let loc = ((offset as u32) << 8) | sectors as u32;
        out[i * 4..i * 4 + 4].copy_from_slice(&loc.to_be_bytes());
        out[SECTOR + i * 4..SECTOR + i * 4 + 4].copy_from_slice(&(1000 + i as u32).to_be_bytes());
        let mut body = vec![(i % 251) as u8 + 1; sectors * SECTOR];
        body[..4].copy_from_slice(&((sectors * SECTOR - 4) as u32).to_be_bytes());
        body[4] = 2;
        out.extend(body);
    }
    out
}

fn entry(file: &[u8], lx: usize, lz: usize) -> (u32, u32) {
    let i = lx + lz * 32;
    (location(file, i), u32::from_be_bytes(file[SECTOR + i * 4..SECTOR + i * 4 + 4].try_into().unwrap()))
}

fn body(file: &[u8], loc: u32) -> &[u8] {
    let (o, n) = ((loc >> 8) as usize * SECTOR, (loc & 0xff) as usize * SECTOR);
    &file[o..o + n]
}

fn rect(x1: i32, z1: i32, x2: i32, z2: i32) -> Rect {
    Rect::of(Area { x1, z1, x2, z2, blocks: false }).unwrap()
}

#[test]
fn clearing_changes_only_the_8_header_bytes_of_each_chunk() {
    let t = Temp::new();
    let chunks = [(0, 0, 1), (1, 0, 2), (2, 0, 3), (0, 1, 1), (1, 1, 4), (31, 31, 2), (5, 7, 1)];
    let original = region(&chunks);
    let path = t.file("r.0.0.mca", &original);

    assert_eq!(clear_chunks(&path, 0, 0, &rect(1, 0, 2, 1)).unwrap(), 3); // (1,0) (2,0) (1,1); (2,1) isn't saved.

    let after = std::fs::read(&path).unwrap();
    assert_eq!(after.len(), original.len(), "the file keeps its size");
    assert_eq!(after[HEADER..], original[HEADER..], "no data sector changed");
    let cleared = [(1, 0), (2, 0), (1, 1)];
    for &(lx, lz) in &cleared {
        assert_eq!(entry(&after, lx, lz), (0, 0));
    }
    for byte in 0..HEADER {
        let slot = (byte % SECTOR) / 4;
        if !cleared.iter().any(|&(x, z)| x + z * 32 == slot) {
            assert_eq!(after[byte], original[byte], "header byte {byte} belongs to another chunk");
        }
    }
    // Each neighbour's offset still lands on its own bytes.
    for &(lx, lz, _) in chunks.iter().filter(|&&(x, z, _)| !cleared.contains(&(x, z))) {
        let i = lx + lz * 32;
        let (loc, stamp) = entry(&after, lx, lz);
        assert_eq!(stamp, 1000 + i as u32);
        assert!(body(&after, loc)[5..].iter().all(|b| *b == (i % 251) as u8 + 1));
    }
    check_header(&after, after.len() as u64, "r.0.0.mca").unwrap();
}

#[test]
fn nothing_in_the_area_leaves_the_file_as_it_was() {
    let t = Temp::new();
    let original = region(&[(0, 0, 1)]);
    let path = t.file("r.0.0.mca", &original);
    assert_eq!(clear_chunks(&path, 0, 0, &rect(5, 5, 6, 6)).unwrap(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn blocks_round_down_to_chunks_on_the_negative_side() {
    let of = |x1, z1, x2, z2| Rect::of(Area { x1, z1, x2, z2, blocks: true }).unwrap();
    assert_eq!(of(-1, -1, -1, -1), rect(-1, -1, -1, -1));
    assert_eq!(of(-16, 0, -1, 15), rect(-1, 0, -1, 0));
    assert_eq!(of(-17, 16, 0, 31), rect(-2, 1, 0, 1));
    // Corners in any order.
    assert_eq!(of(31, 31, -17, -17), rect(-2, -2, 1, 1));
    assert_eq!(rect(5, -3, -5, 3), Rect { x1: -5, z1: -3, x2: 5, z2: 3 });
    assert!(Rect::of(Area { x1: 30_000_001, z1: 0, x2: 0, z2: 0, blocks: true }).is_err());
    assert!(Rect::of(Area { x1: 1_875_001, z1: 0, x2: 0, z2: 0, blocks: false }).is_err());
    // i32::MIN has no abs(): this must refuse, not panic.
    assert!(Rect::of(Area { x1: i32::MIN, z1: 0, x2: 0, z2: 0, blocks: true }).is_err());
}

#[test]
fn a_rectangle_across_four_regions_finds_the_right_slots() {
    let t = Temp::new();
    // Chunk (-1, -1) is the last slot of r.-1.-1; chunk (0, 0) the first of r.0.0.
    let files = [("r.-1.-1.mca", (31, 31, 1)), ("r.0.-1.mca", (0, 31, 1)), ("r.-1.0.mca", (31, 0, 1)), ("r.0.0.mca", (0, 0, 1))];
    for (name, c) in files {
        // Each keeps a chunk that's outside the rectangle.
        let keep = if c.0 == 0 { (1, c.1, 1) } else { (30, c.1, 1) };
        t.file(&format!("dim/region/{name}"), &region(&[c, keep]));
    }
    t.file("dim/region/r.5.5.mca", &region(&[(0, 0, 1)]));
    let count = reset_dimension(&t.0.join("dim"), &rect(-1, -1, 0, 0)).unwrap();
    assert_eq!((count.chunks, count.regions, count.width, count.depth), (4, 4, 32, 32));
    for (name, c) in files {
        let after = std::fs::read(t.0.join("dim/region").join(name)).unwrap();
        assert_eq!(entry(&after, c.0, c.1), (0, 0), "{name}");
        let keep = if c.0 == 0 { (1, c.1) } else { (30, c.1) };
        assert_ne!(entry(&after, keep.0, keep.1).0, 0, "{name}");
    }
    let far = std::fs::read(t.0.join("dim/region/r.5.5.mca")).unwrap();
    assert_ne!(entry(&far, 0, 0).0, 0);
}

#[test]
fn entities_poi_and_external_chunks_go_with_the_terrain() {
    let t = Temp::new();
    for folder in ["region", "entities", "poi"] {
        t.file(&format!("d/{folder}/r.0.0.mca"), &region(&[(1, 1, 1), (9, 9, 1)]));
    }
    t.file("d/region/c.1.1.mcc", b"big");
    t.file("d/region/c.9.9.mcc", b"keep");
    t.file("d/region/r.0.0_oversized_1_1.nbt", b"paper");
    let count = reset_dimension(&t.0.join("d"), &rect(0, 0, 2, 2)).unwrap();
    assert_eq!((count.chunks, count.entities, count.poi, count.external), (1, 1, 1, 1));
    for folder in ["region", "entities", "poi"] {
        let after = std::fs::read(t.0.join(format!("d/{folder}/r.0.0.mca"))).unwrap();
        assert_eq!(entry(&after, 1, 1), (0, 0));
        assert_ne!(entry(&after, 9, 9).0, 0);
    }
    assert!(!t.0.join("d/region/c.1.1.mcc").exists());
    assert!(t.0.join("d/region/c.9.9.mcc").exists());
}

#[test]
fn old_worlds_without_entities_or_poi_folders_still_reset() {
    let t = Temp::new();
    t.file("d/region/r.0.0.mca", &region(&[(1, 1, 1)]));
    assert_eq!(reset_dimension(&t.0.join("d"), &rect(1, 1, 1, 1)).unwrap().chunks, 1);
}

#[test]
fn a_damaged_header_is_refused_and_nothing_at_all_changes() {
    let mut beyond = region(&[(0, 0, 1)]);
    beyond[0..4].copy_from_slice(&((90u32 << 8) | 1).to_be_bytes());
    let mut into_header = region(&[(0, 0, 1)]);
    into_header[0..4].copy_from_slice(&((1u32 << 8) | 1).to_be_bytes());
    let mut overlap = region(&[(0, 0, 2), (1, 0, 1)]);
    let loc = location(&overlap, 0);
    overlap[4..8].copy_from_slice(&loc.to_be_bytes());
    let mut no_length = region(&[(0, 0, 1)]);
    no_length[3] = 0;
    for bad in [beyond, into_header, overlap, no_length] {
        let t = Temp::new();
        let good = region(&[(2, 2, 1)]);
        // Sorted so the good one may come first: it must stay as it was all the same.
        t.file("d/region/r.-1.0.mca", &good);
        t.file("d/region/r.0.0.mca", &bad);
        let err = reset_dimension(&t.0.join("d"), &rect(-32, 0, 31, 31)).unwrap_err();
        assert!(err.to_string().contains("damaged header"), "{err}");
        assert_eq!(std::fs::read(t.0.join("d/region/r.0.0.mca")).unwrap(), bad);
        assert_eq!(std::fs::read(t.0.join("d/region/r.-1.0.mca")).unwrap(), good);
    }
}

#[test]
fn a_last_sector_cut_short_is_still_a_region() {
    let mut file = region(&[(0, 0, 1)]);
    file.truncate(HEADER + 100);
    check_header(&file, file.len() as u64, "r").unwrap();
}

#[test]
fn empty_regions_are_fine_and_short_ones_are_refused() {
    let t = Temp::new();
    t.file("d/region/r.0.0.mca", b"");
    assert_eq!(reset_dimension(&t.0.join("d"), &rect(0, 0, 3, 3)).unwrap(), ChunkCount { width: 64, depth: 64, ..ChunkCount::default() });
    t.file("d/region/r.0.0.mca", &[0u8; 100]);
    assert!(reset_dimension(&t.0.join("d"), &rect(0, 0, 3, 3)).unwrap_err().to_string().contains("shorter"));
}

#[test]
fn formats_it_cant_edit_are_refused() {
    for (name, why) in [("r.0.0.linear", "isn't an Anvil"), ("r.0.0.b_linear", "isn't an Anvil"), ("r.0.0.sf", "isn't an Anvil"), ("r.0.0.mcr", "McRegion"), ("x.mca", "isn't a region file")] {
        let t = Temp::new();
        let f = t.file(&format!("d/region/{name}"), b"data");
        let err = reset_dimension(&t.0.join("d"), &rect(0, 0, 1, 1)).unwrap_err();
        assert!(err.to_string().contains(why), "{name}: {err}");
        assert_eq!(std::fs::read(f).unwrap(), b"data");
    }
}

#[test]
fn an_old_mcr_beside_converted_regions_is_left_alone() {
    let t = Temp::new();
    t.file("d/region/r.0.0.mcr", b"old");
    t.file("d/region/r.0.0.mca", &region(&[(0, 0, 1)]));
    assert_eq!(reset_dimension(&t.0.join("d"), &rect(0, 0, 0, 0)).unwrap().chunks, 1);
}

// ---------- Layouts ----------

fn paths(dims: &[Dimension]) -> Vec<&str> {
    dims.iter().map(|d| d.path.as_str()).collect()
}

fn level_dat(data_version: i32) -> Vec<u8> {
    let mut data = Compound::new();
    data.set("version", Tag::Int(ANVIL as i32));
    data.set("DataVersion", Tag::Int(data_version));
    let mut root = Compound::new();
    root.set("Data", Tag::Compound(data));
    nbt::write(&NbtFile { name: String::new(), root, compression: Compression::Gzip }).unwrap()
}

#[test]
fn vanilla_finds_dim_folders_inside_the_world() {
    let t = Temp::new();
    for d in ["world/region", "world/DIM-1/region", "world/DIM1/region"] {
        t.dir(d);
    }
    let dims = find_dimensions(&t.0, "world", false);
    assert_eq!(paths(&dims), ["world", "world/DIM-1", "world/DIM1"]);
    assert_eq!(dims[1].name, "minecraft:the_nether");
    assert_eq!(dims[2].name, "minecraft:the_end");
}

#[test]
fn paper_finds_its_separate_folders() {
    let t = Temp::new();
    for d in ["world/region", "world_nether/DIM-1/region", "world_the_end/DIM1/region"] {
        t.dir(d);
    }
    assert_eq!(paths(&find_dimensions(&t.0, "world", true)), ["world", "world_nether/DIM-1", "world_the_end/DIM1"]);
}

#[test]
fn a_dimension_in_two_places_lists_both_with_the_softwares_own_first() {
    let t = Temp::new();
    for d in ["world/region", "world/DIM-1/region", "world_nether/DIM-1/region"] {
        t.dir(d);
    }
    assert_eq!(paths(&find_dimensions(&t.0, "world", true))[1..], ["world_nether/DIM-1", "world/DIM-1"]);
    assert_eq!(paths(&find_dimensions(&t.0, "world", false))[1..], ["world/DIM-1", "world_nether/DIM-1"]);
}

#[test]
fn minecraft_26_keeps_dimensions_in_their_own_folder() {
    let t = Temp::new();
    for d in ["w/dimensions/minecraft/overworld/region", "w/dimensions/minecraft/the_nether/region", "w/dimensions/minecraft/the_end/region", "w/dimensions/mymod/caves/deep/region", "w/dimensions/minecraft/overworld/data"] {
        t.dir(d);
    }
    let dims = find_dimensions(&t.0, "w", true);
    assert_eq!(paths(&dims), ["w/dimensions/minecraft/overworld", "w/dimensions/minecraft/the_nether", "w/dimensions/minecraft/the_end", "w/dimensions/mymod/caves/deep"]);
    assert_eq!(dims[3].name, "mymod:caves/deep");
}

#[test]
fn forge_numbered_dimensions_are_found() {
    let t = Temp::new();
    t.dir("world/region");
    t.dir("world/DIM7/region");
    assert_eq!(find_dimensions(&t.0, "world", false)[1], Dimension { path: "world/DIM7".into(), name: "dimension 7".into() });
}

#[test]
fn worlds_keep_their_nether_and_end_with_them() {
    let t = Temp::new();
    for w in ["world", "world_nether", "world_the_end", "other", "lonely_nether"] {
        t.file(&format!("{w}/level.dat"), &level_dat(4903));
    }
    t.dir("plugins/Thing/region");
    t.dir(".paper");
    t.file("world/players/data/a.dat", b"x");
    let worlds = scan_worlds(&t.0, "world", true);
    let names: Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["lonely_nether", "other", "world"]);
    let world = worlds.iter().find(|w| w.name == "world").unwrap();
    assert!(world.active && world.generated);
    assert_eq!(world.folders, ["world", "world_nether", "world_the_end"]);
    assert_eq!((world.players, world.data_version), (1, Some(4903)));
    assert!(world.size > 0);
}

#[test]
fn the_world_not_made_yet_is_still_listed() {
    let t = Temp::new();
    t.file("world/level.dat", &level_dat(1));
    let worlds = scan_worlds(&t.0, "fresh", false);
    assert_eq!((worlds[0].name.as_str(), worlds[0].active, worlds[0].generated), ("fresh", true, false));
    assert!(!worlds[1].active);
}

#[test]
fn bedrock_and_mcregion_worlds_are_refused() {
    let t = Temp::new();
    t.file("bedrock/level.dat", &[9, 0, 0, 0, 100, 0, 0, 0, 10, 0, 0]);
    t.dir("bedrock/db");
    // A Bedrock level.dat on its own: little-endian, with no way to read it as NBT.
    t.file("broken/level.dat", &[9, 0, 0, 0, 100, 0, 0, 0, 10, 0, 0]);
    let mut data = Compound::new();
    data.set("version", Tag::Int(19132));
    let mut root = Compound::new();
    root.set("Data", Tag::Compound(data));
    t.file("oldest/level.dat", &nbt::write(&NbtFile { name: String::new(), root, compression: Compression::Gzip }).unwrap());
    let worlds = scan_worlds(&t.0, "world", false);
    let problem = |n: &str| worlds.iter().find(|w| w.name == n).unwrap().problem.clone().unwrap();
    assert!(problem("bedrock").contains("Bedrock"));
    assert!(problem("broken").contains("can't be read"));
    assert!(problem("oldest").contains("older format"));
    assert!(worlds.iter().find(|w| w.name == "bedrock").unwrap().dimensions.is_empty());
    assert!(usable(worlds.iter().find(|w| w.name == "oldest").unwrap().clone()).is_err());
}

#[test]
fn servers_without_java_worlds_are_refused() {
    for kind in [ServerKind::Bedrock, ServerKind::Pocketmine, ServerKind::Nukkit, ServerKind::Velocity, ServerKind::Bungeecord, ServerKind::Glowstone, ServerKind::Cuberite, ServerKind::Pumpkin, ServerKind::Horizon] {
        assert!(unsupported(kind).is_some(), "{kind:?}");
    }
    for kind in [ServerKind::Vanilla, ServerKind::Paper, ServerKind::Fabric, ServerKind::Forge, ServerKind::Neoforge] {
        assert!(unsupported(kind).is_none(), "{kind:?}");
    }
}

#[test]
fn world_names_stay_plain() {
    for good in ["world", "Survival 2", "smp-2026", "a_b"] {
        check_world_name(good).unwrap();
    }
    for bad in ["", "../x", "a/b", "a\\b", "a:b", "wörld", " x", "x ", "-x", "plugins", "Mods", "CON", "lpt1", "x_nether", "x_the_end", &"a".repeat(65)] {
        assert!(check_world_name(bad).is_err(), "{bad}");
    }
}

// ---------- Players ----------

fn gzip(raw: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(raw).unwrap();
    e.finish().unwrap()
}

/// A player file as the game writes it, with things an editor must not disturb.
fn player_file(compression: Compression) -> NbtFile {
    let mut root = Compound::new();
    root.set("DataVersion", Tag::Int(4903));
    root.set("Pos", Tag::List(nbt::DOUBLE, vec![Tag::Double(0.1), Tag::Double(64.0), Tag::Double(-0.3)]));
    root.set("Dimension", Tag::String("minecraft:the_nether".into()));
    root.set("playerGameType", Tag::Int(0));
    root.set("Health", Tag::Float(19.5));
    root.set("foodLevel", Tag::Int(20));
    root.set("XpLevel", Tag::Int(3));
    root.set("XpP", Tag::Float(0.25));
    root.set("XpTotal", Tag::Int(40));
    root.set("seenCredits", Tag::Byte(1));
    root.set("Air", Tag::Short(300));
    root.set("Rotation", Tag::List(nbt::FLOAT, vec![Tag::Float(65.519226), Tag::Float(9.999786)]));
    root.set("UUID", Tag::IntArray(vec![1, -2, 3, -4]));
    root.set("Heights", Tag::LongArray(vec![i64::MIN, i64::MAX]));
    root.set("Bytes", Tag::ByteArray(vec![-1, 0, 1]));
    root.set("Empty typed", Tag::List(nbt::COMPOUND, Vec::new()));
    root.set("Empty", Tag::List(nbt::END, Vec::new()));
    root.set("Unicode", Tag::String("Puirkle \u{1F40D} é".into()));
    let mut bukkit = Compound::new();
    bukkit.set("lastKnownName", Tag::String("Puirkle".into()));
    root.set("bukkit", Tag::Compound(bukkit));
    NbtFile { name: String::new(), root, compression }
}

fn bytes_of(c: Compression) -> Vec<u8> {
    nbt::write(&player_file(c)).unwrap()
}

#[test]
fn gzip_round_trip_is_exact() {
    let data = bytes_of(Compression::Gzip);
    assert_eq!(&data[..2], [0x1f, 0x8b]);
    let parsed = read_exact_nbt(&data).unwrap();
    assert_eq!(parsed, player_file(Compression::Gzip));
    let plain = NbtFile { compression: Compression::None, ..parsed.clone() };
    assert_eq!(nbt::write(&plain).unwrap(), inflate(&data, Compression::Gzip).unwrap());
    assert_eq!(nbt::parse(&nbt::write(&parsed).unwrap()).unwrap(), parsed);
}

#[test]
fn an_empty_list_keeps_its_element_type() {
    let parsed = read_exact_nbt(&bytes_of(Compression::Gzip)).unwrap();
    assert_eq!(parsed.root.get("Empty typed"), Some(&Tag::List(nbt::COMPOUND, Vec::new())));
}

#[test]
fn an_edit_changes_only_the_fields_it_names() {
    let data = bytes_of(Compression::Gzip);
    let edit = PlayerEdit { pos: Some([10.5, 70.0, -20.25]), game_mode: Some(1), health: Some(7.5), food: Some(3), xp_level: Some(30), xp_progress: Some(0.5), xp_total: Some(1395) };
    let out = edit_player(&data, &edit).unwrap();
    assert_eq!(&out[..2], [0x1f, 0x8b], "stays gzipped");
    let after = read_exact_nbt(&out).unwrap();

    let mut want = player_file(Compression::Gzip);
    want.root.set("Pos", Tag::List(nbt::DOUBLE, vec![Tag::Double(10.5), Tag::Double(70.0), Tag::Double(-20.25)]));
    want.root.set("playerGameType", Tag::Int(1));
    want.root.set("Health", Tag::Float(7.5));
    want.root.set("foodLevel", Tag::Int(3));
    want.root.set("XpLevel", Tag::Int(30));
    want.root.set("XpP", Tag::Float(0.5));
    want.root.set("XpTotal", Tag::Int(1395));
    assert_eq!(after, want, "same keys, same order, same types");
    let order = |f: &NbtFile| f.root.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    assert_eq!(order(&after), order(&player_file(Compression::Gzip)));
}

#[test]
fn an_empty_edit_keeps_every_byte() {
    let data = bytes_of(Compression::Gzip);
    let out = edit_player(&data, &PlayerEdit::default()).unwrap();
    assert_eq!(inflate(&out, Compression::Gzip).unwrap(), inflate(&data, Compression::Gzip).unwrap());
}

#[test]
fn an_edit_keeps_the_type_and_compression_it_found() {
    let mut old = player_file(Compression::Zlib);
    old.root.set("Health", Tag::Short(20));
    old.root.set("playerGameType", Tag::Byte(0));
    let out = edit_player(&nbt::write(&old).unwrap(), &PlayerEdit { health: Some(5.0), game_mode: Some(2), ..PlayerEdit::default() }).unwrap();
    let after = nbt::parse(&out).unwrap();
    assert_eq!(after.compression, Compression::Zlib);
    assert_eq!(after.root.get("Health"), Some(&Tag::Short(5)));
    assert_eq!(after.root.get("playerGameType"), Some(&Tag::Byte(2)));
    let out = edit_player(&bytes_of(Compression::None), &PlayerEdit { food: Some(1), ..PlayerEdit::default() }).unwrap();
    assert_eq!(nbt::parse(&out).unwrap().compression, Compression::None);
    let _ = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
}

#[test]
fn bad_values_and_missing_fields_are_refused() {
    let data = bytes_of(Compression::Gzip);
    let bad = [
        PlayerEdit { game_mode: Some(4), ..PlayerEdit::default() },
        PlayerEdit { game_mode: Some(-1), ..PlayerEdit::default() },
        PlayerEdit { health: Some(-1.0), ..PlayerEdit::default() },
        PlayerEdit { health: Some(f64::NAN), ..PlayerEdit::default() },
        PlayerEdit { food: Some(21), ..PlayerEdit::default() },
        PlayerEdit { xp_progress: Some(1.5), ..PlayerEdit::default() },
        PlayerEdit { xp_level: Some(-1), ..PlayerEdit::default() },
        PlayerEdit { pos: Some([30_000_001.0, 0.0, 0.0]), ..PlayerEdit::default() },
        PlayerEdit { pos: Some([0.0, f64::INFINITY, 0.0]), ..PlayerEdit::default() },
    ];
    for edit in bad {
        assert!(edit_player(&data, &edit).is_err(), "{edit:?}");
    }
    let mut file = player_file(Compression::Gzip);
    file.root = Compound::new();
    file.root.set("DataVersion", Tag::Int(1));
    let err = edit_player(&nbt::write(&file).unwrap(), &PlayerEdit { xp_level: Some(1), ..PlayerEdit::default() }).unwrap_err();
    assert!(err.to_string().contains("no XpLevel"));
}

#[test]
fn files_it_cant_keep_exactly_are_refused() {
    let raw = nbt::write(&NbtFile { compression: Compression::None, ..player_file(Compression::None) }).unwrap();
    let mut trailing = raw.clone();
    trailing.extend_from_slice(b"extra");
    assert!(read_exact_nbt(&gzip(&trailing)).is_err());
    // A lone surrogate: Rust's strings can't hold it, so reading it would change it.
    let mut lone = vec![nbt::COMPOUND, 0, 0, nbt::STRING, 0, 1, b'a', 0, 3, 0xed, 0xa0, 0x80, nbt::END];
    assert!(read_exact_nbt(&lone).is_err());
    lone[9] = b'o';
    lone[10] = b'k';
    lone[11] = b'!';
    assert!(read_exact_nbt(&lone).is_ok());
    assert!(edit_player(b"not nbt at all", &PlayerEdit::default()).is_err());
    assert!(edit_player(&[0x1f, 0x8b, 1, 2, 3], &PlayerEdit::default()).is_err());
}

#[test]
fn usercache_names_players_and_bukkit_fills_the_gaps() {
    let t = Temp::new();
    let uuid = "ce455cc5-9923-4368-885e-209cecf5151b";
    t.file("usercache.json", format!(r#"[{{"uuid":"{}","name":"Cached","expiresOn":"2026-11-24 09:56:30 -0800"}}]"#, uuid.to_uppercase()).as_bytes());
    t.file(&format!("world/players/data/{uuid}.dat"), &bytes_of(Compression::Gzip));
    t.file("world/players/data/00000000-0000-0000-0009-01fe91d7be47.dat", &bytes_of(Compression::Gzip));
    t.file("world/players/data/00000000-0000-0000-0009-01fe91d7be47.dat_old", b"old");
    t.file("world/players/data/broken.dat", b"nope");
    let list = list_players(&t.0, "world");
    assert_eq!(list.len(), 3, "the .dat_old isn't a player");
    assert_eq!(list[0].name.as_deref(), Some("Cached"));
    assert_eq!(list[1].name.as_deref(), Some("Puirkle"));
    assert_eq!(list[1].pos, Some([0.1, 64.0, -0.3]));
    assert_eq!(list[1].dimension.as_deref(), Some("minecraft:the_nether"));
    assert_eq!((list[1].game_mode, list[1].health, list[1].xp_level), (Some(0), Some(19.5), Some(3)));
    assert_eq!(list[2].uuid, "broken");
    assert!(list[2].error.is_some());
}

#[test]
fn older_worlds_keep_players_in_playerdata() {
    let t = Temp::new();
    t.file("world/playerdata/a.dat", &bytes_of(Compression::Gzip));
    assert_eq!(list_players(&t.0, "world").len(), 1);
    assert_eq!(list_players(&t.0, "missing").len(), 0);
}

#[test]
fn a_bad_usercache_only_loses_names() {
    assert!(user_names("not json").is_empty());
    assert!(user_names("").is_empty());
}

// ---------- The rest ----------

#[test]
fn one_tool_at_a_time_and_no_start_meanwhile() {
    let held = claim("busy-test").unwrap();
    assert!(claim("busy-test").is_err());
    assert!(check_free("busy-test").is_err());
    check_free("another-server").unwrap();
    drop(held);
    check_free("busy-test").unwrap();
}

#[test]
fn the_web_console_cant_reach_these() {
    let (remote, other_pc) = (include_str!("../remote.rs"), include_str!("../../../src/renderer/src/remotePc.ts"));
    for call in ["worldTools", "worldSwitch", "worldCreate", "worldDelete", "chunkPreview", "chunkReset", "worldPlayers", "worldPlayerSave"] {
        assert!(!remote.contains(call), "{call} in remote.rs");
        assert!(!other_pc.contains(call), "{call} in remotePc.ts");
    }
}

/// On a scratch copy of a real server folder: RESIN_WORLD_SCRATCH=<folder> cargo test real_world_copy -- --ignored.
/// The caller compares the result with an untouched copy (region data, neighbours, player files).
#[test]
#[ignore]
fn real_world_copy() {
    let dir = PathBuf::from(std::env::var("RESIN_WORLD_SCRATCH").expect("RESIN_WORLD_SCRATCH"));
    let worlds = scan_worlds(&dir, "world", true);
    println!("{}", serde_json::to_string_pretty(&worlds).unwrap());
    let world = worlds.iter().find(|w| w.active).unwrap();
    assert!(world.problem.is_none());
    let rect = Rect::of(Area { x1: -40, z1: -40, x2: 40, z2: 40, blocks: true }).unwrap();
    for d in &world.dimensions {
        let count = reset_dimension(&dir.join(&d.path), &rect).unwrap();
        println!("{} {count:?}", d.path);
    }
    let players = list_players(&dir, &world.name);
    println!("{players:#?}");
    for p in &players {
        assert!(p.error.is_none(), "{:?}", p.error);
    }
    let path = player_dir(&dir.join(&world.name)).unwrap().join(&players[0].file);
    let edit = PlayerEdit { pos: Some([1.5, 100.0, -2.5]), game_mode: Some(2), health: Some(11.5), food: Some(7), xp_level: Some(12), xp_progress: Some(0.5), xp_total: Some(200) };
    let out = edit_player(&std::fs::read(&path).unwrap(), &edit).unwrap();
    std::fs::write(&path, out).unwrap();
    println!("edited {}", players[0].file);
}

/// The whole path on a scratch copy of a server: RESIN_WORLD_E2E=<games folder holding servers/<id>/server.json>,
/// RESIN_WORLD_ID=<id>. Runs the same requests the page sends, backups and the Recycle Bin included.
#[test]
#[ignore]
fn real_requests_on_a_scratch_server() {
    let games = PathBuf::from(std::env::var("RESIN_WORLD_E2E").expect("RESIN_WORLD_E2E"));
    let id = std::env::var("RESIN_WORLD_ID").expect("RESIN_WORLD_ID");
    crate::core::ctx::init(games.clone(), None);
    crate::core::paths::set_games(Some(games.clone()));
    let dir = games.join("servers").join(&id);
    let call = |channel: &'static str, args: Vec<Value>| {
        let id = id.clone();
        async move {
            let mut all = vec![json!(id)];
            all.extend(args);
            handle(channel, Args::new(all)).await
        }
    };
    let zips = |dir: &Path| std::fs::read_dir(dir.join("backups")).map(|d| d.count()).unwrap_or(0);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let info = call("server:worldTools", vec![]).await.unwrap();
        println!("info {info}");
        assert_eq!(info["active"], "world");
        let dimension = info["worlds"][0]["dimensions"][1]["path"].as_str().unwrap().to_string();
        let area = json!({ "x1": -40, "z1": -40, "x2": 40, "z2": 40, "blocks": true });
        let target = json!({ "world": "world", "dimension": dimension, "area": area });
        let preview = call("server:chunkPreview", vec![target.clone()]).await.unwrap();
        println!("preview {preview}");
        assert!(preview["chunks"].as_u64().unwrap() > 0);

        // A folder that isn't one of the world's dimensions.
        let bad = json!({ "world": "world", "dimension": "../..", "area": area, "backup": false });
        assert!(call("server:chunkReset", vec![bad]).await.is_err());

        let before = zips(&dir);
        let mut reset = target.clone();
        reset["backup"] = json!(true);
        let done = call("server:chunkReset", vec![reset]).await.unwrap();
        println!("reset {done}");
        assert_eq!(done["chunks"], preview["chunks"]);
        assert_eq!(zips(&dir), before + 1, "a backup came first");
        let mut again = target.clone();
        again["backup"] = json!(false);
        assert!(call("server:chunkReset", vec![again]).await.is_err(), "nothing left to reset");

        let players = call("server:worldPlayers", vec![json!("world")]).await.unwrap();
        println!("players {players}");
        let file = players[0]["file"].as_str().unwrap().to_string();
        let save = json!({ "world": "world", "file": file, "edit": { "health": 5.5, "gameMode": 3 }, "backup": true });
        call("server:worldPlayerSave", vec![save]).await.unwrap();
        let after = call("server:worldPlayers", vec![json!("world")]).await.unwrap();
        assert_eq!((after[0]["health"].as_f64(), after[0]["gameMode"].as_i64()), (Some(5.5), Some(3)));
        let sneaky = json!({ "world": "world", "file": "../level.dat", "edit": {}, "backup": false });
        assert!(call("server:worldPlayerSave", vec![sneaky]).await.is_err());

        call("server:worldCreate", vec![json!({ "name": "second", "seed": "123", "backup": true })]).await.unwrap();
        let props = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        assert!(props.contains("level-name=second") && props.contains("level-seed=123"), "{props}");
        assert!(call("server:worldCreate", vec![json!({ "name": "world", "seed": "", "backup": false })]).await.is_err(), "already there");
        assert!(call("server:worldDelete", vec![json!({ "name": "world", "confirm": "wrong", "backup": false })]).await.is_err());
        call("server:worldSwitch", vec![json!({ "name": "world", "backup": false })]).await.unwrap();
        assert!(std::fs::read_to_string(dir.join("server.properties")).unwrap().contains("level-name=world"));
        assert!(call("server:worldDelete", vec![json!({ "name": "world", "confirm": "world", "backup": false })]).await.is_err(), "the world in use stays");

        // A tool at work stops a second one.
        let held = claim(&id).unwrap();
        assert!(call("server:worldSwitch", vec![json!({ "name": "world", "backup": false })]).await.is_err());
        drop(held);

        // A world to delete for real, with its nether beside it.
        let copy = dir.join("spare");
        std::fs::create_dir_all(&copy).unwrap();
        std::fs::copy(dir.join("world/level.dat"), copy.join("level.dat")).unwrap();
        std::fs::create_dir_all(dir.join("spare_nether/DIM-1/region")).unwrap();
        let before = zips(&dir);
        call("server:worldDelete", vec![json!({ "name": "spare", "confirm": "spare", "backup": true })]).await.unwrap();
        assert!(!copy.exists() && !dir.join("spare_nether").exists());
        assert_eq!(zips(&dir), before + 1);
    });
}

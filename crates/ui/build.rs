//! Embeds the SVG icon set (assets/icons) as `(name, svg)` tables.

use std::fmt::Write;
use std::path::Path;

fn table(out: &mut String, name: &str, dir: &Path) {
    let mut files: Vec<_> = std::fs::read_dir(dir).expect("icon dir").flatten().map(|e| e.path()).collect();
    files.sort();
    writeln!(out, "pub static {name}: &[(&str, &str)] = &[").unwrap();
    for f in files.iter().filter(|f| f.extension().is_some_and(|e| e == "svg")) {
        let stem = f.file_stem().unwrap().to_string_lossy();
        writeln!(out, "    ({stem:?}, include_str!({:?})),", f.canonicalize().unwrap()).unwrap();
    }
    writeln!(out, "];").unwrap();
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut out = String::new();
    table(&mut out, "GLYPHS", &root.join("glyph"));
    table(&mut out, "COLOR", &root.join("color"));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("icons_gen.rs"), out).unwrap();
}

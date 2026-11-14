//! Turns `/brand.json` (the ONE place the product name lives) into Rust constants.
use std::{env, fs, path::Path};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let brand_path = Path::new(&manifest)
        .join("..")
        .join("..")
        .join("brand.json");
    println!("cargo:rerun-if-changed={}", brand_path.display());

    let raw = fs::read_to_string(&brand_path).expect("brand.json must exist at the repo root");
    let json: serde_json::Value =
        serde_json::from_str(&raw).expect("brand.json must be valid JSON");
    let name = json["appName"].as_str().expect("brand.json: appName");
    let id = json["appId"].as_str().expect("brand.json: appId");
    let repo = json["repository"].as_str().unwrap_or("");

    let out = format!(
        "/// User-visible product name (from /brand.json).\npub const APP_NAME: &str = {name:?};\n\
         /// Reverse-DNS application id (from /brand.json).\npub const APP_ID: &str = {id:?};\n\
         /// GitHub repository (owner/name) for releases and updates (from /brand.json).\n\
         pub const REPOSITORY: &str = {repo:?};\n"
    );
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    fs::write(Path::new(&out_dir).join("brand.rs"), out).expect("write brand.rs");
}

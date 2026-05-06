// Ensure web/dist/ exists with at least a placeholder index.html so
// rust-embed in src/http.rs can compile even when the web bundle hasn't
// been built yet (e.g. fresh checkout, `cargo build` without bun).
//
// In a real release we expect `bun run build` to have populated
// web/dist before cargo runs (see .github/workflows/release.yml).

use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist = manifest.join("../../web/dist");

    if !dist.exists() {
        fs::create_dir_all(&dist).expect("create web/dist placeholder dir");
    }

    let index = dist.join("index.html");
    if !index.exists() {
        fs::write(
            &index,
            "<!doctype html><meta charset=utf-8><title>relay</title>\n\
             <h1>Web bundle not built</h1>\n\
             <p>Run <code>bun run build</code> in <code>web/</code> and rebuild the master binary.</p>\n",
        )
        .expect("write placeholder index.html");
    }

    println!("cargo:rerun-if-changed=../../web/dist");
}

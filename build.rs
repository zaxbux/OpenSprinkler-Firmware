use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("user_agent_header.rs");

    let ua = env!("CARGO_PKG_VERSION");

    fs::write(&dest_path, format!("const USER_AGENT_STRING: &'static str = \"OpenSprinkler/{} (rust)\";", ua)).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
}

use std::env;
use std::fs;
use std::path::Path;
use std::io::Write;

fn main() -> std::io::Result<()> {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_constants.rs");

    let ua = env!("CARGO_PKG_VERSION");
    
    let mut w: Vec<u8> = Vec::new();

    writeln!(w, "pub mod constants {{")?;
    writeln!(w, "pub const USER_AGENT_STRING: &'static str = \"OpenSprinkler/{} (rust)\";", ua)?;
    writeln!(w, "pub const MAX_EXT_BOARDS: usize = {};", option_env!("MAXIMUM_EXT_BOARDS").unwrap_or("24"))?;
    writeln!(w, "}}")?;

    fs::write(&dest_path, w)?;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=MAXIMUM_EXT_BOARDS");

    Ok(())
}

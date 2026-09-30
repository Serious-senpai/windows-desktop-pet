use std::path::PathBuf;
use std::{env, fs};

#[cfg(debug_assertions)]
const PROFILE: &str = "debug";

#[cfg(not(debug_assertions))]
const PROFILE: &str = "release";

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let rc = root.join("assets").join("app.rc");
    winres::WindowsResource::new()
        .set_resource_file(&rc.display().to_string())
        .set("FileDescription", "Windows Desktop Pet")
        .set("ProductName", "Windows Desktop Pet")
        .set("OriginalFilename", "windows-desktop-pet.exe")
        .set("InternalName", "windows-desktop-pet")
        .set_language(0x0409) // English (United States)
        .compile()
        .unwrap();

    let exe_dir = root.join("target").join(PROFILE);
    let _ = fs::copy(
        root.join("assets").join("config.json"),
        exe_dir.join("config.json"),
    );
    let _ = fs::copy(
        root.join("assets")
            .join("firefly.codex-pet")
            .join("spritesheet.webp"),
        exe_dir.join("spritesheet.webp"),
    );
}

use std::env;
use std::os::windows::fs;
use std::path::PathBuf;

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

    // Ignore error when creating symlinks, as the target file may already exist
    let exe_dir = root.join("target").join(PROFILE);
    let _ = fs::symlink_file(
        root.join("assets").join("config.json"),
        exe_dir.join("config.json"),
    );
    let _ = fs::symlink_file(
        root.join("assets")
            .join("miku.codex-pet")
            .join("spritesheet.webp"),
        exe_dir.join("spritesheet.webp"),
    );
}

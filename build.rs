//! Windows: puts the icon and the version information into the .exe. Explorer
//! shows the icon; the file's properties and the Task Manager show the name,
//! version and description. The texts are in `[package.metadata.winresource]`
//! of Cargo.toml.
//!
//! Also works when building for Windows from Linux, with the MinGW tools
//! (`x86_64-w64-mingw32-windres`).

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/biblo.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon/biblo.ico");
    // French (France).
    res.set_language(0x040C);
    if std::env::var_os("CARGO_FEATURE_DEMO").is_some() {
        res.set("FileDescription", "Biblo (démonstration)");
    }
    if let Err(e) = res.compile() {
        panic!("cannot add the icon and version information to the .exe: {e}");
    }
}

//! Windows only: embed the app icon and version info (VERSIONINFO) into `karnoxpdf.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `KARNOXPDF_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/karnoxpdf.ico");
    println!("cargo:rerun-if-env-changed=KARNOXPDF_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/karnoxpdf.ico")
        .set("ProductName", "KarnoxPDF")
        .set("FileDescription", "KarnoxPDF PDF workbench")
        .set("LegalCopyright", "Copyright (c) the KarnoxPDF contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "karnoxpdf.exe")
        .set("InternalName", "karnoxpdf");
    if let Err(e) = res.compile() {
        if std::env::var_os("KARNOXPDF_REQUIRE_WINRES").is_some() {
            println!("cargo::error=embedding Windows resources failed: {e}");
            return;
        }
        println!("cargo:warning=karnoxpdf.exe built without icon/version resources: {e}");
    }
}

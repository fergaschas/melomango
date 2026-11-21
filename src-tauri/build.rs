fn main() {
    let mut attributes = tauri_build::Attributes::new();

    // Tauri's dialogs need Common Controls 6 (TaskDialogIndirect), which Windows only loads
    // for an executable whose manifest asks for it. tauri-build adds that manifest to the
    // app, but not to test binaries: since the commands are registered through
    // tauri-specta, the tests link those dialogs and every test binary died at start with
    // STATUS_ENTRYPOINT_NOT_FOUND. The linker adds the same dependency (the only thing in
    // tauri-build's manifest) to every binary, app and tests alike, so tauri-build's own
    // copy is left out to avoid two manifests.
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        attributes = attributes.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' \
             version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

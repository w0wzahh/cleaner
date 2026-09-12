fn main() {
    // Embed the app icon + version info into the Windows binary so Explorer,
    // taskbar, and shortcuts all show the broom icon.
    #[cfg(windows)]
    let _ = embed_resource::compile("assets/icon.rc", embed_resource::NONE);
}

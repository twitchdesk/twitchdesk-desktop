fn main() {
    // wry/webview2 on Windows uses ETW + registry APIs that live in advapi32.
    // Some CI link environments don't pull this in automatically.
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}

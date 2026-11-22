fn main() {
    // OBS loads the plugin into a process that already has libobs. Resolve the
    // obs_* symbols at load time instead of linking a libobs binary.
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
    }
}

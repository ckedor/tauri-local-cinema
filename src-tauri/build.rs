fn main() {
  // Allow pointing the linker/runtime at a local libmpv install for dev.
  // Set MPV_LIB_DIR to a folder that contains mpv.lib (and libmpv-2.dll for runtime).
  let dir = std::env::var("MPV_LIB_DIR").ok().unwrap_or_else(|| {
    // Fallback: common dev location on this machine.
    if cfg!(windows) && std::path::Path::new("C:/mpv-dev/mpv.lib").exists() {
      "C:/mpv-dev".to_string()
    } else {
      String::new()
    }
  });
  if !dir.is_empty() {
    println!("cargo:rustc-link-search=native={}", dir);
  }
  println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");

  tauri_build::build()
}

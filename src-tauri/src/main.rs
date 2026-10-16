#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  #[cfg(target_os = "linux")]
  prefer_display_gpu();
  tauri_local_cinema_lib::run();
}

/// Hybrid laptops (Intel/AMD drives the screen, NVIDIA on the side): Omarchy
/// exports NVIDIA as the GL vendor for the whole session, so WebKitGTK renders
/// the UI through the NVIDIA driver and its web process crashes there (the
/// "Process crashed" notice on close). Point EGL/GLX at Mesa, the driver of
/// the GPU that drives the display. Nothing changes on single-GPU machines or
/// when the variable is already set. Runs before any thread exists.
#[cfg(target_os = "linux")]
fn prefer_display_gpu() {
  const MESA_EGL: &str = "/usr/share/glvnd/egl_vendor.d/50_mesa.json";
  const NVIDIA: &str = "0x10de";
  if std::env::var_os("__EGL_VENDOR_LIBRARY_FILENAMES").is_some() || !std::path::Path::new(MESA_EGL).exists() {
    return;
  }
  let vendors: Vec<String> = std::fs::read_dir("/sys/class/drm")
    .into_iter()
    .flatten()
    .flatten()
    .filter(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
    .filter_map(|entry| std::fs::read_to_string(entry.path().join("device/vendor")).ok())
    .map(|vendor| vendor.trim().to_string())
    .collect();
  let hybrid = vendors.iter().any(|v| v == NVIDIA) && vendors.iter().any(|v| v != NVIDIA);
  if hybrid {
    std::env::set_var("__EGL_VENDOR_LIBRARY_FILENAMES", MESA_EGL);
    std::env::set_var("__GLX_VENDOR_LIBRARY_NAME", "mesa");
  }
}

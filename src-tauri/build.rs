use std::path::{Path, PathBuf};

fn copy_runtime_dlls(mpv_dir: &Path) {
  let out_dir = match std::env::var_os("OUT_DIR") {
    Some(value) => PathBuf::from(value),
    None => return,
  };

  let profile_dir = match out_dir.ancestors().nth(3) {
    Some(path) => path,
    None => return,
  };

  if let Ok(entries) = std::fs::read_dir(mpv_dir) {
    for entry in entries.flatten() {
      let path = entry.path();
      if path.extension().and_then(|ext| ext.to_str()) != Some("dll") {
        continue;
      }

      if let Some(name) = path.file_name() {
        let destination = profile_dir.join(name);
        let _ = std::fs::copy(&path, destination);
      }
    }
  }
}

fn main() {
  // Allow pointing the linker/runtime at a local libmpv install for dev.
  // Set MPV_LIB_DIR to a folder that contains mpv.lib and the runtime DLLs.
  let dir = std::env::var("MPV_LIB_DIR").ok().unwrap_or_else(|| {
    if !cfg!(windows) {
      return String::new();
    }

    // Prefer the repo-local vendor folder so `tauri dev` works after the
    // installer prerequisites have been prepared once.
    let repo_mpv_dir = std::path::Path::new("..")
      .join("vendor")
      .join("windows")
      .join("mpv");
    if repo_mpv_dir.join("mpv.lib").exists() {
      return repo_mpv_dir.display().to_string();
    }

    // Fallback: common dev location on this machine.
    if std::path::Path::new("C:/mpv-dev/mpv.lib").exists() {
      "C:/mpv-dev".to_string()
    } else {
      String::new()
    }
  });
  if !dir.is_empty() {
    println!("cargo:rustc-link-search=native={}", dir);

    if cfg!(windows) {
      copy_runtime_dlls(Path::new(&dir));
    }
  }
  println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
  println!("cargo:rerun-if-changed=../vendor/windows/mpv/mpv.lib");

  tauri_build::build()
}

use std::cell::Cell;
use std::path::Path;

use libmpv2::Mpv;
use tauri::{AppHandle, Manager};

use crate::contracts::player::{PlayerRectDto, PlayerSessionDto, PlayerStatusDto, SubtitleTrackDto};
use crate::modules::scanner::list_video_subtitles;

#[cfg(windows)]
mod child_window {
  use std::sync::{Once, OnceLock};
  use tauri::{AppHandle, Emitter};
  use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
  use windows::Win32::System::LibraryLoader::GetModuleHandleW;
  use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, SetWindowPos, ShowWindow,
    HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOW, WM_LBUTTONDBLCLK,
    WM_LBUTTONUP,
    WINDOW_EX_STYLE, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW,
  };
  use windows::core::PCWSTR;

  /// Wrapper that lets us send the HWND across threads (we control access).
  #[derive(Copy, Clone)]
  pub struct ChildHwnd(pub isize);
  unsafe impl Send for ChildHwnd {}

  static REGISTER: Once = Once::new();
  static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
  const CLASS_NAME: &str = "NetCricoMpvHost\0";
  const SURFACE_CLICK_EVENT: &str = "player://surface-click";
  const SURFACE_DOUBLE_CLICK_EVENT: &str = "player://surface-double-click";

  unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
  ) -> LRESULT {
    match msg {
      WM_LBUTTONUP => {
        if let Some(app) = APP_HANDLE.get() {
          let _ = app.emit(SURFACE_CLICK_EVENT, ());
        }
      }
      WM_LBUTTONDBLCLK => {
        if let Some(app) = APP_HANDLE.get() {
          let _ = app.emit(SURFACE_DOUBLE_CLICK_EVENT, ());
        }
      }
      _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
  }

  pub fn remember_app(app: &AppHandle) {
    let _ = APP_HANDLE.set(app.clone());
  }

  fn ensure_class() {
    REGISTER.call_once(|| unsafe {
      let class_w: Vec<u16> = CLASS_NAME.encode_utf16().collect();
      let hinst = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
      let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinst.into(),
        hbrBackground: Default::default(), // NULL: do not erase background
        lpszClassName: PCWSTR(class_w.as_ptr()),
        ..Default::default()
      };
      let _ = RegisterClassExW(&wc);
      // Leak class name buffer intentionally (lives for process lifetime).
      std::mem::forget(class_w);
    });
  }

  pub fn create_child(parent: isize, x: i32, y: i32, w: i32, h: i32) -> Result<ChildHwnd, String> {
    ensure_class();
    let class_w: Vec<u16> = CLASS_NAME.encode_utf16().collect();
    let title: Vec<u16> = "\0".encode_utf16().collect();
    unsafe {
      let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class_w.as_ptr()),
        PCWSTR(title.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
        x, y, w, h,
        HWND(parent as *mut _),
        None,
        None,
        None,
      ).map_err(|e| format!("CreateWindowExW failed: {e}"))?;
      // Force above sibling WebView2 host.
      let _ = SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
      Ok(ChildHwnd(hwnd.0 as isize))
    }
  }

  pub fn move_child(hwnd: ChildHwnd, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
      let _ = SetWindowPos(
        HWND(hwnd.0 as *mut _),
        HWND_TOP,
        x, y, w, h,
        SWP_NOACTIVATE,
      );
    }
  }

  #[allow(dead_code)]
  pub fn show(hwnd: ChildHwnd, visible: bool) {
    unsafe {
      let _ = ShowWindow(HWND(hwnd.0 as *mut _), if visible { SW_SHOW } else { SW_HIDE });
    }
  }

  pub fn destroy(hwnd: ChildHwnd) {
    unsafe {
      let _ = DestroyWindow(HWND(hwnd.0 as *mut _));
    }
  }
}

#[cfg(windows)]
use child_window::ChildHwnd;

pub struct PlayerEngine {
  mpv: Option<Mpv>,
  /// The file has started playing at least once: from then on, mpv going idle
  /// means the user closed its window (Linux), so the session is over.
  started: Cell<bool>,
  #[cfg(windows)]
  child_hwnd: Option<ChildHwnd>,
  session: Option<PlayerSessionDto>,
  last_error: Option<String>
}

impl PlayerEngine {
  pub fn new() -> Self {
    Self {
      mpv: None,
      started: Cell::new(false),
      #[cfg(windows)]
      child_hwnd: None,
      session: None,
      last_error: None
    }
  }

  /// Create the embedded mpv tied to a child HWND inside the main window.
  /// Loads `session.media_path` immediately. Idempotent: replaces any prior session.
  pub fn load(
    &mut self,
    app: &AppHandle,
    session: PlayerSessionDto,
    rect: PlayerRectDto,
  ) -> Result<(), String> {
    if !Path::new(&session.media_path).exists() {
      return Err(format!("media not found: {}", session.media_path));
    }

    // Tear down any previous session so we start clean.
    self.destroy_internal();

    #[cfg(windows)]
    {
      child_window::remember_app(app);

      let main = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
      let parent_hwnd = main
        .hwnd()
        .map_err(|e| format!("hwnd: {e}"))?
        .0 as isize;

      let child = child_window::create_child(parent_hwnd, rect.x, rect.y, rect.width, rect.height)?;
      let wid = child.0 as i64;

      let mpv = Mpv::with_initializer(|init| {
        init.set_property("wid", wid)?;
        init.set_property("vo", "gpu")?;
        init.set_property("hwdec", "auto-safe")?;
        init.set_property("keep-open", "yes")?;
        init.set_property("force-window", "yes")?;
        init.set_property("audio-display", "no")?;
        init.set_property("input-default-bindings", "no")?;
        init.set_property("input-vo-keyboard", "no")?;
        init.set_property("slang", SUBTITLE_LANGUAGES)?;
        // add_subtitle_files adds them all; mpv's own loading would duplicate them.
        init.set_property("sub-auto", "no")?;
        Ok(())
      })
      .map_err(|e| format!("mpv init: {e}"))?;

      mpv.command("loadfile", &[session.media_path.as_str()])
        .map_err(|e| format!("loadfile: {e}"))?;

      add_subtitle_files(&mpv, &session);

      self.mpv = Some(mpv);
      self.child_hwnd = Some(child);
      self.session = Some(session);
      self.last_error = None;
      Ok(())
    }

    // No embedding on Linux (Wayland has no foreign window handles to draw
    // into): mpv opens its own fullscreen window, with its on-screen controller
    // and keys. The app's controls still drive this same mpv.
    #[cfg(not(windows))]
    {
      let _ = (app, rect);
      // libmpv refuses to start (mpv_create returns NULL, "mpv init: Null")
      // unless LC_NUMERIC is "C", and GTK sets the user's locale at startup.
      // Only number formatting in C code is affected; the UI runs in WebKit.
      unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };
      let mpv = Mpv::with_initializer(|init| {
        init.set_property("hwdec", "auto-safe")?;
        init.set_property("keep-open", "yes")?;
        init.set_property("force-window", "yes")?;
        init.set_property("fullscreen", "yes")?;
        init.set_property("osc", "yes")?;
        init.set_property("input-default-bindings", "yes")?;
        init.set_property("input-vo-keyboard", "yes")?;
        init.set_property("title", session.media_title.as_str())?;
        init.set_property("slang", SUBTITLE_LANGUAGES)?;
        // add_subtitle_files adds them all; mpv's own loading would duplicate them.
        init.set_property("sub-auto", "no")?;
        Ok(())
      })
      .map_err(|e| format!("mpv init: {e}"))?;

      // q and closing the window would quit mpv's core and leave a dead handle:
      // make them stop playback instead, which status() reports as the end.
      for key in ["q", "Q", "CLOSE_WIN"] {
        let _ = mpv.command("keybind", &[key, "stop"]);
      }

      mpv.command("loadfile", &[session.media_path.as_str()])
        .map_err(|e| format!("loadfile: {e}"))?;

      add_subtitle_files(&mpv, &session);

      self.mpv = Some(mpv);
      self.session = Some(session);
      self.last_error = None;
      Ok(())
    }
  }

  pub fn set_rect(&mut self, rect: PlayerRectDto) {
    #[cfg(windows)]
    if let Some(hwnd) = self.child_hwnd {
      child_window::move_child(hwnd, rect.x, rect.y, rect.width, rect.height);
    }
    let _ = rect;
  }

  pub fn toggle_pause(&mut self) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    let paused: bool = mpv.get_property("pause").unwrap_or(false);
    mpv.set_property("pause", !paused).map_err(|e| format!("pause: {e}"))
  }

  pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    mpv.set_property("pause", paused).map_err(|e| format!("pause: {e}"))
  }

  pub fn set_volume(&mut self, volume_percent: f64) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    let clamped_volume = volume_percent.clamp(0.0, 100.0);
    mpv.set_property("volume", clamped_volume).map_err(|e| format!("volume: {e}"))
  }

  pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    mpv.set_property("mute", muted).map_err(|e| format!("mute: {e}"))
  }

  pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    let s = format!("{seconds}");
    mpv.command("seek", &[s.as_str(), "relative"])
      .map_err(|e| format!("seek: {e}"))
  }

  pub fn seek_to(&mut self, seconds: f64) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    let s = format!("{seconds}");
    mpv.command("seek", &[s.as_str(), "absolute"])
      .map_err(|e| format!("seek_to: {e}"))
  }

  /// Subtitle tracks mpv knows: embedded in the file and the added files.
  pub fn subtitle_tracks(&self) -> Vec<SubtitleTrackDto> {
    let Some(mpv) = self.mpv.as_ref() else {
      return Vec::new();
    };
    let count = mpv.get_property::<i64>("track-list/count").unwrap_or(0);
    (0..count)
      .filter(|index| mpv.get_property::<String>(&format!("track-list/{index}/type")).is_ok_and(|kind| kind == "sub"))
      .filter_map(|index| {
        let field = |name: &str| mpv.get_property::<String>(&format!("track-list/{index}/{name}")).ok().filter(|value| !value.is_empty());
        Some(SubtitleTrackDto {
          id: mpv.get_property::<i64>(&format!("track-list/{index}/id")).ok()?.to_string(),
          language: field("lang"),
          title: field("title"),
          external: mpv.get_property::<bool>(&format!("track-list/{index}/external")).unwrap_or(false),
          selected: mpv.get_property::<bool>(&format!("track-list/{index}/selected")).unwrap_or(false)
        })
      })
      .collect()
  }

  /// `None` turns subtitles off.
  pub fn set_subtitle(&mut self, id: Option<String>) -> Result<(), String> {
    let mpv = self.mpv.as_ref().ok_or_else(|| "no active player".to_string())?;
    match id.and_then(|id| id.parse::<i64>().ok()) {
      Some(id) => mpv.set_property("sid", id),
      None => mpv.set_property("sid", "no")
    }
    .map_err(|e| format!("sid: {e}"))
  }

  pub fn stop(&mut self) {
    self.destroy_internal();
  }

  pub fn status(&self) -> PlayerStatusDto {
    let is_paused = self
      .mpv
      .as_ref()
      .and_then(|m| m.get_property::<bool>("pause").ok())
      .unwrap_or(false);
    let position_sec = self
      .mpv
      .as_ref()
      .and_then(|m| m.get_property::<f64>("time-pos").ok())
      .unwrap_or(0.0);
    let duration_sec = self
      .mpv
      .as_ref()
      .and_then(|m| m.get_property::<f64>("duration").ok())
      .unwrap_or(0.0);
    let volume_percent = self
      .mpv
      .as_ref()
      .and_then(|m| m.get_property::<f64>("volume").ok())
      .unwrap_or(100.0)
      .clamp(0.0, 100.0);
    let is_muted = self
      .mpv
      .as_ref()
      .and_then(|m| m.get_property::<bool>("mute").ok())
      .unwrap_or(false);
    // Idle after having played: the user closed mpv's own window (Linux).
    let idle = self
      .mpv
      .as_ref()
      .and_then(|m| m.get_property::<bool>("idle-active").ok())
      .unwrap_or(false);
    if self.mpv.is_some() && !idle {
      self.started.set(true);
    }
    PlayerStatusDto {
      session: self.session.clone(),
      is_playing: self.mpv.is_some() && !(idle && self.started.get()),
      is_paused,
      position_sec,
      duration_sec,
      volume_percent,
      is_muted,
      last_error: self.last_error.clone()
    }
  }

  fn destroy_internal(&mut self) {
    // Drop mpv first (releases its hold on the HWND), then destroy the child window.
    self.mpv = None;
    self.started.set(false);
    #[cfg(windows)]
    if let Some(hwnd) = self.child_hwnd.take() {
      child_window::destroy(hwnd);
    }
    self.session = None;
  }
}

impl Drop for PlayerEngine {
  fn drop(&mut self) {
    self.destroy_internal();
  }
}

/// mpv picks the first of these it finds, embedded or external.
const SUBTITLE_LANGUAGES: &str = "en,eng,en-US,en-us";

/// Every subtitle file next to the video (library contract names), so the
/// player can switch between them; the session's own one is always included.
fn add_subtitle_files(mpv: &Mpv, session: &PlayerSessionDto) {
  let mut files = list_video_subtitles(Path::new(&session.media_path));
  if let Some(sub) = session.subtitle_path.as_deref().filter(|sub| !sub.trim().is_empty() && Path::new(sub).exists()) {
    if !files.iter().any(|(path, _)| path == Path::new(sub)) {
      files.push((Path::new(sub).to_path_buf(), String::new()));
    }
  }
  for (path, tag) in files {
    let path = path.to_string_lossy().to_string();
    // "pt-br.sdh" -> language "pt-br"; mpv shows the full tag as the title.
    let language = tag.split('.').next().unwrap_or_default().to_string();
    let _ = mpv.command("sub-add", &[path.as_str(), "auto", tag.as_str(), language.as_str()]);
  }
}

pub struct PlayerModule;

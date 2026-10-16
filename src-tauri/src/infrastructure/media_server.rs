//! Serves local media files over HTTP, for the <video> player on Linux.
//!
//! WebKitGTK's media pipeline (GStreamer) can't read from custom URI schemes
//! such as Tauri's asset:// (the video fails with MEDIA_ERR_SRC_NOT_SUPPORTED),
//! but plays plain HTTP with Range requests, which seeking needs. So the app
//! runs this tiny server: bound to 127.0.0.1 only, and every URL carries a
//! random token, so other local users or web pages can't read files through it.
//!
//! URL: http://127.0.0.1:<port>/<token>/<percent-encoded absolute path>

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use tiny_http::{Header, Request, Response, Server, StatusCode};

pub struct MediaServer {
  /// http://127.0.0.1:<port>/<token>/ — append the percent-encoded path.
  pub base_url: String,
}

impl MediaServer {
  pub fn start() -> Result<Self, String> {
    let server = Server::http("127.0.0.1:0").map_err(|e| format!("media server: {e}"))?;
    let port = server
      .server_addr()
      .to_ip()
      .ok_or("media server: no IP address")?
      .port();
    let token = random_token()?;
    let base_url = format!("http://127.0.0.1:{port}/{token}/");

    let prefix = format!("/{token}/");
    std::thread::spawn(move || {
      for request in server.incoming_requests() {
        let prefix = prefix.clone();
        // One thread per request: the player keeps one stream open while it
        // also asks for other ranges (the index at the end of an mp4, seeks).
        std::thread::spawn(move || serve(request, &prefix));
      }
    });
    Ok(Self { base_url })
  }
}

fn random_token() -> Result<String, String> {
  let mut bytes = [0u8; 16];
  File::open("/dev/urandom")
    .and_then(|mut f| f.read_exact(&mut bytes))
    .map_err(|e| format!("media server token: {e}"))?;
  Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn header(name: &str, value: &str) -> Header {
  Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn content_type(path: &PathBuf) -> &'static str {
  let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
  match ext.as_str() {
    "mp4" | "m4v" => "video/mp4",
    "mkv" => "video/x-matroska",
    "webm" => "video/webm",
    "avi" => "video/x-msvideo",
    "mov" => "video/quicktime",
    "srt" => "text/plain; charset=utf-8",
    "vtt" => "text/vtt; charset=utf-8",
    _ => "application/octet-stream",
  }
}

fn percent_decode(s: &str) -> Option<String> {
  let bytes = s.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'%' {
      let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
      out.push(u8::from_str_radix(hex, 16).ok()?);
      i += 3;
    } else {
      out.push(bytes[i]);
      i += 1;
    }
  }
  String::from_utf8(out).ok()
}

/// "bytes=START-END" or "bytes=START-" -> (start, end inclusive).
fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
  let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
  let start: u64 = start.parse().ok()?;
  let end: u64 = if end.is_empty() { size - 1 } else { end.parse().ok()? };
  (start <= end && end < size).then_some((start, end))
}

fn serve(request: Request, prefix: &str) {
  let reply = |request: Request, code: u16| {
    let _ = request.respond(Response::empty(StatusCode(code)));
  };
  let Some(path) = request
    .url()
    .strip_prefix(prefix)
    .and_then(percent_decode)
    .map(|p| PathBuf::from(format!("/{}", p.trim_start_matches('/'))))
  else {
    return reply(request, 404);
  };
  let Ok(mut file) = File::open(&path) else {
    return reply(request, 404);
  };
  let Ok(size) = file.metadata().map(|m| m.len()) else {
    return reply(request, 500);
  };

  let range = request
    .headers()
    .iter()
    .find(|h| h.field.equiv("Range"))
    .map(|h| h.value.as_str().to_string());
  let (status, start, end) = match range {
    Some(r) => match parse_range(&r, size) {
      Some((start, end)) => (206, start, end),
      None => return reply(request, 416),
    },
    None => (200, 0, size.saturating_sub(1)),
  };
  if file.seek(SeekFrom::Start(start)).is_err() {
    return reply(request, 500);
  }
  let length = if size == 0 { 0 } else { end - start + 1 };

  let mut headers = vec![
    header("Content-Type", content_type(&path)),
    header("Accept-Ranges", "bytes"),
    // Subtitles are fetch()ed from the app's own origin.
    header("Access-Control-Allow-Origin", "*"),
  ];
  if status == 206 {
    headers.push(header("Content-Range", &format!("bytes {start}-{end}/{size}")));
  }
  // Always a Content-Length: tiny_http would send bodies over 32 KB chunked,
  // and without the size WebKit takes the video for a live stream (no
  // duration, no seeking).
  let response = Response::new(
    StatusCode(status),
    headers,
    file.take(length),
    Some(length as usize),
    None,
  )
  .with_chunked_threshold(usize::MAX);
  // The player drops connections it no longer needs (after a seek): not an error.
  let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ranges() {
    assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
    assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
    assert_eq!(parse_range("bytes=50-200", 100), None);
    assert_eq!(parse_range("items=0-1", 100), None);
  }

  #[test]
  fn decodes_paths() {
    assert_eq!(
      percent_decode("run/media/Extreme%20SSD/a%C3%A7%C3%A3o.mkv").as_deref(),
      Some("run/media/Extreme SSD/ação.mkv")
    );
    assert_eq!(percent_decode("bad%zz"), None);
  }
}

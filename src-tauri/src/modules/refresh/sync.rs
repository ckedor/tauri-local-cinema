//! Aligns downloaded subtitles with their video. The reference is a subtitle
//! track embedded in the video (in sync by definition) and `alass` shifts the
//! file onto it; it matches timing patterns, so the languages needn't match.
//! alass can also align against the video's audio, but on the release tried
//! that gave the wrong shift (-10 s where +5 s was right), so a video with no
//! embedded text track is left alone and reported.

use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Below this, a file counts as already in sync and is not rewritten.
const IN_SYNC_MS: i64 = 150;
/// Image-based tracks (PGS, VobSub) can't be a reference.
const TEXT_CODECS: &[&str] = &["subrip", "srt", "ass", "ssa", "mov_text", "webvtt", "text"];
/// What alass can read and write.
pub(crate) const SYNCABLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa"];

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
	pub synced: usize,
	pub already_in_sync: usize,
	/// Files left alone because their video has no embedded text track.
	pub no_reference: usize,
	pub notes: Vec<String>
}

/// A scratch folder that disappears with it.
struct TempDir(PathBuf);

impl TempDir {
	fn new() -> Result<Self, String> {
		let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or_default();
		let path = env::temp_dir().join(format!("netcrico-sync-{}-{}", std::process::id(), nanos));
		fs::create_dir_all(&path).map_err(|error| format!("{}: {}", path.display(), error))?;
		Ok(Self(path))
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

/// Aligns each of `subtitles` (files of `video`) in place. A file that
/// changes first goes to `backup_dir` when one is given. Results and reasons
/// for skipping land in `summary`.
pub fn sync_files(video: &Path, subtitles: &[PathBuf], backup_dir: Option<&Path>, summary: &mut SyncSummary) {
	let (Some(alass), Some(_)) = (find_tool("alass-cli"), find_tool("ffmpeg")) else {
		let note = "alass-cli ou ffmpeg nao encontrado: as legendas nao foram sincronizadas (instale com: cargo install alass-cli).".to_string();
		if !summary.notes.contains(&note) {
			summary.notes.push(note);
		}
		return;
	};
	let Ok(temp) = TempDir::new() else {
		summary.notes.push("Nao foi possivel criar a pasta temporaria para sincronizar.".to_string());
		return;
	};

	let reference = match extract_reference(video, &temp.0) {
		Ok(Some(reference)) => reference,
		Ok(None) => {
			summary.no_reference += subtitles.len();
			return;
		}
		Err(error) => {
			let video_name = video.file_name().and_then(|name| name.to_str()).unwrap_or_default();
			summary.notes.push(format!("{}: {}", video_name, error));
			return;
		}
	};

	for (index, subtitle) in subtitles.iter().enumerate() {
		let extension = subtitle.extension().and_then(|value| value.to_str()).unwrap_or("srt");
		let aligned = temp.0.join(format!("aligned-{}.{}", index, extension));
		let name = subtitle.file_name().and_then(|value| value.to_str()).unwrap_or_default();

		let shifts = match align(&alass, &reference, subtitle, &aligned) {
			Ok(shifts) => shifts,
			Err(error) => {
				summary.notes.push(format!("{}: {}", name, error));
				continue;
			}
		};
		if shifts.iter().all(|shift| shift.abs() < IN_SYNC_MS) {
			summary.already_in_sync += 1;
			continue;
		}

		if let Some(backup_dir) = backup_dir {
			let parked = backup_dir.join(name);
			let moved = fs::create_dir_all(backup_dir).and_then(|_| fs::copy(subtitle, &parked));
			if let Err(error) = moved {
				summary.notes.push(format!("{}: nao foi possivel guardar o original ({})", name, error));
				continue;
			}
		}
		// Copy rather than rename: the temp folder can be on another disk.
		match fs::copy(&aligned, subtitle) {
			Ok(_) => summary.synced += 1,
			Err(error) => summary.notes.push(format!("{}: {}", name, error))
		}
	}
}

/// A text subtitle track inside a video.
#[derive(Debug, Clone)]
pub struct TextTrack {
	pub index: i64,
	/// As tagged by the release: "eng", "por", "pt-BR".
	pub language: String,
	pub title: String
}

/// The video's text subtitle tracks (image-based ones can't be extracted to
/// text), never "forced" ones: those hold only the foreign-language lines.
pub fn text_tracks(video: &Path) -> Result<Vec<TextTrack>, String> {
	let ffprobe = find_tool("ffprobe").ok_or_else(|| "ffprobe nao encontrado".to_string())?;
	let probe = Command::new(ffprobe)
		.args(["-v", "error", "-select_streams", "s", "-show_entries", "stream=index,codec_name:stream_tags=language,title:stream_disposition=forced", "-of", "json"])
		.arg(video)
		.output()
		.map_err(|error| format!("ffprobe: {}", error))?;
	let probe = serde_json::from_slice::<Value>(&probe.stdout).map_err(|error| format!("ffprobe: {}", error))?;

	Ok(probe["streams"]
		.as_array()
		.into_iter()
		.flatten()
		.filter(|stream| stream["codec_name"].as_str().is_some_and(|codec| TEXT_CODECS.contains(&codec)))
		.filter(|stream| stream["disposition"]["forced"].as_i64().unwrap_or(0) == 0)
		.filter_map(|stream| {
			let title = stream["tags"]["title"].as_str().unwrap_or_default().to_string();
			let lowered = title.to_lowercase();
			if ["forced", "sign", "song"].iter().any(|word| lowered.contains(word)) {
				return None;
			}
			Some(TextTrack {
				index: stream["index"].as_i64()?,
				language: stream["tags"]["language"].as_str().unwrap_or_default().to_string(),
				title
			})
		})
		.collect())
}

/// The track for a language like "pt-BR" or "en", if the video has one. A
/// plain track beats an SDH one (no [music] cues), then the first.
pub fn track_for_language<'a>(tracks: &'a [TextTrack], preferred: &str) -> Option<&'a TextTrack> {
	let mut matching = tracks.iter().filter(|track| language_matches(preferred, &track.language, &track.title)).collect::<Vec<_>>();
	matching.sort_by_key(|track| (track.title.to_lowercase().contains("sdh"), track.index));
	matching.into_iter().next()
}

/// "pt-BR" matches pt-BR, pob, and por/pt titled "Brazil"; "en" matches en, eng,
/// en-US. Releases tag languages with 3-letter codes and name the variant in
/// the title. (The player's menu applies the same rule in TypeScript.)
fn language_matches(preferred: &str, language: &str, title: &str) -> bool {
	let preferred = preferred.to_lowercase();
	let (base, region) = match preferred.split_once('-') {
		Some((base, region)) => (base, Some(region)),
		None => (preferred.as_str(), None)
	};
	let code = language.to_lowercase();
	let code = code.split(['.', '_']).next().unwrap_or_default();
	let code = match code {
		"eng" => "en",
		"por" => "pt",
		"pob" => "pt-br",
		"spa" => "es",
		"fre" | "fra" => "fr",
		"ger" | "deu" => "de",
		"ita" => "it",
		"jpn" => "ja",
		other => other
	};
	let (track_base, track_region) = match code.split_once('-') {
		Some((base, region)) => (base, Some(region)),
		None => (code, None)
	};

	if track_base != base {
		return false;
	}
	let Some(region) = region else { return true };
	if let Some(track_region) = track_region {
		return track_region == region;
	}
	let words: &[&str] = match region {
		"br" => &["brazil", "brasil"],
		"pt" => &["portugal", "european"],
		"us" => &["american"],
		"gb" => &["british"],
		"mx" => &["mexic"],
		_ => &[]
	};
	let title = title.to_lowercase();
	words.iter().any(|word| title.contains(word))
}

/// Writes one track as a subtitle file.
pub fn extract_track(video: &Path, index: i64, output: &Path) -> Result<(), String> {
	let ffmpeg = find_tool("ffmpeg").ok_or_else(|| "ffmpeg nao encontrado".to_string())?;
	// Written under a name the scanner ignores, so a half-written file is never read as a subtitle.
	let partial = output.with_extension("srt.partial");
	let status = Command::new(ffmpeg)
		.args(["-v", "error", "-y", "-i"])
		.arg(video)
		.args(["-map", &format!("0:{}", index), "-f", "srt"])
		.arg(&partial)
		.status()
		.map_err(|error| format!("ffmpeg: {}", error))?;
	if !status.success() || !partial.exists() {
		let _ = fs::remove_file(&partial);
		return Err("ffmpeg nao conseguiu extrair a legenda embutida.".to_string());
	}
	fs::rename(&partial, output).map_err(|error| format!("{}: {}", output.display(), error))
}

/// Whether the tools that read a video's embedded tracks are installed.
pub fn can_read_embedded() -> bool {
	find_tool("ffprobe").is_some() && find_tool("ffmpeg").is_some()
}

/// The best text track of the video as an .srt in `dir`, English first.
fn extract_reference(video: &Path, dir: &Path) -> Result<Option<PathBuf>, String> {
	let tracks = text_tracks(video)?;
	let Some(track) = track_for_language(&tracks, "en").or_else(|| tracks.first()) else {
		return Ok(None);
	};

	let output = dir.join("reference.srt");
	extract_track(video, track.index, &output)?;
	Ok(Some(output))
}

/// Runs alass and returns the shift of each block it moved, in milliseconds.
fn align(alass: &Path, reference: &Path, subtitle: &Path, output: &Path) -> Result<Vec<i64>, String> {
	let result = Command::new(alass)
		.arg(reference)
		.arg(subtitle)
		.arg(output)
		.output()
		.map_err(|error| format!("alass: {}", error))?;
	let log = format!("{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));

	if !result.status.success() || !output.exists() {
		let reason = log.lines().rev().map(str::trim).find(|line| line.starts_with("error")).unwrap_or("falhou");
		return Err(format!("alass {}", reason));
	}

	// alass redraws a progress bar with carriage returns, so split on those too.
	let shifts = log.split(['\n', '\r']).filter_map(parse_shift).collect::<Vec<_>>();
	if shifts.is_empty() {
		return Err("alass nao informou o deslocamento".to_string());
	}
	Ok(shifts)
}

/// "shifted block of 640 subtitles with length 0:24:53.117 by -0:00:05.309" -> -5309.
fn parse_shift(line: &str) -> Option<i64> {
	let shift = line.trim().strip_prefix("shifted block of")?.rsplit_once(" by ")?.1.trim();
	let (sign, shift) = match shift.strip_prefix('-') {
		Some(rest) => (-1, rest),
		None => (1, shift)
	};
	let mut parts = shift.split(':');
	let hours = parts.next()?.parse::<i64>().ok()?;
	let minutes = parts.next()?.parse::<i64>().ok()?;
	let seconds = parts.next()?.parse::<f64>().ok()?;
	Some(sign * ((hours * 3600 + minutes * 60) * 1000 + (seconds * 1000.0).round() as i64))
}

/// A tool on PATH, or where `cargo install` and the app's own folder put it.
fn find_tool(name: &str) -> Option<PathBuf> {
	let file_name = if cfg!(windows) { format!("{}.exe", name) } else { name.to_string() };
	let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")).map(PathBuf::from);
	let app_dir = env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));

	env::var_os("PATH")
		.iter()
		.flat_map(env::split_paths)
		.chain(home.iter().map(|home| home.join(".cargo").join("bin")))
		.chain(home.iter().map(|home| home.join(".local").join("bin")))
		.chain(app_dir)
		.map(|dir| dir.join(&file_name))
		.find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
	use super::parse_shift;

	#[test]
	fn reads_alass_shift_lines() {
		assert_eq!(parse_shift("shifted block of 640 subtitles with length 0:24:53.117 by 0:00:05.309"), Some(5309));
		assert_eq!(parse_shift("shifted block of 60 subtitles with length 0:01:57.952 by -0:00:17.376"), Some(-17376));
		assert_eq!(parse_shift("shifted block of 1 subtitles with length 0:00:01.000 by 1:02:03.004"), Some(3_723_004));
		assert_eq!(parse_shift("warn: some subtitles now have negative timings"), None);
	}

	#[test]
	fn matches_release_language_tags() {
		use super::language_matches;
		assert!(language_matches("en", "eng", "SDH SRT"));
		assert!(language_matches("en", "en", ""));
		assert!(language_matches("pt-BR", "por", "Brazil"));
		assert!(language_matches("pt-BR", "pob", ""));
		assert!(language_matches("pt-BR", "pt-BR", ""));
		assert!(!language_matches("pt-BR", "por", ""));
		assert!(!language_matches("pt-BR", "spa", "Latin America"));
		assert!(!language_matches("en", "ara", ""));
	}
}

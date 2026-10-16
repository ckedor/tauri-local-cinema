//! One Claude call that places episode files whose names don't say which
//! TMDb episode they are (release-numbered Specials, unnumbered episodes),
//! given TMDb's episode list for their season.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const MODEL: &str = "claude-opus-5-5";
const REQUEST_TIMEOUT_SECONDS: u64 = 150;

const SYSTEM_PROMPT: &str = "You match video files of a TV show to TMDb episodes. Each file sits in a season folder; \
pick the TMDb episode of that same season it is, using the title words in the file name (release names are noisy: \
resolution, codecs, group tags, release numbering that differs from TMDb). Use null when no episode clearly matches \
or when the file is clearly not an episode (a trailer, a featurette). Never give two files the same episode.";

/// `files` are (path, season). Returns path -> TMDb episode number.
pub fn match_episodes(
	anthropic_key: &str,
	show_title: &str,
	files: &[(String, i32)],
	names: &HashMap<i32, HashMap<u32, String>>
) -> Result<HashMap<String, u32>, String> {
	let mut seasons = files.iter().map(|(_, season)| *season).collect::<Vec<_>>();
	seasons.sort();
	seasons.dedup();

	let mut message = format!("Show: {}\n", show_title);
	for season in &seasons {
		let mut episodes = names.get(season).map(|episodes| episodes.iter().collect::<Vec<_>>()).unwrap_or_default();
		episodes.sort();
		message.push_str(&format!("\nTMDb season {}:\n", season));
		for (number, name) in episodes {
			message.push_str(&format!("  E{:02} {}\n", number, name));
		}
	}
	message.push_str("\nFiles (index, season folder, file name):\n");
	for (index, (path, season)) in files.iter().enumerate() {
		let name = Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or_default();
		message.push_str(&format!("{}. season {}: {}\n", index, season, name));
	}

	let response = reqwest::blocking::Client::builder()
		.timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
		.build()
		.map_err(|error| error.to_string())?
		.post("https://api.anthropic.com/v1/messages")
		.header("x-api-key", anthropic_key)
		.header("anthropic-version", "2023-06-01")
		// Server-side fallback: a refused request is retried on a fallback model.
		.header("anthropic-beta", "server-side-fallback-2026-07-01")
		.json(&json!({
			"model": MODEL,
			"max_tokens": 16000,
			"system": SYSTEM_PROMPT,
			"messages": [{ "role": "user", "content": message }],
			"output_config": {
				"effort": "low",
				"format": {
					"type": "json_schema",
					"schema": {
						"type": "object",
						"additionalProperties": false,
						"properties": {
							"matches": {
								"type": "array",
								"items": {
									"type": "object",
									"additionalProperties": false,
									"properties": {
										"index": { "type": "integer" },
										"episode": { "type": ["integer", "null"] }
									},
									"required": ["index", "episode"]
								}
							}
						},
						"required": ["matches"]
					}
				}
			},
			"fallbacks": "default"
		}))
		.send()
		.map_err(|error| if error.is_timeout() { format!("sem resposta em {}s", REQUEST_TIMEOUT_SECONDS) } else { error.to_string() })?;

	let status = response.status();
	let body = response.json::<Value>().map_err(|error| format!("resposta invalida: {}", error))?;
	if !status.is_success() {
		return Err(format!("Anthropic {}: {}", status.as_u16(), body.pointer("/error/message").and_then(Value::as_str).unwrap_or("erro")));
	}
	let stop_reason = body["stop_reason"].as_str().unwrap_or_default();
	if stop_reason == "refusal" || stop_reason == "max_tokens" {
		return Err(format!("Claude parou ({})", stop_reason));
	}

	let text = body["content"]
		.as_array()
		.into_iter()
		.flatten()
		.find(|block| block["type"] == "text")
		.and_then(|block| block["text"].as_str())
		.ok_or_else(|| "resposta sem texto".to_string())?;
	let parsed = serde_json::from_str::<Value>(text).map_err(|error| format!("JSON invalido: {}", error))?;

	// Keep only episodes TMDb has for that season, each used once.
	let mut matches = HashMap::new();
	let mut used = Vec::<(i32, u32)>::new();
	for entry in parsed["matches"].as_array().into_iter().flatten() {
		let (Some(index), Some(episode)) = (entry["index"].as_u64(), entry["episode"].as_u64()) else { continue };
		let Some((path, season)) = files.get(index as usize) else { continue };
		let episode = episode as u32;
		let known = names.get(season).is_some_and(|episodes| episodes.contains_key(&episode));
		if known && !used.contains(&(*season, episode)) {
			used.push((*season, episode));
			matches.insert(path.clone(), episode);
		}
	}
	Ok(matches)
}

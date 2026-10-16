//! Claude agent that identifies a dropped folder: what it is, its TMDb entry,
//! and whether it continues a show already in the library. It only proposes
//! an identity; the importer turns that into the plan the user confirms.

use super::{existing_show_dto, item_name, library_video_items, normalize_for_match, ImportIdentity, IMPORT_CATEGORIES};
use crate::app::state::AppState;
use crate::modules::metadata::MetadataModule;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

const MODEL: &str = "claude-opus-5-5";
const MAX_TURNS: usize = 16;
const MAX_LISTED_FILES: usize = 300;
const REQUEST_TIMEOUT_SECONDS: u64 = 150;

const SYSTEM_PROMPT: &str = "You identify a folder (or loose files) the user is adding to NetCrico, their local media library \
(movies, TV shows, documentaries, stand-up specials, concerts). Work out what the folder contains, then call \
submit_identification exactly once.

How to work:
- Read the folder and file names. Release names are noisy (resolution, codecs, group tags); the real title and year are in there.
- Check the user's library with search_library before deciding. If the folder holds episodes of a show the user already has \
(a new season, or missing episodes), set existing_show_id to that show's id; the files will be added to it. Compare seasons \
already present with the seasons in the folder.
- Find the TMDb entry with tmdb_search and set tmdb_id. Use the category's TMDb kind: shows are \"tv\", everything else is \"movie\" \
(documentaries, stand-up specials and concerts are TMDb movies).
- Use web_search only when the names are ambiguous or TMDb doesn't find a clear match (e.g. a concert or stand-up special with an unusual title).
- title is the clean display title (prefer the title the user's library already uses for an existing show; otherwise TMDb's title). \
year is the release year (first air year for shows).
- default_season: for shows, the season number to use when a file's name and folder don't state it; null otherwise.
- If a category was chosen by the user, keep it.
- notes: one or two short sentences for the user about anything uncertain (no notes if it's clear-cut). Write notes in Portuguese.";

/// Returns the identity plus notes for the dry run. `report` receives short
/// progress lines for the UI.
pub fn identify(
	state: &AppState,
	anthropic_key: &str,
	tmdb_key: Option<&str>,
	sources: &[PathBuf],
	forced_category: Option<&str>,
	heuristic: &ImportIdentity,
	report: &dyn Fn(String)
) -> Result<(ImportIdentity, Vec<String>), String> {
	let client = Client::builder()
		.timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
		.build()
		.map_err(|error| error.to_string())?;

	let mut listing = Vec::new();
	let files = sources
		.iter()
		.flat_map(|source| WalkDir::new(source).into_iter().filter_map(Result::ok).map(move |entry| (source, entry)))
		.filter(|(_, entry)| entry.file_type().is_file());
	for (source, entry) in files {
		if listing.len() == MAX_LISTED_FILES {
			listing.push("... (more files omitted)".to_string());
			break;
		}
		// Files inside a dropped folder are listed relative to it; loose files by name.
		let relative = entry.path().strip_prefix(source).ok().filter(|path| !path.as_os_str().is_empty()).unwrap_or_else(|| Path::new(entry.file_name()));
		let size_mb = entry.metadata().map(|metadata| metadata.len() / 1_000_000).unwrap_or_default();
		listing.push(format!("{} ({} MB)", relative.display(), size_mb));
	}

	let category_line = match forced_category {
		Some(category) => format!("The user chose the category: {}.", category),
		None => "Category: decide it.".to_string()
	};
	let user_message = format!(
		"Dropped: {}\n{}\nA filename-based guess (may be wrong): {}\nFiles:\n{}",
		sources.iter().map(|source| item_name(source)).collect::<Vec<_>>().join(", "),
		category_line,
		serde_json::to_string(heuristic).unwrap_or_default(),
		listing.join("\n")
	);

	let mut tools = vec![
		json!({
			"name": "search_library",
			"description": "Search the user's library (movies, shows, documentaries, stand-up, concerts) by title words. Returns id, title, year, category, folder path, tmdbId and, for shows, the season numbers already present.",
			"input_schema": {
				"type": "object",
				"properties": { "query": { "type": "string", "description": "Title words, e.g. \"breaking bad\"." } },
				"required": ["query"]
			}
		}),
		json!({
			"name": "submit_identification",
			"description": "Submit the final identification. Call exactly once, at the end.",
			"strict": true,
			"input_schema": {
				"type": "object",
				"additionalProperties": false,
				"properties": {
					"category": { "type": "string", "enum": IMPORT_CATEGORIES },
					"title": { "type": "string" },
					"year": { "type": ["integer", "null"] },
					"tmdb_id": { "type": ["string", "null"] },
					"existing_show_id": { "type": ["string", "null"], "description": "id from search_library of the show these episodes belong to, or null." },
					"default_season": { "type": ["integer", "null"] },
					"notes": { "type": "array", "items": { "type": "string" } }
				},
				"required": ["category", "title", "year", "tmdb_id", "existing_show_id", "default_season", "notes"]
			}
		}),
		json!({ "type": "web_search_20260209", "name": "web_search", "max_uses": 3 })
	];
	if tmdb_key.is_some() {
		tools.insert(1, json!({
			"name": "tmdb_search",
			"description": "Search TMDb. Returns up to 8 candidates with tmdbId, title, original title, year and a short overview.",
			"input_schema": {
				"type": "object",
				"properties": {
					"query": { "type": "string" },
					"kind": { "type": "string", "enum": ["movie", "tv"] },
					"year": { "type": ["integer", "null"] }
				},
				"required": ["query", "kind"]
			}
		}));
	}

	let mut messages = vec![json!({ "role": "user", "content": user_message })];

	for turn in 1..=MAX_TURNS {
		report(format!("Claude pensando (rodada {})...", turn));
		let started = Instant::now();
		let response = client
			.post("https://api.anthropic.com/v1/messages")
			.header("x-api-key", anthropic_key)
			.header("anthropic-version", "2023-06-01")
			// Server-side fallback: a refused request is retried on a fallback model.
			// Display updates: short progress notes between tool calls, shown in the log.
			.header("anthropic-beta", "server-side-fallback-2026-07-01,thinking-display-updates-2026-08-18")
			.json(&json!({
				"model": MODEL,
				"max_tokens": 16000,
				"system": SYSTEM_PROMPT,
				"tools": tools,
				"messages": messages,
				"thinking": { "type": "adaptive", "display": "updates" },
				"output_config": { "effort": "medium" },
				"fallbacks": "default"
			}))
			.send()
			.map_err(|error| {
				if error.is_timeout() {
					format!("Claude nao respondeu em {}s", REQUEST_TIMEOUT_SECONDS)
				} else {
					format!("sem conexao com a Anthropic: {}", error)
				}
			})?;

		let status = response.status();
		let body = response.json::<Value>().map_err(|error| format!("resposta invalida da Anthropic: {}", error))?;
		if !status.is_success() {
			let message = body.pointer("/error/message").and_then(Value::as_str).unwrap_or("erro desconhecido");
			return Err(format!("Anthropic {}: {}", status.as_u16(), message));
		}

		let content = body.get("content").cloned().unwrap_or_else(|| json!([]));
		let stop_reason = body.get("stop_reason").and_then(Value::as_str).unwrap_or_default();
		report(format!("Claude respondeu em {}s.", started.elapsed().as_secs()));
		for block in content.as_array().into_iter().flatten() {
			match block["type"].as_str().unwrap_or_default() {
				"thinking" => {
					if let Some(note) = block["thinking"].as_str().map(str::trim).filter(|note| !note.is_empty()) {
						report(format!("Claude: {}", note));
					}
				}
				"server_tool_use" => {
					report(format!("Claude pesquisando na web: \"{}\"", block["input"]["query"].as_str().unwrap_or_default()));
				}
				_ => {}
			}
		}
		messages.push(json!({ "role": "assistant", "content": content }));

		match stop_reason {
			"tool_use" => {}
			// The server paused a long web search turn; resend to let it continue.
			"pause_turn" => continue,
			"end_turn" => {
				messages.push(json!({ "role": "user", "content": "Call submit_identification now." }));
				continue;
			}
			other => return Err(format!("Claude parou ({})", other))
		}

		let mut results = Vec::new();
		for block in content.as_array().into_iter().flatten().filter(|block| block["type"] == "tool_use") {
			let id = block["id"].clone();
			let input = &block["input"];

			match block["name"].as_str().unwrap_or_default() {
				"submit_identification" => {
					report(format!(
						"Claude identificou: {} ({}), {}.",
						input["title"].as_str().unwrap_or_default(),
						input["year"].as_i64().map(|year| year.to_string()).unwrap_or("sem ano".to_string()),
						input["category"].as_str().unwrap_or_default()
					));
					return parse_submission(state, heuristic, input);
				}
				"search_library" => {
					let query = input["query"].as_str().unwrap_or_default();
					report(format!("Procurando \"{}\" na biblioteca...", query));
					results.push(tool_result(id, search_library(state, query)));
				}
				"tmdb_search" => {
					let query = input["query"].as_str().unwrap_or_default();
					let kind = if input["kind"] == "tv" { "tv" } else { "movie" };
					let year = input["year"].as_i64().map(|year| year as i32);
					report(format!("Buscando \"{}\" no TMDb...", query));
					let result = MetadataModule::tmdb_search(tmdb_key.unwrap_or_default(), kind, query, year)
						.and_then(|candidates| serde_json::to_value(candidates).map_err(|error| error.to_string()));
					results.push(tool_result(id, result));
				}
				name => results.push(tool_result(id, Err(format!("unknown tool {}", name))))
			}
		}

		messages.push(json!({ "role": "user", "content": results }));
	}

	Err(format!("Claude nao terminou em {} rodadas", MAX_TURNS))
}

fn search_library(state: &AppState, query: &str) -> Result<Value, String> {
	let words = normalize_for_match(query).split_whitespace().map(str::to_string).collect::<Vec<_>>();
	let matches = library_video_items(state)?
		.into_iter()
		.filter(|item| {
			let haystack = normalize_for_match(&format!(
				"{} {} {} {}",
				item.title,
				item.english_title.as_deref().unwrap_or_default(),
				item.original_title.as_deref().unwrap_or_default(),
				item.source_name
			));
			words.iter().all(|word| haystack.contains(word.as_str()))
		})
		.take(15)
		.map(|item| {
			let seasons = (item.media_type == "show")
				.then(|| existing_show_dto(state, &item.id).ok().flatten().map(|show| show.seasons))
				.flatten();
			json!({
				"id": item.id,
				"title": item.title,
				"year": item.year,
				"category": item.media_type,
				"folder": item.media_path,
				"tmdbId": item.tmdb_id,
				"seasons": seasons
			})
		})
		.collect::<Vec<_>>();

	Ok(json!(matches))
}

fn parse_submission(state: &AppState, heuristic: &ImportIdentity, input: &Value) -> Result<(ImportIdentity, Vec<String>), String> {
	let category = input["category"].as_str().unwrap_or_default().to_string();
	if !IMPORT_CATEGORIES.contains(&category.as_str()) {
		return Err(format!("agent returned an invalid category: {}", category));
	}

	let title = input["title"].as_str().map(str::trim).unwrap_or_default().to_string();
	if title.is_empty() {
		return Err("agent returned an empty title".to_string());
	}

	let mut notes = input["notes"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(Value::as_str)
		.map(str::to_string)
		.collect::<Vec<_>>();

	// Only accept a show id that really is a show in the database.
	let existing_show_id = input["existing_show_id"]
		.as_str()
		.filter(|_| category == "show")
		.and_then(|show_id| match existing_show_dto(state, show_id) {
			Ok(Some(show)) => Some(show.id),
			_ => {
				notes.push(format!("Serie existente sugerida nao encontrada: {}", show_id));
				None
			}
		});

	let tmdb_id = input["tmdb_id"]
		.as_str()
		.map(|id| id.trim().to_string())
		.or_else(|| input["tmdb_id"].as_i64().map(|id| id.to_string()))
		.filter(|id| !id.is_empty());

	Ok((
		ImportIdentity {
			source_paths: heuristic.source_paths.clone(),
			category,
			title,
			year: input["year"].as_i64().map(|year| year as i32),
			tmdb_id,
			existing_show_id,
			default_season: input["default_season"].as_u64().map(|season| season as u32),
			library_root: None,
			poster_url: None,
			cover_url: None
		},
		notes
	))
}

fn tool_result(tool_use_id: Value, result: Result<Value, String>) -> Value {
	match result {
		Ok(value) => json!({ "type": "tool_result", "tool_use_id": tool_use_id, "content": value.to_string() }),
		Err(error) => json!({ "type": "tool_result", "tool_use_id": tool_use_id, "content": error, "is_error": true })
	}
}

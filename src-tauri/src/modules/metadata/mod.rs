use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::Duration;

pub struct MetadataModule;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
	pub year: Option<i32>,
	pub poster_path: Option<String>,
	pub cover_path: Option<String>,
	pub genre: Option<String>,
	pub director: Option<String>,
	pub actors: Option<String>,
	pub imdb_rating: Option<String>,
	pub imdb_id: Option<String>,
	pub tmdb_id: Option<String>
}

#[derive(Debug, Clone, Default)]
struct LocalMetadataConfig {
	year: Option<i32>,
	poster_path: Option<String>,
	cover_path: Option<String>,
	genre: Option<String>,
	director: Option<String>,
	actors: Option<String>,
	imdb_rating: Option<String>,
	imdb_id: Option<String>,
	tmdb_id: Option<String>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OmdbResponse {
	response: String,
	#[serde(default)]
	year: Option<String>,
	#[serde(default)]
	genre: Option<String>,
	#[serde(default)]
	director: Option<String>,
	#[serde(default)]
	actors: Option<String>,
	#[serde(default)]
	imdb_rating: Option<String>,
	#[serde(default)]
	imdb_id: Option<String>,
	#[serde(default)]
	poster: Option<String>
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
	#[serde(default)]
	results: Vec<TmdbSearchResult>
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResult {
	id: i64
}

#[derive(Debug, Deserialize)]
struct TmdbDetailsResponse {
	id: i64,
	#[serde(default)]
	poster_path: Option<String>,
	#[serde(default)]
	genres: Vec<TmdbGenre>,
	#[serde(default)]
	credits: Option<TmdbCredits>,
	#[serde(default)]
	created_by: Vec<TmdbCreatedBy>
}

#[derive(Debug, Deserialize)]
struct TmdbGenre {
	name: String
}

#[derive(Debug, Deserialize)]
struct TmdbCredits {
	#[serde(default)]
	cast: Vec<TmdbCast>,
	#[serde(default)]
	crew: Vec<TmdbCrew>
}

#[derive(Debug, Deserialize)]
struct TmdbCast {
	name: String
}

#[derive(Debug, Deserialize)]
struct TmdbCrew {
	job: String,
	name: String
}

#[derive(Debug, Deserialize)]
struct TmdbCreatedBy {
	name: String
}

impl MetadataModule {
	pub fn resolve_video_metadata(
		metadata_root: &Path,
		title: &str,
		year: Option<i32>,
		media_type: &str,
		local_poster_path: Option<String>,
		local_cover_path: Option<String>,
		app_data_dir: &Path,
		allow_remote_enrichment: bool
	) -> VideoMetadata {
		let local_config = read_local_metadata_config(metadata_root);
		let resolved_poster_path = local_config.poster_path.clone().or(local_config.cover_path.clone()).or(local_poster_path.clone());
		let resolved_cover_path = local_config.cover_path.clone().or(local_config.poster_path.clone()).or(local_cover_path).or(local_poster_path);
		let mut resolved = VideoMetadata {
			year: local_config.year.or(year),
			poster_path: resolved_poster_path,
			cover_path: resolved_cover_path,
			genre: local_config.genre.clone(),
			director: local_config.director.clone(),
			actors: local_config.actors.clone(),
			imdb_rating: local_config.imdb_rating.clone(),
			imdb_id: local_config.imdb_id.clone(),
			tmdb_id: local_config.tmdb_id.clone()
		};

		if !allow_remote_enrichment {
			return resolved;
		}

		let cache_key = build_cache_key(
			media_type,
			title,
			resolved.year,
			resolved.imdb_id.as_deref(),
			resolved.tmdb_id.as_deref()
		);
		let cache_root = app_data_dir.join("metadata-cache");
		let poster_cache_root = app_data_dir.join("remote-posters");

		let live_remote = build_http_client().and_then(|client| {
			fetch_live_remote_metadata(
				&client,
				title,
				resolved.year,
				media_type,
				resolved.imdb_id.as_deref(),
				resolved.tmdb_id.as_deref(),
				&poster_cache_root,
				&cache_key
			)
		});

		let mut remote = live_remote.clone().unwrap_or_default();

		if let Some(cached) = read_cached_remote_metadata(&cache_root, &cache_key) {
			remote.merge_missing(cached);
		}

		resolved.merge_missing(remote.clone());

		if live_remote.is_some() && !remote.is_empty() {
			let _ = write_cached_remote_metadata(&cache_root, &cache_key, &remote);
		}

		resolved
	}
}

impl VideoMetadata {
	fn merge_missing(&mut self, incoming: VideoMetadata) {
		if self.year.is_none() {
			self.year = incoming.year;
		}

		if self.poster_path.is_none() {
			self.poster_path = incoming.poster_path.clone().or(incoming.cover_path.clone());
		}

		if self.cover_path.is_none() {
			self.cover_path = incoming.cover_path.or(incoming.poster_path);
		}

		if self.genre.is_none() {
			self.genre = incoming.genre;
		}

		if self.director.is_none() {
			self.director = incoming.director;
		}

		if self.actors.is_none() {
			self.actors = incoming.actors;
		}

		if self.imdb_rating.is_none() {
			self.imdb_rating = incoming.imdb_rating;
		}

		if self.imdb_id.is_none() {
			self.imdb_id = incoming.imdb_id;
		}

		if self.tmdb_id.is_none() {
			self.tmdb_id = incoming.tmdb_id;
		}
	}

	fn is_empty(&self) -> bool {
		self.year.is_none()
			&& self.poster_path.is_none()
			&& self.cover_path.is_none()
			&& self.genre.is_none()
			&& self.director.is_none()
			&& self.actors.is_none()
			&& self.imdb_rating.is_none()
			&& self.imdb_id.is_none()
			&& self.tmdb_id.is_none()
	}
}

fn build_http_client() -> Option<Client> {
	Client::builder()
		.timeout(Duration::from_secs(4))
		.user_agent("tauri-local-cinema/0.1")
		.build()
		.ok()
}

fn fetch_live_remote_metadata(
	client: &Client,
	title: &str,
	year: Option<i32>,
	media_type: &str,
	imdb_id: Option<&str>,
	tmdb_id: Option<&str>,
	poster_cache_root: &Path,
	cache_key: &str
) -> Option<VideoMetadata> {
	let mut metadata = VideoMetadata::default();

	if let Some(omdb_metadata) = fetch_omdb_metadata(client, title, year, media_type, imdb_id, poster_cache_root, cache_key) {
		metadata.merge_missing(omdb_metadata);
	}

	if needs_tmdb_fallback(&metadata) {
		if let Some(tmdb_metadata) = fetch_tmdb_metadata(client, title, year, media_type, tmdb_id, poster_cache_root, cache_key) {
			metadata.merge_missing(tmdb_metadata);
		}
	}

	if metadata.is_empty() {
		None
	} else {
		Some(metadata)
	}
}

fn fetch_omdb_metadata(
	client: &Client,
	title: &str,
	year: Option<i32>,
	media_type: &str,
	imdb_id: Option<&str>,
	poster_cache_root: &Path,
	cache_key: &str
) -> Option<VideoMetadata> {
	let api_key = std::env::var("OMDB_API_KEY").ok().filter(|value| !value.trim().is_empty())?;
	let mut request = client
		.get("https://www.omdbapi.com/")
		.query(&[("apikey", api_key.as_str()), ("r", "json")]);

	if let Some(imdb_id) = imdb_id.filter(|value| !value.trim().is_empty()) {
		request = request.query(&[("i", imdb_id)]);
	} else {
		let omdb_type = if media_type == "show" { "series" } else { "movie" };
		request = request.query(&[("t", title), ("type", omdb_type)]);

		if let Some(year) = year {
			let year_value = year.to_string();
			request = request.query(&[("y", year_value.as_str())]);
		}
	}

	let response = request.send().ok()?.error_for_status().ok()?;
	let payload = response.json::<OmdbResponse>().ok()?;

	if payload.response != "True" {
		return None;
	}

	let poster_path = payload
		.poster
		.as_deref()
		.and_then(normalize_text_value)
		.and_then(|poster_url| cache_remote_poster(client, poster_cache_root, &format!("{}-omdb", cache_key), poster_url));

	Some(VideoMetadata {
		year: payload.year.as_deref().and_then(parse_year_from_text),
		cover_path: poster_path.clone(),
		poster_path,
		genre: payload.genre.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		director: payload.director.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		actors: payload.actors.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		imdb_rating: payload.imdb_rating.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		imdb_id: payload.imdb_id.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		tmdb_id: None
	})
}

fn fetch_tmdb_metadata(
	client: &Client,
	title: &str,
	year: Option<i32>,
	media_type: &str,
	tmdb_id: Option<&str>,
	poster_cache_root: &Path,
	cache_key: &str
) -> Option<VideoMetadata> {
	let api_key = std::env::var("TMDB_API_KEY").ok().filter(|value| !value.trim().is_empty())?;
	let tmdb_type = if media_type == "show" { "tv" } else { "movie" };
	let resolved_tmdb_id = tmdb_id
		.and_then(|value| value.trim().parse::<i64>().ok())
		.or_else(|| search_tmdb_id(client, &api_key, tmdb_type, title, year))?;
	let details_url = format!("https://api.themoviedb.org/3/{}/{}", tmdb_type, resolved_tmdb_id);
	let response = client
		.get(details_url)
		.query(&[("api_key", api_key.as_str()), ("append_to_response", "credits")])
		.send()
		.ok()?
		.error_for_status()
		.ok()?;
	let payload = response.json::<TmdbDetailsResponse>().ok()?;
	let poster_path = payload
		.poster_path
		.as_deref()
		.map(|poster| format!("https://image.tmdb.org/t/p/w780{}", poster))
		.and_then(|poster_url| cache_remote_poster(client, poster_cache_root, &format!("{}-tmdb", cache_key), &poster_url));
	let director = payload
		.credits
		.as_ref()
		.and_then(|credits| {
			credits
				.crew
				.iter()
				.find(|member| member.job.eq_ignore_ascii_case("director"))
				.map(|member| member.name.clone())
		})
		.or_else(|| payload.created_by.first().map(|member| member.name.clone()));
	let actors = payload.credits.as_ref().map(|credits| {
		credits
			.cast
			.iter()
			.take(6)
			.map(|member| member.name.clone())
			.collect::<Vec<_>>()
			.join(", ")
	}).filter(|value| !value.is_empty());
	let genre = (!payload.genres.is_empty()).then(|| {
		payload
			.genres
			.iter()
			.map(|entry| entry.name.clone())
			.collect::<Vec<_>>()
			.join(", ")
	});

	Some(VideoMetadata {
		year,
		cover_path: poster_path.clone(),
		poster_path,
		genre,
		director,
		actors,
		imdb_rating: None,
		imdb_id: None,
		tmdb_id: Some(payload.id.to_string())
	})
}

fn search_tmdb_id(client: &Client, api_key: &str, tmdb_type: &str, title: &str, year: Option<i32>) -> Option<i64> {
	let search_url = format!("https://api.themoviedb.org/3/search/{}", tmdb_type);
	let mut request = client
		.get(search_url)
		.query(&[("api_key", api_key), ("query", title), ("include_adult", "false")]);

	if let Some(year) = year {
		let year_value = year.to_string();
		let year_param = if tmdb_type == "tv" { "first_air_date_year" } else { "year" };
		request = request.query(&[(year_param, year_value.as_str())]);
	}

	request
		.send()
		.ok()?
		.error_for_status()
		.ok()?
		.json::<TmdbSearchResponse>()
		.ok()?
		.results
		.into_iter()
		.next()
		.map(|result| result.id)
}

fn read_local_metadata_config(metadata_root: &Path) -> LocalMetadataConfig {
	for file_name in LOCAL_METADATA_FILE_NAMES {
		let config_path = metadata_root.join(file_name);

		if !config_path.exists() {
			continue;
		}

		let Ok(raw) = fs::read_to_string(&config_path) else {
			continue;
		};
		let Ok(json) = serde_json::from_str::<Value>(&raw) else {
			continue;
		};

		return LocalMetadataConfig {
			year: json.get("year").and_then(value_to_year),
			poster_path: json
				.get("posterPath")
				.or_else(|| json.get("poster"))
				.and_then(Value::as_str)
				.and_then(|value| resolve_local_path(metadata_root, value)),
			cover_path: json
				.get("coverPath")
				.or_else(|| json.get("cover"))
				.and_then(Value::as_str)
				.and_then(|value| resolve_local_path(metadata_root, value)),
			genre: json
				.get("genre")
				.and_then(value_to_string)
				.or_else(|| json.get("genres").and_then(value_to_joined_string)),
			director: json.get("director").and_then(value_to_string),
			actors: json
				.get("actors")
				.and_then(value_to_string)
				.or_else(|| json.get("cast").and_then(value_to_joined_string)),
			imdb_rating: json
				.get("imdbRating")
				.or_else(|| json.get("rating"))
				.and_then(value_to_string),
			imdb_id: json.get("imdbId").and_then(value_to_string),
			tmdb_id: json.get("tmdbId").and_then(value_to_string)
		};
	}

	LocalMetadataConfig::default()
}

fn resolve_local_path(metadata_root: &Path, raw_path: &str) -> Option<String> {
	let trimmed = raw_path.trim();

	if trimmed.is_empty() {
		return None;
	}

	let candidate = Path::new(trimmed);
	let resolved = if candidate.is_absolute() {
		candidate.to_path_buf()
	} else {
		metadata_root.join(candidate)
	};

	resolved.exists().then(|| resolved.to_string_lossy().to_string())
}

fn read_cached_remote_metadata(cache_root: &Path, cache_key: &str) -> Option<VideoMetadata> {
	let cache_path = cache_root.join(format!("{}.json", hash_cache_key(cache_key)));
	let raw = fs::read_to_string(cache_path).ok()?;
	serde_json::from_str(&raw).ok()
}

fn write_cached_remote_metadata(cache_root: &Path, cache_key: &str, metadata: &VideoMetadata) -> Result<(), String> {
	fs::create_dir_all(cache_root).map_err(|error| error.to_string())?;
	let cache_path = cache_root.join(format!("{}.json", hash_cache_key(cache_key)));
	let json = serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())?;
	fs::write(cache_path, json).map_err(|error| error.to_string())
}

fn cache_remote_poster(client: &Client, poster_cache_root: &Path, cache_key: &str, poster_url: &str) -> Option<String> {
	fs::create_dir_all(poster_cache_root).ok()?;
	let extension = infer_extension_from_url(poster_url);
	let poster_path = poster_cache_root.join(format!("{}.{}", hash_cache_key(cache_key), extension));

	if poster_path.exists() {
		return Some(poster_path.to_string_lossy().to_string());
	}

	let bytes = client
		.get(poster_url)
		.send()
		.ok()?
		.error_for_status()
		.ok()?
		.bytes()
		.ok()?;

	fs::write(&poster_path, &bytes).ok()?;
	Some(poster_path.to_string_lossy().to_string())
}

fn needs_tmdb_fallback(metadata: &VideoMetadata) -> bool {
	metadata.poster_path.is_none() || metadata.genre.is_none() || metadata.director.is_none() || metadata.actors.is_none()
}

fn build_cache_key(
	media_type: &str,
	title: &str,
	year: Option<i32>,
	imdb_id: Option<&str>,
	tmdb_id: Option<&str>
) -> String {
	let identity = imdb_id
		.filter(|value| !value.trim().is_empty())
		.map(|value| format!("imdb:{}", value))
		.or_else(|| tmdb_id.filter(|value| !value.trim().is_empty()).map(|value| format!("tmdb:{}", value)))
		.unwrap_or_else(|| format!("{}:{}:{}", media_type, title.to_ascii_lowercase(), year.unwrap_or_default()));

	identity.replace(['\\', '/', ':', '?', '*', '"', '<', '>', '|'], "_")
}

fn hash_cache_key(value: &str) -> String {
	let mut hasher = DefaultHasher::new();
	value.hash(&mut hasher);
	format!("{:016x}", hasher.finish())
}

fn infer_extension_from_url(raw_url: &str) -> &'static str {
	let path = raw_url.split('?').next().unwrap_or(raw_url).to_ascii_lowercase();

	if path.ends_with(".png") {
		return "png";
	}

	if path.ends_with(".webp") {
		return "webp";
	}

	"jpg"
}

fn parse_year_from_text(raw: &str) -> Option<i32> {
	raw
		.split(|character: char| !character.is_ascii_digit())
		.find(|token| token.len() == 4)
		.and_then(|token| token.parse::<i32>().ok())
		.filter(|value| (1900..=2100).contains(value))
}

fn normalize_text_value(raw: &str) -> Option<&str> {
	let trimmed = raw.trim();

	if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") {
		return None;
	}

	Some(trimmed)
}

fn value_to_string(value: &Value) -> Option<String> {
	match value {
		Value::String(raw) => normalize_text_value(raw).map(ToOwned::to_owned),
		Value::Number(number) => Some(number.to_string()),
		_ => None
	}
}

fn value_to_joined_string(value: &Value) -> Option<String> {
	match value {
		Value::Array(items) => {
			let parts = items.iter().filter_map(value_to_string).collect::<Vec<_>>();
			(!parts.is_empty()).then(|| parts.join(", "))
		}
		_ => value_to_string(value)
	}
}

fn value_to_year(value: &Value) -> Option<i32> {
	value_to_string(value).as_deref().and_then(parse_year_from_text)
}

const LOCAL_METADATA_FILE_NAMES: &[&str] = &[".local-cinema.json", "metadata.json", "media.json"];

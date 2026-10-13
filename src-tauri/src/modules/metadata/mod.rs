use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct MetadataModule;

#[derive(Debug, Clone, Default)]
pub struct MetadataApiKeys {
	pub tmdb_api_key: Option<String>,
	pub omdb_api_key: Option<String>
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
	pub year: Option<i32>,
	pub poster_path: Option<String>,
	pub cover_path: Option<String>,
	pub overview: Option<String>,
	pub english_title: Option<String>,
	pub original_title: Option<String>,
	pub genre: Option<String>,
	pub director: Option<String>,
	#[serde(default)]
	pub directors: Vec<DirectorMetadata>,
	pub actors: Option<String>,
	pub country: Option<String>,
	pub imdb_rating: Option<String>,
	pub imdb_id: Option<String>,
	pub tmdb_id: Option<String>
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorMetadata {
	pub name: String,
	pub tmdb_person_id: Option<String>,
	pub image_path: Option<String>
}

#[derive(Debug, Clone, Default)]
struct LocalMetadataConfig {
	has_metadata_file: bool,
	year: Option<i32>,
	poster_path: Option<String>,
	cover_path: Option<String>,
	overview: Option<String>,
	english_title: Option<String>,
	original_title: Option<String>,
	genre: Option<String>,
	director: Option<String>,
	actors: Option<String>,
	country: Option<String>,
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
	country: Option<String>,
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
	id: i64,
	#[serde(default)]
	title: Option<String>,
	#[serde(default)]
	name: Option<String>,
	#[serde(default)]
	original_title: Option<String>,
	#[serde(default)]
	original_name: Option<String>,
	#[serde(default)]
	release_date: Option<String>,
	#[serde(default)]
	first_air_date: Option<String>,
	#[serde(default)]
	popularity: Option<f64>
}

#[derive(Debug, Deserialize)]
struct TmdbDetailsResponse {
	id: i64,
	#[serde(default)]
	title: Option<String>,
	#[serde(default)]
	name: Option<String>,
	#[serde(default)]
	original_title: Option<String>,
	#[serde(default)]
	original_name: Option<String>,
	#[serde(default)]
	poster_path: Option<String>,
	#[serde(default)]
	overview: Option<String>,
	#[serde(default)]
	release_date: Option<String>,
	#[serde(default)]
	first_air_date: Option<String>,
	#[serde(default)]
	vote_average: Option<f64>,
	#[serde(default)]
	genres: Vec<TmdbGenre>,
	#[serde(default)]
	production_countries: Vec<TmdbCountry>,
	#[serde(default)]
	origin_country: Vec<String>,
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
struct TmdbCountry {
	iso_3166_1: String,
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
	id: i64,
	job: String,
	name: String,
	#[serde(default)]
	profile_path: Option<String>
}

#[derive(Debug, Deserialize)]
struct TmdbCreatedBy {
	id: i64,
	name: String,
	#[serde(default)]
	profile_path: Option<String>
}

impl MetadataModule {
	pub fn resolve_video_metadata(
		metadata_root: &Path,
		title: &str,
		year: Option<i32>,
		media_type: &str,
		local_poster_path: Option<String>,
		local_cover_path: Option<String>,
		api_keys: &MetadataApiKeys,
		app_data_dir: &Path,
		allow_remote_enrichment: bool,
		force_remote_refresh: bool
	) -> VideoMetadata {
		metadata_debug(format!(
			"resolve start title={:?} year={:?} media_type={} root={} local_poster={:?} local_cover={:?} allow_remote={}",
			title,
			year,
			media_type,
			metadata_root.display(),
			local_poster_path,
			local_cover_path,
			allow_remote_enrichment
		));

		let local_config = read_local_metadata_config(metadata_root);
		let resolved_poster_path = local_config.poster_path.clone().or(local_config.cover_path.clone()).or(local_poster_path.clone());
		let resolved_cover_path = local_config.cover_path.clone().or(local_config.poster_path.clone()).or(local_cover_path).or(local_poster_path);
		let mut resolved = VideoMetadata {
			year: local_config.year.or(year),
			poster_path: resolved_poster_path,
			cover_path: resolved_cover_path,
			overview: local_config.overview.clone(),
			english_title: local_config.english_title.clone(),
			original_title: local_config.original_title.clone(),
			genre: local_config.genre.clone(),
			director: local_config.director.clone(),
			directors: local_config
				.director
				.clone()
				.map(|name| vec![DirectorMetadata {
					name,
					tmdb_person_id: None,
					image_path: None
				}])
				.unwrap_or_default(),
			actors: local_config.actors.clone(),
			country: local_config.country.clone(),
			imdb_rating: local_config.imdb_rating.clone(),
			imdb_id: local_config.imdb_id.clone(),
			tmdb_id: local_config.tmdb_id.clone()
		};

		if !allow_remote_enrichment {
			metadata_debug("resolve skip remote enrichment".to_string());
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

		if !force_remote_refresh {
			if let Some(cached) = read_cached_remote_metadata(&cache_root, &cache_key) {
				metadata_debug(format!("resolve cache hit cache_key={}", cache_key));
				resolved.merge_missing(cached);
			}

			let has_local_artwork = has_existing_artwork(&resolved);
			let needs_director_images = needs_director_enrichment(&resolved, api_keys, metadata_root, app_data_dir);
			let needs_metadata_fields = !is_metadata_complete_for_scan(&resolved, has_local_artwork);

			if !needs_director_images && !needs_metadata_fields {
				metadata_debug(format!(
					"resolve skip remote metadata already complete title={:?} root={}",
					title,
					metadata_root.display()
				));
				return resolved;
			}

			if !has_remote_metadata_keys(api_keys) {
				metadata_debug(format!(
					"resolve skip remote no api keys title={:?} needs_fields={} needs_director_images={}",
					title,
					needs_metadata_fields,
					needs_director_images
				));
				return resolved;
			}
		}

		let has_local_artwork = has_existing_artwork(&resolved);
		metadata_debug(format!(
			"resolve local state title={:?} resolved_poster={:?} resolved_cover={:?} has_local_artwork={} imdb_id={:?} tmdb_id={:?}",
			title,
			resolved.poster_path,
			resolved.cover_path,
			has_local_artwork,
			resolved.imdb_id,
			resolved.tmdb_id
		));

		let live_remote = build_http_client().and_then(|client| {
			fetch_live_remote_metadata(
				&client,
				metadata_root,
				app_data_dir,
				title,
				resolved.year,
				media_type,
				api_keys,
				resolved.imdb_id.as_deref(),
				resolved.tmdb_id.as_deref(),
				has_local_artwork
			)
		});

		let mut remote = live_remote.clone().unwrap_or_default();

		if !force_remote_refresh {
			resolved.merge_missing(remote.clone());
		} else if let Some(live) = live_remote.clone() {
			resolved.merge_prefer_incoming(live.clone());
			remote = live;
		}

		if live_remote.is_some() && !remote.is_empty() {
			let _ = write_local_metadata_config(metadata_root, &resolved);
		}

		metadata_debug(format!(
			"resolve done title={:?} final_poster={:?} final_cover={:?} genre={:?} director={:?} actors={:?} country={:?} rating={:?} tmdb_id={:?}",
			title,
			resolved.poster_path,
			resolved.cover_path,
			resolved.genre,
			resolved.director,
			resolved.actors,
			resolved.country,
			resolved.imdb_rating,
			resolved.tmdb_id
		));

		resolved
	}
}

impl LocalMetadataConfig {
	fn is_complete(&self) -> bool {
		self.has_metadata_file
			&& self.year.is_some()
			&& self.overview.is_some()
			&& self.english_title.is_some()
			&& self.original_title.is_some()
			&& self.genre.is_some()
			&& self.director.is_some()
			&& self.actors.is_some()
			&& self.country.is_some()
			&& self.imdb_rating.is_some()
			&& (self.imdb_id.is_some() || self.tmdb_id.is_some())
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

		if self.overview.is_none() {
			self.overview = incoming.overview;
		}

		if self.english_title.is_none() {
			self.english_title = incoming.english_title;
		}

		if self.original_title.is_none() {
			self.original_title = incoming.original_title;
		}

		if self.genre.is_none() {
			self.genre = incoming.genre;
		}

		if self.director.is_none() {
			self.director = incoming.director;
		}

		if self.directors.is_empty() {
			self.directors = incoming.directors;
			if self.director.is_none() {
				self.director = join_director_names(&self.directors);
			}
		}

		if self.actors.is_none() {
			self.actors = incoming.actors;
		}

		if self.country.is_none() {
			self.country = incoming.country;
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

	fn merge_prefer_incoming(&mut self, incoming: VideoMetadata) {
		if incoming.year.is_some() {
			self.year = incoming.year;
		}

		if let Some(poster_path) = incoming.poster_path.clone().or(incoming.cover_path.clone()) {
			self.poster_path = Some(poster_path);
		}

		if let Some(cover_path) = incoming.cover_path.clone().or(incoming.poster_path.clone()) {
			self.cover_path = Some(cover_path);
		}

		if incoming.overview.is_some() {
			self.overview = incoming.overview;
		}

		if incoming.english_title.is_some() {
			self.english_title = incoming.english_title;
		}

		if incoming.original_title.is_some() {
			self.original_title = incoming.original_title;
		}

		if incoming.genre.is_some() {
			self.genre = incoming.genre;
		}

		if incoming.director.is_some() {
			self.director = incoming.director;
		}

		if !incoming.directors.is_empty() {
			self.directors = incoming.directors;
			if self.director.is_none() {
				self.director = join_director_names(&self.directors);
			}
		}

		if incoming.actors.is_some() {
			self.actors = incoming.actors;
		}

		if incoming.country.is_some() {
			self.country = incoming.country;
		}

		if incoming.imdb_rating.is_some() {
			self.imdb_rating = incoming.imdb_rating;
		}

		if incoming.imdb_id.is_some() {
			self.imdb_id = incoming.imdb_id;
		}

		if incoming.tmdb_id.is_some() {
			self.tmdb_id = incoming.tmdb_id;
		}
	}

	fn is_empty(&self) -> bool {
		self.year.is_none()
			&& self.poster_path.is_none()
			&& self.cover_path.is_none()
			&& self.overview.is_none()
			&& self.english_title.is_none()
			&& self.original_title.is_none()
			&& self.genre.is_none()
			&& self.director.is_none()
			&& self.directors.is_empty()
			&& self.actors.is_none()
			&& self.country.is_none()
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
	metadata_root: &Path,
	app_data_dir: &Path,
	title: &str,
	year: Option<i32>,
	media_type: &str,
	api_keys: &MetadataApiKeys,
	imdb_id: Option<&str>,
	tmdb_id: Option<&str>,
	has_local_artwork: bool
) -> Option<VideoMetadata> {
	metadata_debug(format!(
		"remote start title={:?} year={:?} media_type={} imdb_id={:?} tmdb_id={:?} has_local_artwork={}",
		title,
		year,
		media_type,
		imdb_id,
		tmdb_id,
		has_local_artwork
	));

	let mut metadata = VideoMetadata::default();

	if let Some(omdb_metadata) = fetch_omdb_metadata(client, metadata_root, title, year, media_type, api_keys, imdb_id, has_local_artwork) {
		metadata.merge_missing(omdb_metadata);
	}

	if needs_tmdb_fallback(&metadata) {
		if let Some(tmdb_metadata) = fetch_tmdb_metadata(
			client,
			metadata_root,
			app_data_dir,
			title,
			year,
			media_type,
			api_keys,
			tmdb_id,
			has_local_artwork
		) {
			metadata.merge_missing(tmdb_metadata);
		}
	}

	if metadata.is_empty() {
		metadata_debug("remote result empty".to_string());
		None
	} else {
		metadata_debug(format!(
			"remote result poster={:?} genre={:?} director={:?} actors={:?} country={:?} rating={:?} tmdb_id={:?}",
			metadata.poster_path,
			metadata.genre,
			metadata.director,
			metadata.actors,
			metadata.country,
			metadata.imdb_rating,
			metadata.tmdb_id
		));
		Some(metadata)
	}
}

fn fetch_omdb_metadata(
	client: &Client,
	metadata_root: &Path,
	title: &str,
	year: Option<i32>,
	media_type: &str,
	api_keys: &MetadataApiKeys,
	imdb_id: Option<&str>,
	has_local_artwork: bool
) -> Option<VideoMetadata> {
	let api_key = api_keys.omdb_api_key.as_deref().filter(|value| !value.trim().is_empty()).map(str::to_string);
	if api_key.is_none() {
		metadata_debug("omdb skip missing OMDB_API_KEY".to_string());
	}
	let api_key = api_key?;
	metadata_debug(format!("omdb request title={:?} year={:?} media_type={} imdb_id={:?}", title, year, media_type, imdb_id));
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

	let response = match request.send() {
		Ok(response) => response,
		Err(error) => {
			metadata_debug(format!("omdb request failed: {}", error));
			return None;
		}
	};
	let response = match response.error_for_status() {
		Ok(response) => response,
		Err(error) => {
			metadata_debug(format!("omdb status error: {}", error));
			return None;
		}
	};
	let payload = match response.json::<OmdbResponse>() {
		Ok(payload) => payload,
		Err(error) => {
			metadata_debug(format!("omdb json error: {}", error));
			return None;
		}
	};

	if payload.response != "True" {
		metadata_debug(format!("omdb no match title={:?} year={:?}", title, year));
		return None;
	}

	let poster_path = payload
		.poster
		.as_deref()
		.and_then(normalize_text_value)
		.and_then(|poster_url| store_poster_in_metadata_root(client, metadata_root, poster_url, has_local_artwork));
	metadata_debug(format!("omdb result poster={:?} imdb_id={:?} rating={:?}", poster_path, payload.imdb_id, payload.imdb_rating));

	Some(VideoMetadata {
		year: payload.year.as_deref().and_then(parse_year_from_text),
		cover_path: poster_path.clone(),
		poster_path,
		overview: None,
		english_title: None,
		original_title: None,
		genre: payload.genre.as_deref().and_then(normalize_text_value).map(normalize_joined_genres),
		director: payload.director.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		directors: Vec::new(),
		actors: payload.actors.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		country: payload.country.as_deref().and_then(normalize_text_value).map(normalize_joined_countries),
		imdb_rating: payload.imdb_rating.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		imdb_id: payload.imdb_id.as_deref().and_then(normalize_text_value).map(ToOwned::to_owned),
		tmdb_id: None
	})
}

fn fetch_tmdb_metadata(
	client: &Client,
	metadata_root: &Path,
	app_data_dir: &Path,
	title: &str,
	year: Option<i32>,
	media_type: &str,
	api_keys: &MetadataApiKeys,
	tmdb_id: Option<&str>,
	has_local_artwork: bool
) -> Option<VideoMetadata> {
	let api_key = api_keys.tmdb_api_key.as_deref().filter(|value| !value.trim().is_empty()).map(str::to_string);
	if api_key.is_none() {
		metadata_debug("tmdb skip missing TMDB_API_KEY".to_string());
	}
	let api_key = api_key?;
	let tmdb_type = if media_type == "show" { "tv" } else { "movie" };
	let resolved_tmdb_id = tmdb_id
		.and_then(|value| value.trim().parse::<i64>().ok())
		.or_else(|| search_tmdb_id(client, &api_key, tmdb_type, title, year))?;
	metadata_debug(format!(
		"tmdb details request title={:?} year={:?} tmdb_type={} resolved_tmdb_id={} has_local_artwork={}",
		title,
		year,
		tmdb_type,
		resolved_tmdb_id,
		has_local_artwork
	));
	let details_url = format!("https://api.themoviedb.org/3/{}/{}", tmdb_type, resolved_tmdb_id);
	let response = match client
		.get(details_url)
		.query(&[("api_key", api_key.as_str()), ("append_to_response", "credits"), ("language", "en-US")])
		.send()
	{
		Ok(response) => response,
		Err(error) => {
			metadata_debug(format!("tmdb details request failed: {}", error));
			return None;
		}
	};
	let response = match response.error_for_status() {
		Ok(response) => response,
		Err(error) => {
			metadata_debug(format!("tmdb details status error: {}", error));
			return None;
		}
	};
	let payload = match response.json::<TmdbDetailsResponse>() {
		Ok(payload) => payload,
		Err(error) => {
			metadata_debug(format!("tmdb details json error: {}", error));
			return None;
		}
	};
	let resolved_year = payload
		.release_date
		.as_deref()
		.and_then(parse_year_from_text)
		.or_else(|| payload.first_air_date.as_deref().and_then(parse_year_from_text))
		.or(year);
	let english_title = payload
		.title
		.as_deref()
		.or(payload.name.as_deref())
		.and_then(normalize_text_value)
		.map(ToOwned::to_owned);
	let original_title = payload
		.original_title
		.as_deref()
		.or(payload.original_name.as_deref())
		.and_then(normalize_text_value)
		.map(ToOwned::to_owned);
	let overview = payload
		.overview
		.as_deref()
		.and_then(normalize_text_value)
		.map(ToOwned::to_owned);
	let poster_path = payload
		.poster_path
		.as_deref()
		.map(|poster| format!("https://image.tmdb.org/t/p/w780{}", poster))
		.and_then(|poster_url| store_poster_in_metadata_root(client, metadata_root, &poster_url, has_local_artwork));
	let directors = payload
		.credits
		.as_ref()
		.map(|credits| {
			credits
				.crew
				.iter()
				.filter(|member| member.job.eq_ignore_ascii_case("director"))
				.map(|member| DirectorMetadata {
					name: member.name.clone(),
					tmdb_person_id: Some(member.id.to_string()),
					image_path: member.profile_path.as_deref().and_then(|profile_path| {
						store_director_image_in_cache(client, metadata_root, app_data_dir, &member.id.to_string(), profile_path)
					})
				})
				.collect::<Vec<_>>()
		})
		.filter(|entries| !entries.is_empty())
		.unwrap_or_else(|| {
			payload
				.created_by
				.iter()
				.map(|member| DirectorMetadata {
					name: member.name.clone(),
					tmdb_person_id: Some(member.id.to_string()),
					image_path: member.profile_path.as_deref().and_then(|profile_path| {
						store_director_image_in_cache(client, metadata_root, app_data_dir, &member.id.to_string(), profile_path)
					})
				})
				.collect::<Vec<_>>()
		});
	let director = join_director_names(&directors);
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
			.map(|entry| normalize_genre_label(&entry.name))
			.filter(|value| !value.is_empty())
			.collect::<Vec<_>>()
			.join(", ")
	});
	let country = normalize_tmdb_countries(&payload.production_countries, &payload.origin_country);
	let rating = payload.vote_average.and_then(format_rating_value);
	metadata_debug(format!(
		"tmdb result id={} poster={:?} year={:?} genre={:?} director={:?} actors={:?} country={:?} rating={:?}",
		payload.id,
		poster_path,
		resolved_year,
		genre,
		director,
		actors,
		country,
		rating
	));

	Some(VideoMetadata {
		year: resolved_year,
		cover_path: poster_path.clone(),
		poster_path,
		overview,
		english_title,
		original_title,
		genre,
		director,
		directors,
		actors,
		country,
		imdb_rating: rating,
		imdb_id: None,
		tmdb_id: Some(payload.id.to_string())
	})
}

fn search_tmdb_id(client: &Client, api_key: &str, tmdb_type: &str, title: &str, year: Option<i32>) -> Option<i64> {
	let search_url = format!("https://api.themoviedb.org/3/search/{}", tmdb_type);
	let mut request = client
		.get(search_url)
		.query(&[("api_key", api_key), ("query", title), ("include_adult", "false"), ("language", "en-US")]);

	if let Some(year) = year {
		let year_value = year.to_string();
		let year_param = if tmdb_type == "tv" { "first_air_date_year" } else { "year" };
		request = request.query(&[(year_param, year_value.as_str())]);
	}

	let results = request
		.send()
		.ok()?
		.error_for_status()
		.ok()?
		.json::<TmdbSearchResponse>()
		.ok()?
		.results;
	let normalized_title = normalize_match_title(title);
	metadata_debug(format!(
		"tmdb search title={:?} year={:?} tmdb_type={} results={}",
		title,
		year,
		tmdb_type,
		results.len()
	));
	for result in results.iter().take(5) {
		let candidate_title = preferred_tmdb_title(result);
		metadata_debug(format!(
			"tmdb candidate id={} title={:?} original={:?} release={:?}/{:?} popularity={:?} score={:.2}",
			result.id,
			candidate_title,
			result.original_title.as_deref().or(result.original_name.as_deref()),
			result.release_date,
			result.first_air_date,
			result.popularity,
			score_tmdb_result(&normalized_title, year, result)
		));
	}

	results
		.into_iter()
		.max_by(|left, right| {
			let left_score = score_tmdb_result(&normalized_title, year, left);
			let right_score = score_tmdb_result(&normalized_title, year, right);
			left_score.partial_cmp(&right_score).unwrap_or(Ordering::Equal)
		})
		.and_then(|result| {
			let score = score_tmdb_result(&normalized_title, year, &result);
			metadata_debug(format!("tmdb best candidate id={} accepted={} score={:.2}", result.id, score >= 30.0, score));
			(score >= 30.0).then_some(result.id)
		})
}

fn score_tmdb_result(normalized_title: &str, year: Option<i32>, result: &TmdbSearchResult) -> f64 {
	let preferred_title = preferred_tmdb_title(result);
	let normalized_candidate = normalize_match_title(preferred_title);
	let normalized_original = normalize_match_title(result.original_title.as_deref().or(result.original_name.as_deref()).unwrap_or_default());

	if normalized_candidate.is_empty() && normalized_original.is_empty() {
		return f64::MIN;
	}

	let candidate_year = result
		.release_date
		.as_deref()
		.and_then(parse_year_from_text)
		.or_else(|| result.first_air_date.as_deref().and_then(parse_year_from_text));

	if (normalized_candidate == normalized_title || normalized_original == normalized_title) && candidate_year == year && year.is_some() {
		return 1_000.0;
	}

	let display_score = score_title_variant(normalized_title, &normalized_candidate);
	let original_score = score_title_variant(normalized_title, &normalized_original);
	let mut score = display_score.max(original_score);
	let popularity = result.popularity.unwrap_or_default().clamp(0.0, 500.0);

	if (!normalized_candidate.is_empty() && (normalized_candidate.starts_with(normalized_title) || normalized_title.starts_with(&normalized_candidate)))
		|| (!normalized_original.is_empty() && (normalized_original.starts_with(normalized_title) || normalized_title.starts_with(&normalized_original)))
	{
		score += 10.0;
	}

	score += (popularity / 100.0).min(6.0);

	if let (Some(expected_year), Some(candidate_year)) = (year, candidate_year) {
		let delta = (expected_year - candidate_year).abs();
		score += match delta {
			0 => 18.0,
			1 => 10.0,
			2 => 4.0,
			_ => -14.0
		};
	}

	if contains_bad_keyword(&normalized_candidate) || contains_bad_keyword(&normalized_original) {
		score -= 18.0;
	}

	score
}

fn contains_bad_keyword(title: &str) -> bool {
	BAD_MATCH_KEYWORDS.iter().any(|keyword| title.contains(keyword))
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
			has_metadata_file: true,
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
			overview: json
				.get("overview")
				.or_else(|| json.get("description"))
				.or_else(|| json.get("plot"))
				.and_then(value_to_string),
			english_title: json
				.get("englishTitle")
				.or_else(|| json.get("english_title"))
				.and_then(value_to_string),
			original_title: json
				.get("originalTitle")
				.or_else(|| json.get("original_title"))
				.and_then(value_to_string),
			genre: json
				.get("genre")
				.and_then(value_to_string)
				.or_else(|| json.get("genres").and_then(value_to_joined_string))
				.map(|value| normalize_joined_genres(&value)),
			director: json.get("director").and_then(value_to_string),
			actors: json
				.get("actors")
				.and_then(value_to_string)
				.or_else(|| json.get("cast").and_then(value_to_joined_string)),
			country: json
				.get("country")
				.or_else(|| json.get("countries"))
				.or_else(|| json.get("nationality"))
				.and_then(value_to_joined_string)
				.map(|value| normalize_joined_countries(&value)),
			imdb_rating: json
				.get("rating")
				.or_else(|| json.get("imdbRating"))
				.and_then(value_to_string),
			imdb_id: json.get("imdbId").and_then(value_to_string),
			tmdb_id: json.get("tmdbId").and_then(value_to_string)
		};
	}

	LocalMetadataConfig::default()
}

fn has_existing_artwork(metadata: &VideoMetadata) -> bool {
	metadata.poster_path.as_deref().is_some_and(path_exists) || metadata.cover_path.as_deref().is_some_and(path_exists)
}

fn path_exists(path: &str) -> bool {
	Path::new(path).exists()
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

fn write_local_metadata_config(metadata_root: &Path, metadata: &VideoMetadata) -> Result<(), String> {
	fs::create_dir_all(metadata_root).map_err(|error| error.to_string())?;
	let path = metadata_root.join(LOCAL_METADATA_FILE_NAMES[0]);
	let mut document = read_local_metadata_document(metadata_root).unwrap_or_else(|| Value::Object(Map::new()));
	let object = document
		.as_object_mut()
		.ok_or_else(|| "Local metadata file is not a JSON object".to_string())?;

	set_json_string(object, "poster", poster_file_name(&metadata.poster_path));
	set_json_string(object, "cover", poster_file_name(&metadata.cover_path));
	set_json_string(object, "overview", metadata.overview.clone());
	set_json_string(object, "englishTitle", metadata.english_title.clone());
	set_json_string(object, "originalTitle", metadata.original_title.clone());
	set_json_number(object, "year", metadata.year);
	set_json_string(object, "genre", metadata.genre.clone());
	set_json_string(object, "director", metadata.director.clone());
	set_json_string(object, "actors", metadata.actors.clone());
	set_json_string(object, "country", metadata.country.clone());
	set_json_string(object, "rating", metadata.imdb_rating.clone());
	set_json_string(object, "imdbId", metadata.imdb_id.clone());
	set_json_string(object, "tmdbId", metadata.tmdb_id.clone());

	let json = serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?;
	fs::write(path, json).map_err(|error| error.to_string())
}

fn read_local_metadata_document(metadata_root: &Path) -> Option<Value> {
	for file_name in LOCAL_METADATA_FILE_NAMES {
		let config_path = metadata_root.join(file_name);
		if !config_path.exists() {
			continue;
		}

		let raw = fs::read_to_string(config_path).ok()?;
		let document = serde_json::from_str::<Value>(&raw).ok()?;
		return document.is_object().then_some(document);
	}

	None
}

fn set_json_string(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
	if let Some(value) = value.filter(|entry| !entry.trim().is_empty()) {
		object.insert(key.to_string(), Value::String(value));
	} else {
		object.remove(key);
	}
}

fn preferred_tmdb_title(result: &TmdbSearchResult) -> &str {
	result.title.as_deref().or(result.name.as_deref()).unwrap_or_default()
}

fn score_title_variant(normalized_title: &str, candidate: &str) -> f64 {
	if candidate.is_empty() {
		return f64::MIN;
	}

	let similarity = dice_coefficient(normalized_title, candidate) * 70.0;
	let token_overlap = token_overlap_score(normalized_title, candidate) * 20.0;
	similarity + token_overlap
}

fn set_json_number(object: &mut Map<String, Value>, key: &str, value: Option<i32>) {
	if let Some(value) = value {
		object.insert(key.to_string(), Value::Number(value.into()));
	} else {
		object.remove(key);
	}
}

fn poster_file_name(path: &Option<String>) -> Option<String> {
	path
		.as_deref()
		.and_then(|value| Path::new(value).file_name().and_then(|segment| segment.to_str()))
		.map(ToOwned::to_owned)
}

fn metadata_debug(message: String) {
	eprintln!("[metadata] {}", message);
}

fn store_poster_in_metadata_root(
	client: &Client,
	metadata_root: &Path,
	poster_url: &str,
	has_local_artwork: bool
) -> Option<String> {
	if has_local_artwork {
		metadata_debug(format!(
			"poster skip existing artwork root={} url={}",
			metadata_root.display(),
			poster_url
		));
		return None;
	}

	if let Err(error) = fs::create_dir_all(metadata_root) {
		metadata_debug(format!("poster create_dir failed root={} error={}", metadata_root.display(), error));
		return None;
	}
	let extension = infer_extension_from_url(poster_url);
	let poster_path = metadata_root.join(format!("poster.{}", extension));
	metadata_debug(format!(
		"poster target root={} url={} path={}",
		metadata_root.display(),
		poster_url,
		poster_path.display()
	));

	if !poster_path.exists() {
		let response = match client.get(poster_url).send() {
			Ok(response) => response,
			Err(error) => {
				metadata_debug(format!("poster request failed url={} error={}", poster_url, error));
				return None;
			}
		};
		let response = match response.error_for_status() {
			Ok(response) => response,
			Err(error) => {
				metadata_debug(format!("poster status error url={} error={}", poster_url, error));
				return None;
			}
		};
		let bytes = match response.bytes() {
			Ok(bytes) => bytes,
			Err(error) => {
				metadata_debug(format!("poster bytes read failed url={} error={}", poster_url, error));
				return None;
			}
		};
		metadata_debug(format!("poster downloaded bytes={} path={}", bytes.len(), poster_path.display()));

		if let Err(error) = fs::write(&poster_path, &bytes) {
			metadata_debug(format!("poster write failed path={} error={}", poster_path.display(), error));
			return None;
		}
	} else {
		metadata_debug(format!("poster already exists path={}", poster_path.display()));
	}

	metadata_debug(format!("poster success path={}", poster_path.display()));
	Some(poster_path.to_string_lossy().to_string())
}

fn store_director_image_in_cache(
	client: &Client,
	metadata_root: &Path,
	app_data_dir: &Path,
	director_key: &str,
	profile_path: &str
) -> Option<String> {
	let image_url = format!("https://image.tmdb.org/t/p/w500{}", profile_path);
	let cache_root = resolve_scanned_library_root(metadata_root)
		.map(|root| root.join(".local-cinema").join("people").join("directors"))
		.unwrap_or_else(|| app_data_dir.join("people").join("directors"));

	if let Err(error) = fs::create_dir_all(&cache_root) {
		metadata_debug(format!("director image create_dir failed root={} error={}", cache_root.display(), error));
		return None;
	}

	let extension = infer_extension_from_url(&image_url);
	let image_path = cache_root.join(format!("{}.{}", director_key, extension));

	if !image_path.exists() {
		let response = match client.get(&image_url).send() {
			Ok(response) => response,
			Err(error) => {
				metadata_debug(format!("director image request failed url={} error={}", image_url, error));
				return None;
			}
		};
		let response = match response.error_for_status() {
			Ok(response) => response,
			Err(error) => {
				metadata_debug(format!("director image status error url={} error={}", image_url, error));
				return None;
			}
		};
		let bytes = match response.bytes() {
			Ok(bytes) => bytes,
			Err(error) => {
				metadata_debug(format!("director image bytes read failed url={} error={}", image_url, error));
				return None;
			}
		};

		if let Err(error) = fs::write(&image_path, &bytes) {
			metadata_debug(format!("director image write failed path={} error={}", image_path.display(), error));
			return None;
		}
	}

	Some(image_path.to_string_lossy().to_string())
}

fn join_director_names(directors: &[DirectorMetadata]) -> Option<String> {
	let joined = directors
		.iter()
		.map(|director| director.name.trim())
		.filter(|name| !name.is_empty())
		.collect::<Vec<_>>()
		.join(", ");

	normalize_text_value(&joined).map(ToOwned::to_owned)
}

fn needs_director_enrichment(
	metadata: &VideoMetadata,
	api_keys: &MetadataApiKeys,
	metadata_root: &Path,
	app_data_dir: &Path
) -> bool {
	let has_tmdb_key = api_keys
		.tmdb_api_key
		.as_deref()
		.is_some_and(|value| !value.trim().is_empty());

	if !has_tmdb_key {
		return false;
	}

	if metadata.directors.is_empty() {
		return metadata.director.is_some();
	}

	let portable_cache_root = resolve_scanned_library_root(metadata_root)
		.map(|root| root.join(".local-cinema").join("people").join("directors"));
	let legacy_cache_root = app_data_dir.join("people").join("directors");

	metadata.directors.iter().any(|director| {
		director.tmdb_person_id.is_none()
			|| director.image_path.as_deref().is_none_or(|path| {
				if !path_exists(path) {
					return true;
				}

				let image_path = Path::new(path);
				let in_portable_root = portable_cache_root
					.as_ref()
					.is_some_and(|root| image_path.starts_with(root));

				if in_portable_root {
					return false;
				}

				// Migrate old AppData paths to portable cache on the scanned library root.
				image_path.starts_with(&legacy_cache_root)
			})
	})
}

fn resolve_scanned_library_root(metadata_root: &Path) -> Option<PathBuf> {
	for candidate in metadata_root.ancestors() {
		if contains_known_library_categories(candidate) {
			return Some(candidate.to_path_buf());
		}
	}

	None
}

fn contains_known_library_categories(path: &Path) -> bool {
	["movies", "shows", "music", "documentary", "documentaries", "standup", "concerts"]
		.iter()
		.any(|category| path.join(category).is_dir())
}

fn has_remote_metadata_keys(api_keys: &MetadataApiKeys) -> bool {
	api_keys
		.tmdb_api_key
		.as_deref()
		.is_some_and(|value| !value.trim().is_empty())
		|| api_keys
			.omdb_api_key
			.as_deref()
			.is_some_and(|value| !value.trim().is_empty())
}

fn is_metadata_complete_for_scan(metadata: &VideoMetadata, has_local_artwork: bool) -> bool {
	has_local_artwork
		&& metadata.year.is_some()
		&& metadata.genre.is_some()
		&& metadata.director.is_some()
		&& metadata.actors.is_some()
		&& metadata.country.is_some()
		&& metadata.imdb_rating.is_some()
}

fn needs_tmdb_fallback(metadata: &VideoMetadata) -> bool {
	metadata.poster_path.is_none()
		|| metadata.english_title.is_none()
		|| metadata.original_title.is_none()
		|| metadata.genre.is_none()
		|| metadata.director.is_none()
		|| metadata.directors.is_empty()
		|| metadata.actors.is_none()
		|| metadata.country.is_none()
		|| metadata.imdb_rating.is_none()
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

fn normalize_match_title(raw: &str) -> String {
	let lower = raw.to_lowercase();
	let mut normalized = String::with_capacity(lower.len());

	for character in lower.chars() {
		let mapped = match character {
			'a'..='z' | '0'..='9' => Some(character),
			'á' | 'à' | 'â' | 'ã' | 'ä' => Some('a'),
			'é' | 'è' | 'ê' | 'ë' => Some('e'),
			'í' | 'ì' | 'î' | 'ï' => Some('i'),
			'ó' | 'ò' | 'ô' | 'õ' | 'ö' => Some('o'),
			'ú' | 'ù' | 'û' | 'ü' => Some('u'),
			'ç' => Some('c'),
			'ñ' => Some('n'),
			_ => None
		};

		if let Some(value) = mapped {
			normalized.push(value);
		} else {
			normalized.push(' ');
		}
	}

	normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn dice_coefficient(left: &str, right: &str) -> f64 {
	if left == right {
		return 1.0;
	}

	let left_chars = left.chars().collect::<Vec<_>>();
	let right_chars = right.chars().collect::<Vec<_>>();

	if left_chars.len() < 2 || right_chars.len() < 2 {
		return 0.0;
	}

	let left_pairs = left_chars.windows(2).map(|pair| pair.iter().collect::<String>()).collect::<Vec<_>>();
	let mut right_pairs = right_chars.windows(2).map(|pair| pair.iter().collect::<String>()).collect::<Vec<_>>();
	let mut matches = 0;

	for left_pair in left_pairs {
		if let Some(index) = right_pairs.iter().position(|right_pair| right_pair == &left_pair) {
			right_pairs.remove(index);
			matches += 1;
		}
	}

	(2.0 * matches as f64) / ((left_chars.len() - 1 + right_chars.len() - 1) as f64)
}

fn token_overlap_score(left: &str, right: &str) -> f64 {
	let left_tokens = left.split_whitespace().collect::<Vec<_>>();
	let right_tokens = right.split_whitespace().collect::<Vec<_>>();

	if left_tokens.is_empty() || right_tokens.is_empty() {
		return 0.0;
	}

	let shared = left_tokens
		.iter()
		.filter(|token| right_tokens.iter().any(|candidate| candidate == *token))
		.count();

	shared as f64 / left_tokens.len().max(right_tokens.len()) as f64
}

fn format_rating_value(value: f64) -> Option<String> {
	(value > 0.0).then(|| format!("{value:.1}"))
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

fn normalize_joined_genres(raw: &str) -> String {
	normalize_joined_values(raw, normalize_genre_label)
}

fn normalize_joined_countries(raw: &str) -> String {
	normalize_joined_values(raw, normalize_country_label)
}

fn normalize_joined_values(raw: &str, mapper: fn(&str) -> String) -> String {
	let mut values = Vec::<String>::new();

	for value in raw.split(',').map(str::trim).filter(|value| !value.is_empty()) {
		let normalized = mapper(value);
		if !normalized.is_empty() && !values.iter().any(|existing| existing == &normalized) {
			values.push(normalized);
		}
	}

	values.join(", ")
}

fn normalize_genre_label(raw: &str) -> String {
	match normalize_match_title(raw).as_str() {
		"action" | "acao" => "Acao".to_string(),
		"adventure" | "aventura" => "Aventura".to_string(),
		"animation" | "animacao" => "Animacao".to_string(),
		"comedy" | "comedia" => "Comedia".to_string(),
		"crime" => "Crime".to_string(),
		"documentary" | "documentario" => "Documentario".to_string(),
		"drama" => "Drama".to_string(),
		"family" | "familia" => "Familia".to_string(),
		"fantasy" | "fantasia" => "Fantasia".to_string(),
		"history" | "historia" => "Historia".to_string(),
		"horror" | "terror" => "Terror".to_string(),
		"music" | "musical" => "Musical".to_string(),
		"mystery" | "misterio" => "Misterio".to_string(),
		"romance" => "Romance".to_string(),
		"science fiction" | "sci fi" | "ficcao cientifica" => "Ficcao cientifica".to_string(),
		"thriller" | "suspense" => "Suspense".to_string(),
		"war" | "guerra" => "Guerra".to_string(),
		"western" => "Faroeste".to_string(),
		_ => raw.trim().to_string()
	}
}

fn normalize_country_label(raw: &str) -> String {
	match normalize_match_title(raw).as_str() {
		"us" | "usa" | "united states" | "united states of america" | "eua" => "EUA".to_string(),
		"br" | "brazil" | "brasil" => "Brasil".to_string(),
		"jp" | "japan" | "japao" => "Japao".to_string(),
		"gb" | "uk" | "united kingdom" | "great britain" | "england" | "reino unido" | "inglaterra" => "Reino Unido".to_string(),
		"fr" | "france" | "franca" => "Franca".to_string(),
		"de" | "germany" | "alemanha" => "Alemanha".to_string(),
		"it" | "italy" | "italia" => "Italia".to_string(),
		"es" | "spain" | "espanha" => "Espanha".to_string(),
		"kr" | "south korea" | "korea south" | "coreia do sul" => "Coreia do Sul".to_string(),
		"cn" | "china" => "China".to_string(),
		"in" | "india" => "India".to_string(),
		"ca" | "canada" => "Canada".to_string(),
		_ => raw.trim().to_string()
	}
}

fn normalize_tmdb_countries(production_countries: &[TmdbCountry], origin_country: &[String]) -> Option<String> {
	if !production_countries.is_empty() {
		let values = production_countries
			.iter()
			.map(|entry| normalize_country_label(if entry.name.trim().is_empty() { &entry.iso_3166_1 } else { &entry.name }))
			.collect::<Vec<_>>();
		let joined = dedupe_joined(values);
		return (!joined.is_empty()).then_some(joined);
	}

	let joined = dedupe_joined(origin_country.iter().map(|entry| normalize_country_label(entry)).collect());
	(!joined.is_empty()).then_some(joined)
}

fn dedupe_joined(values: Vec<String>) -> String {
	let mut unique = Vec::<String>::new();

	for value in values.into_iter().filter(|value| !value.trim().is_empty()) {
		if !unique.iter().any(|existing| existing == &value) {
			unique.push(value);
		}
	}

	unique.join(", ")
}

const LOCAL_METADATA_FILE_NAMES: &[&str] = &[".local-cinema.json", "metadata.json", "media.json"];

const BAD_MATCH_KEYWORDS: &[&str] = &[
	"trailer",
	"teaser",
	"featurette",
	"behind the scenes",
	"behind scenes",
	"making of",
	"interview",
	"special",
	"documentary"
];

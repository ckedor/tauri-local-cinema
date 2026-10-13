use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct DatabaseInfrastructure {
	db_path: PathBuf
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItemRecord {
	pub id: String,
	pub title: String,
	pub media_type: String,
	pub year: Option<i32>,
	pub poster_path: Option<String>,
	pub cover_path: Option<String>,
	pub overview: Option<String>,
	pub english_title: Option<String>,
	pub original_title: Option<String>,
	pub genre: Option<String>,
	pub director: Option<String>,
	pub actors: Option<String>,
	pub country: Option<String>,
	pub imdb_rating: Option<String>,
	pub imdb_id: Option<String>,
	pub tmdb_id: Option<String>,
	pub media_path: String,
	pub subtitle_path: Option<String>,
	pub source_name: String,
	pub show_id: Option<String>,
	pub season_number: Option<i32>,
	pub episode_number: Option<i32>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorRecord {
	pub id: String,
	pub name: String,
	pub tmdb_person_id: Option<String>,
	pub image_path: Option<String>,
	pub biography: Option<String>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorListRecord {
	pub id: String,
	pub name: String,
	pub tmdb_person_id: Option<String>,
	pub image_path: Option<String>,
	pub movie_count: i64
}

#[derive(Debug, Clone)]
pub struct MediaDirectorLinkRecord {
	pub media_item_id: String,
	pub director_id: String,
	pub ordinal: i32
}

impl DatabaseInfrastructure {
	pub fn new(app_data_dir: PathBuf) -> Result<Self, String> {
		fs::create_dir_all(&app_data_dir).map_err(|error| error.to_string())?;

		Ok(Self {
			db_path: app_data_dir.join("local-cinema.db")
		})
	}

	pub fn initialize(&self) -> Result<(), String> {
		let connection = self.connection()?;

		connection
			.execute_batch(
				"
				CREATE TABLE IF NOT EXISTS settings (
					key TEXT PRIMARY KEY,
					value TEXT NOT NULL,
					updated_at INTEGER NOT NULL
				);

				CREATE TABLE IF NOT EXISTS media_items (
					id TEXT PRIMARY KEY,
					title TEXT NOT NULL,
					media_type TEXT NOT NULL,
					year INTEGER,
					poster_path TEXT,
					cover_path TEXT,
					overview TEXT,
					english_title TEXT,
					original_title TEXT,
					genre TEXT,
					director TEXT,
					actors TEXT,
					country TEXT,
					imdb_rating TEXT,
					imdb_id TEXT,
					tmdb_id TEXT,
					media_path TEXT NOT NULL,
					subtitle_path TEXT,
					source_name TEXT NOT NULL,
					show_id TEXT,
					season_number INTEGER,
					episode_number INTEGER,
					created_at INTEGER NOT NULL,
					updated_at INTEGER NOT NULL
				);

				CREATE TABLE IF NOT EXISTS directors (
					id TEXT PRIMARY KEY,
					name TEXT NOT NULL,
					tmdb_person_id TEXT,
					image_path TEXT,
					biography TEXT,
					created_at INTEGER NOT NULL,
					updated_at INTEGER NOT NULL
				);

				CREATE TABLE IF NOT EXISTS media_item_directors (
					media_item_id TEXT NOT NULL,
					director_id TEXT NOT NULL,
					ordinal INTEGER NOT NULL DEFAULT 0,
					PRIMARY KEY (media_item_id, director_id)
				);
				"
			)
			.map_err(|error| error.to_string())?;

		ensure_media_items_column(&connection, "show_id", "TEXT")?;
		ensure_media_items_column(&connection, "season_number", "INTEGER")?;
		ensure_media_items_column(&connection, "episode_number", "INTEGER")?;
		ensure_media_items_column(&connection, "cover_path", "TEXT")?;
		ensure_media_items_column(&connection, "overview", "TEXT")?;
		ensure_media_items_column(&connection, "english_title", "TEXT")?;
		ensure_media_items_column(&connection, "original_title", "TEXT")?;
		ensure_media_items_column(&connection, "genre", "TEXT")?;
		ensure_media_items_column(&connection, "director", "TEXT")?;
		ensure_media_items_column(&connection, "actors", "TEXT")?;
		ensure_media_items_column(&connection, "country", "TEXT")?;
		ensure_media_items_column(&connection, "imdb_rating", "TEXT")?;
		ensure_media_items_column(&connection, "imdb_id", "TEXT")?;
		ensure_media_items_column(&connection, "tmdb_id", "TEXT")?;

		connection
			.execute(
				"
				CREATE INDEX IF NOT EXISTS idx_media_items_show_lookup
				ON media_items(media_type, show_id, season_number, episode_number)
				",
				[]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())?;

		connection
			.execute(
				"CREATE UNIQUE INDEX IF NOT EXISTS idx_directors_tmdb_person_id ON directors(tmdb_person_id)",
				[]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())?;

		connection
			.execute(
				"CREATE INDEX IF NOT EXISTS idx_media_item_directors_director_lookup ON media_item_directors(director_id, ordinal)",
				[]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())?;

		connection
			.execute(
				"CREATE INDEX IF NOT EXISTS idx_media_item_directors_media_lookup ON media_item_directors(media_item_id, ordinal)",
				[]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())?;

		Ok(())
	}

	pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
		self
			.connection()?
			.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
			.optional()
			.map_err(|error| error.to_string())
	}

	pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
		let now = now_ms();

		self
			.connection()?
			.execute(
				"
				INSERT INTO settings (key, value, updated_at)
				VALUES (?1, ?2, ?3)
				ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
				",
				params![key, value, now]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())
	}

	pub fn list_media_items(&self) -> Result<Vec<MediaItemRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					id,
					title,
					media_type,
					year,
					poster_path,
					cover_path,
					overview,
					english_title,
					original_title,
					genre,
					director,
					actors,
					country,
					imdb_rating,
					imdb_id,
					tmdb_id,
					media_path,
					subtitle_path,
					source_name,
					show_id,
					season_number,
					episode_number
				FROM media_items
				WHERE show_id IS NULL
				ORDER BY updated_at DESC, title COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map([], map_media_item_row)
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn get_media_item(&self, media_id: &str) -> Result<Option<MediaItemRecord>, String> {
		self
			.connection()?
			.query_row(
				"
				SELECT
					id,
					title,
					media_type,
					year,
					poster_path,
					cover_path,
					overview,
					english_title,
					original_title,
					genre,
					director,
					actors,
					country,
					imdb_rating,
					imdb_id,
					tmdb_id,
					media_path,
					subtitle_path,
					source_name,
					show_id,
					season_number,
					episode_number
				FROM media_items
				WHERE id = ?1
				",
				params![media_id],
				map_media_item_row
			)
			.optional()
			.map_err(|error| error.to_string())
	}

	pub fn list_show_episode_items(&self, show_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					id,
					title,
					media_type,
					year,
					poster_path,
					cover_path,
					overview,
					english_title,
					original_title,
					genre,
					director,
					actors,
					country,
					imdb_rating,
					imdb_id,
					tmdb_id,
					media_path,
					subtitle_path,
					source_name,
					show_id,
					season_number,
					episode_number
				FROM media_items
				WHERE media_type = 'show_episode' AND show_id = ?1
				ORDER BY
					COALESCE(season_number, 1) ASC,
					COALESCE(episode_number, 0) ASC,
					title COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map(params![show_id], map_media_item_row)
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn list_music_album_items(&self, artist_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					id,
					title,
					media_type,
					year,
					poster_path,
					cover_path,
					overview,
					english_title,
					original_title,
					genre,
					director,
					actors,
					country,
					imdb_rating,
					imdb_id,
					tmdb_id,
					media_path,
					subtitle_path,
					source_name,
					show_id,
					season_number,
					episode_number
				FROM media_items
				WHERE media_type = 'music_album' AND show_id = ?1
				ORDER BY
					COALESCE(year, 0) ASC,
					title COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map(params![artist_id], map_media_item_row)
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn list_music_track_items(&self, album_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					id,
					title,
					media_type,
					year,
					poster_path,
					cover_path,
					overview,
					english_title,
					original_title,
					genre,
					director,
					actors,
					country,
					imdb_rating,
					imdb_id,
					tmdb_id,
					media_path,
					subtitle_path,
					source_name,
					show_id,
					season_number,
					episode_number
				FROM media_items
				WHERE media_type = 'music_track' AND show_id = ?1
				ORDER BY
					COALESCE(season_number, 9999) ASC,
					title COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map(params![album_id], map_media_item_row)
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn list_all_music_album_items(&self) -> Result<Vec<MediaItemRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					id,
					title,
					media_type,
					year,
					poster_path,
					cover_path,
					overview,
					english_title,
					original_title,
					genre,
					director,
					actors,
					country,
					imdb_rating,
					imdb_id,
					tmdb_id,
					media_path,
					subtitle_path,
					source_name,
					show_id,
					season_number,
					episode_number
				FROM media_items
				WHERE media_type = 'music_album'
				ORDER BY
					COALESCE(year, 9999) ASC,
					title COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map([], map_media_item_row)
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn list_movie_directors(&self) -> Result<Vec<DirectorListRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					d.id,
					d.name,
					d.tmdb_person_id,
					d.image_path,
					COUNT(DISTINCT mid.media_item_id) AS movie_count
				FROM directors d
				INNER JOIN media_item_directors mid ON mid.director_id = d.id
				INNER JOIN media_items m ON m.id = mid.media_item_id
				WHERE m.media_type = 'movie'
				GROUP BY d.id, d.name, d.tmdb_person_id, d.image_path
				ORDER BY d.name COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map([], |row| {
				Ok(DirectorListRecord {
					id: row.get(0)?,
					name: row.get(1)?,
					tmdb_person_id: row.get(2)?,
					image_path: row.get(3)?,
					movie_count: row.get(4)?
				})
			})
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn get_director(&self, director_id: &str) -> Result<Option<DirectorRecord>, String> {
		self
			.connection()?
			.query_row(
				"
				SELECT
					id,
					name,
					tmdb_person_id,
					image_path,
					biography
				FROM directors
				WHERE id = ?1
				",
				params![director_id],
				map_director_row
			)
			.optional()
			.map_err(|error| error.to_string())
	}

	pub fn update_director_image_path(&self, director_id: &str, image_path: Option<&str>) -> Result<(), String> {
		self
			.connection()?
			.execute(
				"UPDATE directors SET image_path = ?1, updated_at = ?2 WHERE id = ?3",
				params![image_path, now_ms(), director_id]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())
	}

	pub fn update_director_tmdb_and_image(
		&self,
		director_id: &str,
		tmdb_person_id: Option<&str>,
		image_path: Option<&str>
	) -> Result<(), String> {
		self
			.connection()?
			.execute(
				"
				UPDATE directors
				SET
					tmdb_person_id = COALESCE(?1, tmdb_person_id),
					image_path = ?2,
					updated_at = ?3
				WHERE id = ?4
				",
				params![tmdb_person_id, image_path, now_ms(), director_id]
			)
			.map(|_| ())
			.map_err(|error| error.to_string())
	}

	pub fn list_director_movie_items(&self, director_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		let connection = self.connection()?;
		let mut statement = connection
			.prepare(
				"
				SELECT
					m.id,
					m.title,
					m.media_type,
					m.year,
					m.poster_path,
					m.cover_path,
					m.overview,
					m.english_title,
					m.original_title,
					m.genre,
					m.director,
					m.actors,
					m.country,
					m.imdb_rating,
					m.imdb_id,
					m.tmdb_id,
					m.media_path,
					m.subtitle_path,
					m.source_name,
					m.show_id,
					m.season_number,
					m.episode_number
				FROM media_items m
				INNER JOIN media_item_directors mid ON mid.media_item_id = m.id
				WHERE mid.director_id = ?1 AND m.media_type = 'movie'
				ORDER BY COALESCE(m.year, 0) DESC, m.title COLLATE NOCASE ASC
				"
			)
			.map_err(|error| error.to_string())?;

		let rows = statement
			.query_map(params![director_id], map_media_item_row)
			.map_err(|error| error.to_string())?;

		rows
			.collect::<Result<Vec<_>, _>>()
			.map_err(|error| error.to_string())
	}

	pub fn sync_library(
		&self,
		items: &[MediaItemRecord],
		directors: &[DirectorRecord],
		director_links: &[MediaDirectorLinkRecord]
	) -> Result<(), String> {
		let mut connection = self.connection()?;
		let existing_items = load_all_media_items(&connection)?;
		let mut existing_ids = HashSet::new();
		let mut existing_ids_by_match_key = HashMap::<String, VecDeque<String>>::new();

		for item in &existing_items {
			existing_ids.insert(item.id.clone());
			existing_ids_by_match_key
				.entry(media_match_key(item))
				.or_default()
				.push_back(item.id.clone());
		}

		let transaction = connection.transaction().map_err(|error| error.to_string())?;
		let now = now_ms();
		let mut claimed_ids = HashSet::<String>::new();
		let mut parent_id_map = HashMap::<String, String>::new();
		let mut resolved_media_ids = HashMap::<String, String>::new();

		transaction
			.execute_batch(
				"
				CREATE TEMP TABLE IF NOT EXISTS scanned_media_ids (
					id TEXT PRIMARY KEY
				);
				CREATE TEMP TABLE IF NOT EXISTS scanned_director_ids (
					id TEXT PRIMARY KEY
				);
				CREATE TEMP TABLE IF NOT EXISTS scanned_media_director_links (
					media_item_id TEXT NOT NULL,
					director_id TEXT NOT NULL,
					ordinal INTEGER NOT NULL,
					PRIMARY KEY (media_item_id, director_id)
				);
				DELETE FROM scanned_media_ids;
				DELETE FROM scanned_director_ids;
				DELETE FROM scanned_media_director_links;
				"
			)
			.map_err(|error| error.to_string())?;

		{
			let mut mark_seen_statement = transaction
				.prepare("INSERT OR REPLACE INTO scanned_media_ids (id) VALUES (?1)")
				.map_err(|error| error.to_string())?;
			let mut upsert_statement = transaction
				.prepare(
					"
					INSERT INTO media_items (
						id,
						title,
						media_type,
						year,
						poster_path,
						cover_path,
						overview,
						english_title,
						original_title,
						genre,
						director,
						actors,
						country,
						imdb_rating,
						imdb_id,
						tmdb_id,
						media_path,
						subtitle_path,
						source_name,
						show_id,
						season_number,
						episode_number,
						created_at,
						updated_at
					) VALUES (
						?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
						?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
						?21, ?22, ?23, ?24
					)
					ON CONFLICT(id) DO UPDATE SET
						title = excluded.title,
						media_type = excluded.media_type,
						year = excluded.year,
						poster_path = excluded.poster_path,
						cover_path = excluded.cover_path,
						overview = excluded.overview,
						english_title = excluded.english_title,
						original_title = excluded.original_title,
						genre = excluded.genre,
						director = excluded.director,
						actors = excluded.actors,
						country = excluded.country,
						imdb_rating = excluded.imdb_rating,
						imdb_id = excluded.imdb_id,
						tmdb_id = excluded.tmdb_id,
						media_path = excluded.media_path,
						subtitle_path = excluded.subtitle_path,
						source_name = excluded.source_name,
						show_id = excluded.show_id,
						season_number = excluded.season_number,
						episode_number = excluded.episode_number,
						updated_at = excluded.updated_at
					"
				)
				.map_err(|error| error.to_string())?;

			for item in items {
				let mut resolved_item = item.clone();

				if let Some(parent_id) = item.show_id.as_ref().and_then(|show_id| parent_id_map.get(show_id)) {
					resolved_item.show_id = Some(parent_id.clone());
				}

				let resolved_id = if existing_ids.contains(&resolved_item.id) && !claimed_ids.contains(&resolved_item.id) {
					resolved_item.id.clone()
				} else if let Some(existing_id) = take_next_matching_id(
					&mut existing_ids_by_match_key,
					&media_match_key(&resolved_item),
					&claimed_ids
				) {
					existing_id
				} else {
					resolved_item.id.clone()
				};

				claimed_ids.insert(resolved_id.clone());
				resolved_media_ids.insert(item.id.clone(), resolved_id.clone());

				if is_parent_media_type(&item.media_type) {
					parent_id_map.insert(item.id.clone(), resolved_id.clone());
				}

				resolved_item.id = resolved_id;

				mark_seen_statement
					.execute(params![&resolved_item.id])
					.map_err(|error| error.to_string())?;

				upsert_statement
					.execute(params![
						resolved_item.id,
						resolved_item.title,
						resolved_item.media_type,
						resolved_item.year,
						resolved_item.poster_path,
						resolved_item.cover_path,
						resolved_item.overview,
						resolved_item.english_title,
						resolved_item.original_title,
						resolved_item.genre,
						resolved_item.director,
						resolved_item.actors,
						resolved_item.country,
						resolved_item.imdb_rating,
						resolved_item.imdb_id,
						resolved_item.tmdb_id,
						resolved_item.media_path,
						resolved_item.subtitle_path,
						resolved_item.source_name,
						resolved_item.show_id,
						resolved_item.season_number,
						resolved_item.episode_number,
						now,
						now
					])
					.map_err(|error| error.to_string())?;
			}
		}

		{
			let mut mark_seen_director_statement = transaction
				.prepare("INSERT OR REPLACE INTO scanned_director_ids (id) VALUES (?1)")
				.map_err(|error| error.to_string())?;
			let mut upsert_director_statement = transaction
				.prepare(
					"
					INSERT INTO directors (
						id,
						name,
						tmdb_person_id,
						image_path,
						biography,
						created_at,
						updated_at
					) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
					ON CONFLICT(id) DO UPDATE SET
						name = excluded.name,
						tmdb_person_id = COALESCE(excluded.tmdb_person_id, directors.tmdb_person_id),
						image_path = COALESCE(excluded.image_path, directors.image_path),
						biography = COALESCE(excluded.biography, directors.biography),
						updated_at = excluded.updated_at
					"
				)
				.map_err(|error| error.to_string())?;
			let mut upsert_scanned_link_statement = transaction
				.prepare(
					"
					INSERT INTO scanned_media_director_links (media_item_id, director_id, ordinal)
					VALUES (?1, ?2, ?3)
					ON CONFLICT(media_item_id, director_id) DO UPDATE SET ordinal = excluded.ordinal
					"
				)
				.map_err(|error| error.to_string())?;

			for director in directors {
				mark_seen_director_statement
					.execute(params![&director.id])
					.map_err(|error| error.to_string())?;

				upsert_director_statement
					.execute(params![
						&director.id,
						&director.name,
						director.tmdb_person_id.as_deref(),
						director.image_path.as_deref(),
						director.biography.as_deref(),
						now,
						now
					])
					.map_err(|error| error.to_string())?;
			}

			for link in director_links {
				let resolved_media_id = resolved_media_ids
					.get(&link.media_item_id)
					.cloned()
					.unwrap_or_else(|| link.media_item_id.clone());

				upsert_scanned_link_statement
					.execute(params![resolved_media_id, &link.director_id, link.ordinal])
					.map_err(|error| error.to_string())?;
			}
		}

		transaction
			.execute(
				"DELETE FROM media_items WHERE id NOT IN (SELECT id FROM scanned_media_ids)",
				[]
			)
			.map_err(|error| error.to_string())?;

		transaction
			.execute(
				"
				DELETE FROM media_item_directors
				WHERE NOT EXISTS (
					SELECT 1
					FROM scanned_media_director_links scanned
					WHERE scanned.media_item_id = media_item_directors.media_item_id
					  AND scanned.director_id = media_item_directors.director_id
				)
				",
				[]
			)
			.map_err(|error| error.to_string())?;

		transaction
			.execute(
				"
				INSERT OR REPLACE INTO media_item_directors (media_item_id, director_id, ordinal)
				SELECT media_item_id, director_id, ordinal
				FROM scanned_media_director_links
				",
				[]
			)
			.map_err(|error| error.to_string())?;

		transaction
			.execute(
				"
				DELETE FROM directors
				WHERE id NOT IN (SELECT id FROM scanned_director_ids)
				",
				[]
			)
			.map_err(|error| error.to_string())?;

		transaction.commit().map_err(|error| error.to_string())
	}

	pub fn clear_all_data(&self) -> Result<(), String> {
		let mut connection = self.connection()?;
		let transaction = connection.transaction().map_err(|error| error.to_string())?;

		transaction
			.execute("DELETE FROM media_items", [])
			.map_err(|error| error.to_string())?;

		transaction
			.execute("DELETE FROM media_item_directors", [])
			.map_err(|error| error.to_string())?;

		transaction
			.execute("DELETE FROM directors", [])
			.map_err(|error| error.to_string())?;

		transaction
			.execute("DELETE FROM settings", [])
			.map_err(|error| error.to_string())?;

		transaction.commit().map_err(|error| error.to_string())
	}

	pub fn db_path(&self) -> &Path {
		&self.db_path
	}

	fn connection(&self) -> Result<Connection, String> {
		Connection::open(&self.db_path).map_err(|error| error.to_string())
	}
}

fn ensure_media_items_column(connection: &Connection, column_name: &str, sql_type: &str) -> Result<(), String> {
	let mut statement = connection
		.prepare("PRAGMA table_info(media_items)")
		.map_err(|error| error.to_string())?;
	let rows = statement
		.query_map([], |row| row.get::<_, String>(1))
		.map_err(|error| error.to_string())?;
	let existing_columns = rows
		.collect::<Result<Vec<_>, _>>()
		.map_err(|error| error.to_string())?;

	if existing_columns.iter().any(|column| column == column_name) {
		return Ok(());
	}

	connection
		.execute(
			&format!("ALTER TABLE media_items ADD COLUMN {column_name} {sql_type}"),
			[]
		)
		.map(|_| ())
		.map_err(|error| error.to_string())
}

fn load_all_media_items(connection: &Connection) -> Result<Vec<MediaItemRecord>, String> {
	let mut statement = connection
		.prepare(
			"
			SELECT
				id,
				title,
				media_type,
				year,
				poster_path,
				cover_path,
				overview,
				english_title,
				original_title,
				genre,
				director,
				actors,
				country,
				imdb_rating,
				imdb_id,
				tmdb_id,
				media_path,
				subtitle_path,
				source_name,
				show_id,
				season_number,
				episode_number
			FROM media_items
			"
		)
		.map_err(|error| error.to_string())?;
	let rows = statement
		.query_map([], map_media_item_row)
		.map_err(|error| error.to_string())?;

	rows
		.collect::<Result<Vec<_>, _>>()
		.map_err(|error| error.to_string())
}

fn is_parent_media_type(media_type: &str) -> bool {
	matches!(media_type, "show" | "music_artist" | "music_album")
}

fn take_next_matching_id(
	existing_ids_by_match_key: &mut HashMap<String, VecDeque<String>>,
	match_key: &str,
	claimed_ids: &HashSet<String>
) -> Option<String> {
	let candidates = existing_ids_by_match_key.get_mut(match_key)?;

	while let Some(candidate_id) = candidates.pop_front() {
		if !claimed_ids.contains(&candidate_id) {
			return Some(candidate_id);
		}
	}

	None
}

fn media_match_key(item: &MediaItemRecord) -> String {
	match item.media_type.as_str() {
		"show_episode" => format!(
			"{}|{}|{}|{}|{}",
			item.media_type,
			normalize_identity_fragment(&parent_label(item.show_id.as_deref())),
			item.season_number.unwrap_or_default(),
			item.episode_number.unwrap_or_default(),
			normalize_identity_fragment(&item.title)
		),
		"music_album" => format!(
			"{}|{}|{}|{}",
			item.media_type,
			normalize_identity_fragment(&parent_label(item.show_id.as_deref())),
			normalize_identity_fragment(&item.title),
			item.year.unwrap_or_default()
		),
		"music_track" => format!(
			"{}|{}|{}|{}",
			item.media_type,
			normalize_identity_fragment(&parent_label(item.show_id.as_deref())),
			item.season_number.unwrap_or_default(),
			normalize_identity_fragment(&item.title)
		),
		_ => format!(
			"{}|{}|{}",
			item.media_type,
			normalize_identity_fragment(&item.title),
			item.year.unwrap_or_default()
		)
	}
}

fn parent_label(parent_id: Option<&str>) -> String {
	parent_id
		.and_then(|value| Path::new(value).file_name().and_then(|segment| segment.to_str()))
		.unwrap_or_default()
		.to_string()
}

fn normalize_identity_fragment(raw: &str) -> String {
	let mut normalized = String::with_capacity(raw.len());

	for character in raw.chars() {
		if character.is_alphanumeric() {
			normalized.extend(character.to_lowercase());
		} else {
			normalized.push(' ');
		}
	}

	normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn map_media_item_row(row: &Row<'_>) -> rusqlite::Result<MediaItemRecord> {
	Ok(MediaItemRecord {
		id: row.get(0)?,
		title: row.get(1)?,
		media_type: row.get(2)?,
		year: row.get(3)?,
		poster_path: row.get(4)?,
		cover_path: row.get(5)?,
		overview: row.get(6)?,
		english_title: row.get(7)?,
		original_title: row.get(8)?,
		genre: row.get(9)?,
		director: row.get(10)?,
		actors: row.get(11)?,
		country: row.get(12)?,
		imdb_rating: row.get(13)?,
		imdb_id: row.get(14)?,
		tmdb_id: row.get(15)?,
		media_path: row.get(16)?,
		subtitle_path: row.get(17)?,
		source_name: row.get(18)?,
		show_id: row.get(19)?,
		season_number: row.get(20)?,
		episode_number: row.get(21)?
	})
}

fn map_director_row(row: &Row<'_>) -> rusqlite::Result<DirectorRecord> {
	Ok(DirectorRecord {
		id: row.get(0)?,
		name: row.get(1)?,
		tmdb_person_id: row.get(2)?,
		image_path: row.get(3)?,
		biography: row.get(4)?
	})
}

fn now_ms() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as i64)
		.unwrap_or_default()
}

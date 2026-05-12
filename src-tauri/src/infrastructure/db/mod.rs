use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
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
	pub genre: Option<String>,
	pub director: Option<String>,
	pub actors: Option<String>,
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
					genre TEXT,
					director TEXT,
					actors TEXT,
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
				"
			)
			.map_err(|error| error.to_string())?;

		ensure_media_items_column(&connection, "show_id", "TEXT")?;
		ensure_media_items_column(&connection, "season_number", "INTEGER")?;
		ensure_media_items_column(&connection, "episode_number", "INTEGER")?;
		ensure_media_items_column(&connection, "cover_path", "TEXT")?;
		ensure_media_items_column(&connection, "genre", "TEXT")?;
		ensure_media_items_column(&connection, "director", "TEXT")?;
		ensure_media_items_column(&connection, "actors", "TEXT")?;
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
					genre,
					director,
					actors,
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
					genre,
					director,
					actors,
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
					genre,
					director,
					actors,
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
					genre,
					director,
					actors,
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
					genre,
					director,
					actors,
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
					genre,
					director,
					actors,
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

	pub fn replace_media_items(&self, items: &[MediaItemRecord]) -> Result<(), String> {
		let mut connection = self.connection()?;
		let transaction = connection.transaction().map_err(|error| error.to_string())?;
		let now = now_ms();

		transaction
			.execute("DELETE FROM media_items", [])
			.map_err(|error| error.to_string())?;

		for item in items {
			transaction
				.execute(
					"
					INSERT INTO media_items (
						id,
						title,
						media_type,
						year,
						poster_path,
						cover_path,
						genre,
						director,
						actors,
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
						?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
					)
					",
					params![
						item.id,
						item.title,
						item.media_type,
						item.year,
						item.poster_path,
						item.cover_path,
						item.genre,
						item.director,
						item.actors,
						item.imdb_rating,
						item.imdb_id,
						item.tmdb_id,
						item.media_path,
						item.subtitle_path,
						item.source_name,
						item.show_id,
						item.season_number,
						item.episode_number,
						now,
						now
					]
				)
				.map_err(|error| error.to_string())?;
		}

		transaction.commit().map_err(|error| error.to_string())
	}

	pub fn clear_all_data(&self) -> Result<(), String> {
		let mut connection = self.connection()?;
		let transaction = connection.transaction().map_err(|error| error.to_string())?;

		transaction
			.execute("DELETE FROM media_items", [])
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

fn map_media_item_row(row: &Row<'_>) -> rusqlite::Result<MediaItemRecord> {
	Ok(MediaItemRecord {
		id: row.get(0)?,
		title: row.get(1)?,
		media_type: row.get(2)?,
		year: row.get(3)?,
		poster_path: row.get(4)?,
		cover_path: row.get(5)?,
		genre: row.get(6)?,
		director: row.get(7)?,
		actors: row.get(8)?,
		imdb_rating: row.get(9)?,
		imdb_id: row.get(10)?,
		tmdb_id: row.get(11)?,
		media_path: row.get(12)?,
		subtitle_path: row.get(13)?,
		source_name: row.get(14)?,
		show_id: row.get(15)?,
		season_number: row.get(16)?,
		episode_number: row.get(17)?
	})
}

fn now_ms() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as i64)
		.unwrap_or_default()
}

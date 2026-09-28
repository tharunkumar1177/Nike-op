//! Runner-owned durable application state.
//!
//! SQLite is authoritative. Other processes exchange state through Runner IPC
//! and never open this database directly.

use crate::profile::Profile;
use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Default)]
pub struct StateSnapshot {
    pub profiles: Vec<Profile>,
    pub active_profile: Option<String>,
    pub overlay_visible: bool,
}

pub struct StateStore {
    connection: Connection,
    path: PathBuf,
}

impl StateStore {
    pub fn open_default() -> Result<Self> {
        let project_dirs = ProjectDirs::from("", "", "EdgeOptimizer")
            .ok_or_else(|| anyhow!("failed to determine the EdgeOptimizer data directory"))?;
        let directory = project_dirs.data_local_dir();
        fs::create_dir_all(directory)
            .with_context(|| format!("failed to create state directory {}", directory.display()))?;
        Self::open(directory.join("state.db"))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory {}", parent.display())
            })?;
        }

        let connection = Connection::open(&path)
            .with_context(|| format!("failed to open state database {}", path.display()))?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .context("failed to configure state database")?;

        let mut store = Self { connection, path };
        store.migrate_schema()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn backup_previous_valid(&self) -> Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(FULL);")
            .context("failed to checkpoint state database before backup")?;
        let backup = self.path.with_extension("previous.db");
        let temporary = self.path.with_extension("previous.tmp");
        fs::copy(&self.path, &temporary).with_context(|| {
            format!(
                "failed to create temporary state backup {}",
                temporary.display()
            )
        })?;
        if backup.exists() {
            fs::remove_file(&backup)
                .with_context(|| format!("failed to replace backup {}", backup.display()))?;
        }
        fs::rename(&temporary, &backup)
            .with_context(|| format!("failed to publish backup {}", backup.display()))?;
        Ok(())
    }

    fn migrate_schema(&mut self) -> Result<()> {
        let version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(anyhow!(
                "state database schema {} is newer than supported schema {}",
                version,
                SCHEMA_VERSION
            ));
        }

        if version == 0 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "
                CREATE TABLE profiles (
                    name TEXT PRIMARY KEY COLLATE NOCASE,
                    profile_json TEXT NOT NULL,
                    sort_order INTEGER NOT NULL,
                    updated_at_unix_ms INTEGER NOT NULL
                );
                CREATE TABLE app_state (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    active_profile_name TEXT COLLATE NOCASE NULL,
                    overlay_visible INTEGER NOT NULL DEFAULT 0 CHECK (overlay_visible IN (0, 1)),
                    FOREIGN KEY (active_profile_name) REFERENCES profiles(name)
                        ON UPDATE CASCADE ON DELETE SET NULL
                );
                INSERT INTO app_state(singleton, active_profile_name, overlay_visible)
                    VALUES (1, NULL, 0);
                PRAGMA user_version = 2;
                ",
            )?;
            transaction.commit()?;
        }
        if version == 1 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "
                ALTER TABLE profiles ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
                UPDATE profiles SET sort_order = rowid - 1;
                PRAGMA user_version = 2;
                ",
            )?;
            transaction.commit()?;
        }
        Ok(())
    }

    pub fn load_snapshot(&self) -> Result<StateSnapshot> {
        let mut statement = self
            .connection
            .prepare("SELECT profile_json FROM profiles ORDER BY sort_order")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(
                serde_json::from_str(&row?)
                    .context("failed to deserialize a profile from state.db")?,
            );
        }

        let (active_profile, overlay_visible) = self.connection.query_row(
            "SELECT active_profile_name, overlay_visible FROM app_state WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get::<_, bool>(1)?)),
        )?;
        Ok(StateSnapshot {
            profiles,
            active_profile,
            overlay_visible,
        })
    }

    pub fn save_profiles(&mut self, profiles: &[Profile]) -> Result<()> {
        self.backup_previous_valid()?;
        let transaction = self.connection.transaction()?;
        let active: Option<String> = transaction.query_row(
            "SELECT active_profile_name FROM app_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE app_state SET active_profile_name = NULL WHERE singleton = 1",
            [],
        )?;
        transaction.execute("DELETE FROM profiles", [])?;

        for (sort_order, profile) in profiles.iter().enumerate() {
            let json = serde_json::to_string(profile)?;
            transaction.execute(
                "INSERT INTO profiles(name, profile_json, sort_order, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    profile.name,
                    json,
                    sort_order as i64,
                    crate::orchestration::now_unix_ms() as i64
                ],
            )?;
        }

        if let Some(active) = active {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE name = ?1 COLLATE NOCASE)",
                params![active],
                |row| row.get(0),
            )?;
            if exists {
                transaction.execute(
                    "UPDATE app_state SET active_profile_name = ?1 WHERE singleton = 1",
                    params![active],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn set_active_profile(&mut self, active: Option<&str>) -> Result<()> {
        if let Some(name) = active {
            let exists: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE name = ?1 COLLATE NOCASE)",
                params![name],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(anyhow!("cannot activate unknown profile '{name}'"));
            }
        }
        self.backup_previous_valid()?;
        self.connection.execute(
            "UPDATE app_state SET active_profile_name = ?1 WHERE singleton = 1",
            params![active],
        )?;
        Ok(())
    }

    pub fn set_overlay_visible(&mut self, visible: bool) -> Result<()> {
        self.backup_previous_valid()?;
        self.connection.execute(
            "UPDATE app_state SET overlay_visible = ?1 WHERE singleton = 1",
            params![visible],
        )?;
        Ok(())
    }

    pub fn save_active_profile(&mut self, profile: &Profile) -> Result<()> {
        self.backup_previous_valid()?;
        let transaction = self.connection.transaction()?;
        let json = serde_json::to_string(profile)?;
        transaction.execute(
            "INSERT INTO profiles(name, profile_json, sort_order, updated_at_unix_ms)
             VALUES (?1, ?2, COALESCE((SELECT MAX(sort_order) + 1 FROM profiles), 0), ?3)
             ON CONFLICT(name) DO UPDATE SET profile_json = excluded.profile_json,
                updated_at_unix_ms = excluded.updated_at_unix_ms",
            params![
                profile.name,
                json,
                crate::orchestration::now_unix_ms() as i64
            ],
        )?;
        transaction.execute(
            "UPDATE app_state SET active_profile_name = ?1 WHERE singleton = 1",
            params![profile.name],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn import_legacy_if_empty(
        &mut self,
        profiles: &[Profile],
        active_profile: Option<&str>,
        overlay_visible: bool,
    ) -> Result<bool> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))?;
        if count != 0 || (profiles.is_empty() && active_profile.is_none()) {
            return Ok(false);
        }

        self.save_profiles(profiles)?;
        if active_profile.is_some_and(|name| {
            profiles
                .iter()
                .any(|profile| profile.name.eq_ignore_ascii_case(name))
        }) {
            self.set_active_profile(active_profile)?;
        }
        self.connection.execute(
            "UPDATE app_state SET overlay_visible = ?1 WHERE singleton = 1",
            params![overlay_visible],
        )?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::create_profile;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_database() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "edge-optimizer-state-{}-{suffix}.db",
            std::process::id()
        ))
    }

    fn remove_database(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("db-wal"));
        let _ = fs::remove_file(path.with_extension("db-shm"));
        let _ = fs::remove_file(path.with_extension("previous.db"));
        let _ = fs::remove_file(path.with_extension("previous.tmp"));
    }

    #[test]
    fn persists_and_restores_active_profile() {
        // Verifies profiles and active identity survive closing and reopening Runner-owned state.
        let path = temporary_database();
        {
            let mut store = StateStore::open(&path).unwrap();
            let profiles = vec![
                create_profile("Balanced".into()),
                create_profile("Gaming".into()),
            ];
            store.save_profiles(&profiles).unwrap();
            store.set_active_profile(Some("Gaming")).unwrap();
        }
        let store = StateStore::open(&path).unwrap();
        let snapshot = store.load_snapshot().unwrap();
        assert_eq!(snapshot.profiles.len(), 2);
        assert_eq!(snapshot.active_profile.as_deref(), Some("Gaming"));
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn deleting_the_active_profile_clears_activation() {
        // Verifies removing the active profile transactionally clears its activation reference.
        let path = temporary_database();
        let mut store = StateStore::open(&path).unwrap();
        let gaming = create_profile("Gaming".into());
        store.save_profiles(std::slice::from_ref(&gaming)).unwrap();
        store.set_active_profile(Some("Gaming")).unwrap();
        store.save_profiles(&[]).unwrap();
        assert!(store.load_snapshot().unwrap().active_profile.is_none());
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn rejects_unknown_active_profile() {
        // Verifies referential integrity prevents activation of a profile that does not exist.
        let path = temporary_database();
        let mut store = StateStore::open(&path).unwrap();
        assert!(store.set_active_profile(Some("Missing")).is_err());
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn migrates_schema_version_one_without_losing_state() {
        // Verifies the v1-to-v2 migration preserves stored profile data.
        let path = temporary_database();
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "
                    PRAGMA foreign_keys = ON;
                    CREATE TABLE profiles (
                        name TEXT PRIMARY KEY COLLATE NOCASE,
                        profile_json TEXT NOT NULL,
                        updated_at_unix_ms INTEGER NOT NULL
                    );
                    CREATE TABLE app_state (
                        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                        active_profile_name TEXT COLLATE NOCASE NULL,
                        overlay_visible INTEGER NOT NULL DEFAULT 0,
                        FOREIGN KEY (active_profile_name) REFERENCES profiles(name)
                            ON UPDATE CASCADE ON DELETE SET NULL
                    );
                    INSERT INTO app_state VALUES (1, NULL, 0);
                    PRAGMA user_version = 1;
                    ",
                )
                .unwrap();
            let profile = create_profile("Migrated".into());
            connection
                .execute(
                    "INSERT INTO profiles(name, profile_json, updated_at_unix_ms) VALUES (?1, ?2, 0)",
                    params![profile.name, serde_json::to_string(&profile).unwrap()],
                )
                .unwrap();
        }

        let store = StateStore::open(&path).unwrap();
        let snapshot = store.load_snapshot().unwrap();
        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.profiles[0].name, "Migrated");
        let version: i64 = store
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn creates_current_schema_for_a_fresh_database() {
        // Verifies a new state database creates the current supported schema and default application row.
        let path = temporary_database();
        let store = StateStore::open(&path).unwrap();
        let version: i64 = store
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let snapshot = store.load_snapshot().unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(snapshot.profiles.is_empty());
        assert!(snapshot.active_profile.is_none());
        assert!(!snapshot.overlay_visible);
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn rejects_a_schema_newer_than_the_binary() {
        // Verifies unsupported future schemas fail explicitly instead of resetting valid state.
        let path = temporary_database();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
        drop(connection);
        let error = StateStore::open(&path).err().unwrap().to_string();
        assert!(error.contains("newer than supported"));
        remove_database(&path);
    }

    #[test]
    fn creates_previous_valid_backup_before_mutation() {
        // Verifies a state mutation checkpoints and copies the previous valid database first.
        let path = temporary_database();
        let mut store = StateStore::open(&path).unwrap();
        store
            .save_profiles(&[create_profile("Backed up".into())])
            .unwrap();
        assert!(path.with_extension("previous.db").exists());
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn malformed_profile_json_surfaces_an_error() {
        // Verifies corrupt profile data is reported and never silently replaced with defaults.
        let path = temporary_database();
        let mut store = StateStore::open(&path).unwrap();
        store
            .save_profiles(&[create_profile("Corrupt me".into())])
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE profiles SET profile_json = 'not-json' WHERE name = 'Corrupt me'",
                [],
            )
            .unwrap();
        assert!(store.load_snapshot().is_err());
        drop(store);
        remove_database(&path);
    }
}

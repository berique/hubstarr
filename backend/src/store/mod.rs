/* The page state, in SQLite.

   One table per thing the page has — the instances, the Environment, the
   Configuration. There is a single stack, the one in the `--dir` folder, so no
   table carries a stack id. The database lives in one place
   (`~/.hubstarr/hubstarr.db`); what lives in the stack folder are the generated
   files, and only those.

   The server knows the shape of the page state — that is the price of having
   tables instead of a blob. Fields the page invents later land in `extra`,
   which is the rest of the object as JSON: that way a new flag in `SERVICES`
   does not require a migration to come back whole to the page.

   `load()` rebuilds `{added, defaults, config}` exactly in the shape the page
   sent. That is the criterion of the model: a round trip with no loss. */

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;
use serde_json::{json, Value};

pub mod config;
pub mod env;
pub mod instance;
pub mod migrate;

pub use instance::InstanceIn;

pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    /// Opens the database and creates whatever is missing. Running it again on a
    /// ready database changes nothing — it is the same path the first time and
    /// every time after. A database from the many-stacks model goes through the
    /// migration first, which already builds the new schema; it returns what the
/// migration found, for `main` to report.
    pub fn open(path: &Path) -> Result<(Self, Option<migrate::Migrated>), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let done = migrate::run(&conn)?;
        conn.execute_batch(include_str!("schema.sql"))
            .map_err(|e| e.to_string())?;
        // the schema does not add a column to a table that already exists, and the
        // Environment is the table that grows a column per new key — see `ensure_env_cols`
        env::ensure_env_cols(&conn)?;
        Ok((Db(Arc::new(Mutex::new(conn))), done))
    }

    #[cfg(test)]
    pub fn memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        conn.execute_batch(include_str!("schema.sql"))
            .map_err(|e| e.to_string())?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    #[cfg(test)]
    pub fn from_conn(conn: Connection) -> Self {
        Db(Arc::new(Mutex::new(conn)))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.0.lock().map_err(|_| "banco travado".to_string())
    }

    /// The whole stack state, in the shape the page expects to receive.
    pub fn load(&self) -> Result<Option<Value>, String> {
        let added = self.instances()?;
        let defaults = self.env()?;
        let config = self.config()?;
        // a freshly created database: nothing to restore, and the page keeps its own
        // defaults instead of getting an empty state back
        let empty = config["apps"].as_object().is_none_or(|m| m.is_empty())
            && config["clients"].as_object().is_none_or(|m| m.is_empty())
            && config["mm"].as_object().is_none_or(|m| m.is_empty());
        if added.is_empty() && defaults.is_null() && empty {
            return Ok(None);
        }
        Ok(Some(json!({
            "added": added, "defaults": defaults, "config": config,
        })))
    }
}

/* ---------- JSON helpers ---------- */

pub(crate) fn text(o: &serde_json::Map<String, Value>, k: &str) -> String {
    o.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

pub(crate) fn flag(o: &serde_json::Map<String, Value>, k: &str) -> i64 {
    o.get(k).and_then(|v| v.as_bool()).unwrap_or(false) as i64
}

pub(crate) fn obj(v: Option<&Value>) -> serde_json::Map<String, Value> {
    v.and_then(|v| v.as_object()).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_empty_db_returns_no_state() {
        let db = Db::memory().unwrap();
        assert!(db.load().unwrap().is_none());
    }

    #[test]
    fn with_one_instance_the_state_comes_back_whole() {
        let db = Db::memory().unwrap();
        db.put_instance(&InstanceIn {
            key: "sonarr".into(),
            old: None,
            ord: 0,
            data: json!({"id":"sonarr","title":"Sonarr"}),
        })
        .unwrap();
        let st = db.load().unwrap().expect("há uma instância guardada");
        assert_eq!(st["added"][0]["title"], json!("Sonarr"));
        assert_eq!(st["defaults"], json!(null));
    }
}

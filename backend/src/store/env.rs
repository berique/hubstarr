/* The stack Environment.

   The page's `DEFAULTS` becomes the single `stack_env` row, one column per key.
   `ENV_COLS` is the mapping between the two spellings — camelCase on the page,
   snake_case in the table — and it is the only place it appears: adding a key
   to the Environment means adding a column to `schema.sql` and a line here. */

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};

use super::Db;

/// (key in `DEFAULTS`, column). `tls` is left out: it is the only boolean.
const ENV_COLS: [(&str, &str); 26] = [
    ("restart", "restart"),
    // what tells this stack from another on the machine (v0.6)
    ("project", "project"),
    ("cfg", "cfg"),
    ("data", "data"),
    ("dl", "dl"),
    ("http", "http"),
    ("https", "https"),
    ("puid", "puid"),
    ("pgid", "pgid"),
    ("tz", "tz"),
    ("apiKey", "api_key"),
    ("qbitUser", "qbit_user"),
    ("qbitPass", "qbit_pass"),
    ("qbitKey", "qbit_key"),
    // the Jellyfin administrator, which Deploy uses in its wizard
    ("jfUser", "jf_user"),
    ("jfPass", "jf_pass"),
    ("domain", "domain"),
    ("cert", "cert"),
    ("tlsKey", "tls_key"),
    ("vpnProv", "vpn_prov"),
    ("vpnType", "vpn_type"),
    ("wgKey", "wg_key"),
    ("wgAddr", "wg_addr"),
    ("ovpnUser", "ovpn_user"),
    ("ovpnPass", "ovpn_pass"),
    ("countries", "countries"),
];

/* The `ENV_COLS` columns missing from a database of an earlier version.

   Adding a key to the Environment means adding a column to `schema.sql` and a
   line up there — except that `CREATE TABLE IF NOT EXISTS` does not touch a
   table that already exists, so the database of whoever already had a stack
   ends up without it. And a missing column is no detail: the `SELECT` in
   `env()` names them all, and without one of them **every** read of the
   Environment fails — the page opens with an empty stack and, on the first
   save, `reconcile()` deletes the instances it never saw. That is exactly what
   `jf_user`/`jf_pass` did.

   So the list rules over existing databases too: a missing column comes in as
   empty TEXT, which is the `schema.sql` default and what the page reads as "not
   filled in". `tls` is left out because it is not in `ENV_COLS` — it is the only
   boolean, and it was born with the table. */
pub(crate) fn ensure_env_cols(conn: &Connection) -> Result<(), String> {
    let mut tem = std::collections::HashSet::new();
    {
        let mut q = conn
            .prepare("SELECT name FROM pragma_table_info('stack_env')")
            .map_err(|e| e.to_string())?;
        let lines = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for l in lines {
            tem.insert(l.map_err(|e| e.to_string())?);
        }
    }
    // a table that does not exist yet is schema.sql's business, not ours
    if tem.is_empty() {
        return Ok(());
    }
    for (_, col) in ENV_COLS {
        if !tem.contains(col) {
            conn.execute(
                &format!("ALTER TABLE stack_env ADD COLUMN {col} TEXT NOT NULL DEFAULT ''"),
                [],
            )
            .map_err(|e| format!("acrescentando {col} ao stack_env: {e}"))?;
        }
    }
    Ok(())
}

impl Db {
    /// Writes the Environment. Whatever does not come in the JSON stays as it is.
    pub fn put_env(&self, v: &Value) -> Result<(), String> {
        let o = v
            .as_object()
            .ok_or_else(|| "the Environment did not come as an object".to_string())?;
        let conn = self.lock()?;
        conn.execute("INSERT OR IGNORE INTO stack_env (id) VALUES (1)", [])
            .map_err(|e| e.to_string())?;

        for (key, col) in ENV_COLS {
            if let Some(val) = o.get(key) {
                let s = match val {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                conn.execute(
                    &format!("UPDATE stack_env SET {col} = ?1 WHERE id = 1"),
                    params![s],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        if let Some(tls) = o.get("tls").and_then(|v| v.as_bool()) {
            conn.execute(
                "UPDATE stack_env SET tls = ?1 WHERE id = 1",
                params![tls as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        /* The field names, never the values: among them are the stack key and the
           qBittorrent, Jellyfin and VPN passwords. */
        crate::journal::detail(|| {
            let mut fields: Vec<&str> = ENV_COLS
                .iter()
                .filter(|(k, _)| o.contains_key(*k))
                .map(|(k, _)| *k)
                .collect();
            if o.contains_key("tls") {
                fields.push("tls");
            }
            format!("db: Environment, {} field(s): {}", fields.len(), fields.join(", "))
        });
        Ok(())
    }

    /// The Environment's `BASE_CONFIG` — the root of the configurations the
    /// containers mount. `None` while the Environment has not been written.
    pub fn config_base(&self) -> Result<Option<String>, String> {
        let conn = self.lock()?;
        let v: Option<String> = conn
            .query_row("SELECT cfg FROM stack_env WHERE id = 1", [], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(v.filter(|s| !s.trim().is_empty()))
    }

    pub(crate) fn env(&self) -> Result<Value, String> {
        let cols: Vec<&str> = ENV_COLS.iter().map(|(_, c)| *c).collect();
        let sql = format!(
            "SELECT {}, tls FROM stack_env WHERE id = 1",
            cols.join(", ")
        );
        let conn = self.lock()?;
        let row: Option<Map<String, Value>> = conn
            .query_row(&sql, [], |r| {
                let mut m = Map::new();
                for (i, (key, _)) in ENV_COLS.iter().enumerate() {
                    m.insert((*key).to_string(), Value::String(r.get::<_, String>(i)?));
                }
                m.insert(
                    "tls".into(),
                    Value::Bool(r.get::<_, i64>(ENV_COLS.len())? != 0),
                );
                Ok(m)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(row.map(Value::Object).unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_environment_comes_back_as_it_went_in() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"tz":"America/Sao_Paulo","puid":"1000","tls":true,"domain":"casa.example"}))
            .unwrap();
        let back = db.env().unwrap();
        assert_eq!(back["tz"], json!("America/Sao_Paulo"));
        assert_eq!(back["tls"], json!(true));
        assert_eq!(back["domain"], json!("casa.example"));
        assert_eq!(back["cert"], json!(""));
    }

    /// The Jellyfin credentials go to no file at all, so the database is the only
    /// place where they survive a reload — if they get lost here, Deploy stops
    /// going through the wizard and nobody understands why.
    #[test]
    fn the_jellyfin_credentials_survive_the_round_trip() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"jfUser": "henrique", "jfPass": "segredo"}))
            .unwrap();
        let back = db.env().unwrap();
        assert_eq!(back["jfUser"], json!("henrique"));
        assert_eq!(back["jfPass"], json!("segredo"));
    }

    #[test]
    fn writing_again_keeps_the_row_unique() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"tz":"UTC"})).unwrap();
        db.put_env(&json!({"puid":"1000"})).unwrap();
        let back = db.env().unwrap();
        // the second put created no other row and deleted nothing the first one wrote
        assert_eq!(back["tz"], json!("UTC"));
        assert_eq!(back["puid"], json!("1000"));
    }

    /// A database from an earlier version does not have the columns the
    /// Environment gained later. Without `ensure_env_cols`, the `SELECT` naming
    /// them fails as a whole — and the page, which reads that as "nothing is
    /// stored", opens empty and asks to delete the rest.
    #[test]
    fn a_column_missing_from_an_old_db_is_added_on_open() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // stack_env as it was before jf_user/jf_pass
        conn.execute_batch(
            "CREATE TABLE stack_env (
               id INTEGER PRIMARY KEY CHECK (id = 1),
               restart TEXT NOT NULL DEFAULT '', cfg TEXT NOT NULL DEFAULT '',
               data TEXT NOT NULL DEFAULT '', dl TEXT NOT NULL DEFAULT '',
               http TEXT NOT NULL DEFAULT '', https TEXT NOT NULL DEFAULT '',
               puid TEXT NOT NULL DEFAULT '', pgid TEXT NOT NULL DEFAULT '',
               tz TEXT NOT NULL DEFAULT '', api_key TEXT NOT NULL DEFAULT '',
               qbit_user TEXT NOT NULL DEFAULT '', qbit_pass TEXT NOT NULL DEFAULT '',
               qbit_key TEXT NOT NULL DEFAULT '', tls INTEGER NOT NULL DEFAULT 0,
               domain TEXT NOT NULL DEFAULT '', cert TEXT NOT NULL DEFAULT '',
               tls_key TEXT NOT NULL DEFAULT '', vpn_prov TEXT NOT NULL DEFAULT '',
               vpn_type TEXT NOT NULL DEFAULT '', wg_key TEXT NOT NULL DEFAULT '',
               wg_addr TEXT NOT NULL DEFAULT '', ovpn_user TEXT NOT NULL DEFAULT '',
               ovpn_pass TEXT NOT NULL DEFAULT '', countries TEXT NOT NULL DEFAULT ''
             );
             INSERT INTO stack_env (id, tz) VALUES (1, 'America/Sao_Paulo');",
        )
        .unwrap();
        ensure_env_cols(&conn).unwrap();
        let db = Db::from_conn(conn);
        let back = db.env().unwrap();
        // what was already stored survives, and what was missing comes back empty
        assert_eq!(back["tz"], json!("America/Sao_Paulo"));
        assert_eq!(back["jfUser"], json!(""));
        // and the new column takes writes like any other
        db.put_env(&json!({"jfUser": "henrique"})).unwrap();
        assert_eq!(db.env().unwrap()["jfUser"], json!("henrique"));
    }

    /// Running it again on an up-to-date database changes nothing — it is the
    /// same path as the first open and every one after.
    #[test]
    fn adding_a_column_again_does_nothing() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"jfPass": "segredo"})).unwrap();
        let conn = db.lock().unwrap();
        ensure_env_cols(&conn).unwrap();
        ensure_env_cols(&conn).unwrap();
        drop(conn);
        assert_eq!(db.env().unwrap()["jfPass"], json!("segredo"));
    }

    #[test]
    fn a_new_db_has_no_environment_to_return() {
        let db = Db::memory().unwrap();
        assert_eq!(db.env().unwrap(), json!(null));
    }
}

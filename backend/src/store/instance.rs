/* The instances: one row per added service.

   The key is the page's `cname()` — the `container_name`, which is also the
   config folder and the name the Configuration uses to refer to the service.
   Editing the title changes the key along with it: hence `old`, which turns an
   edit into a rename instead of a new row.

   Every add, edit or delete touches a single row; `reconcile()` is the safety
   net for what does not go through the modal — gluetun and flaresolverr coming
   in on their own, the "Clear" that empties everything. */

use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{flag, text, Db};

/// The `added` fields that become a column or a table of their own. The rest
/// goes to `extra` and comes back spread over the object, as the page sent it.
const COLUMNS: [&str; 10] = [
    "id", "title", "data", "abs", "hw", "tpv", "tpt", "vpn", "solver", "libs",
];

/* The alias column, on a database that already had the table.

   Same trap as the Environment's: `CREATE TABLE IF NOT EXISTS` does not touch a
   table that exists, and the `SELECT` naming the column would fail on every
   read — which the page reads as an empty database, and the first save would
   then wipe the stack. So it is added here, on open, when it is missing. */
pub(crate) fn ensure_lib_cols(conn: &Connection) -> Result<(), String> {
    let has: Option<String> = conn
        .query_row(
            "SELECT name FROM pragma_table_info('instance_lib') WHERE name = 'name'",
            [],
            |r| r.get(0),
        )
        .ok();
    if has.is_some() {
        return Ok(());
    }
    // a table that does not exist yet is schema.sql's business, not ours
    let table: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'instance_lib'",
            [],
            |r| r.get(0),
        )
        .ok();
    if table.is_none() {
        return Ok(());
    }
    conn.execute(
        "ALTER TABLE instance_lib ADD COLUMN name TEXT NOT NULL DEFAULT ''",
        [],
    )
    .map_err(|e| format!("adding name to instance_lib: {e}"))?;
    crate::journal::detail(|| "db: instance_lib gained the alias column".to_string());
    Ok(())
}

/// What the page sends when adding or editing a service.
#[derive(Deserialize)]
pub struct InstanceIn {
    /// the instance key: the `container_name`, which is the page's `cname()`
    pub key: String,
    /// the previous key, when an edit renamed the instance
    #[serde(default)]
    pub old: Option<String>,
    /// position in the list, which is the order in the compose
    #[serde(default)]
    pub ord: i64,
    /// the whole object, as the page keeps it in `added`
    pub data: Value,
}

impl Db {
    pub fn put_instance(&self, inc: &InstanceIn) -> Result<(), String> {
        let o = inc
            .data
            .as_object()
            .ok_or_else(|| "the instance did not come as an object".to_string())?;
        if inc.key.trim().is_empty() {
            return Err("the instance came without a key".into());
        }

        let extra: Map<String, Value> = o
            .iter()
            .filter(|(k, _)| !COLUMNS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let extra = serde_json::to_string(&extra).map_err(|e| e.to_string())?;
        let hw = match text(o, "hw") {
            s if s.is_empty() => "cpu".to_string(),
            s => s,
        };
        /* Each extra folder is `{name, path}` — the alias and the path. A bare
           string is what a page from before the alias sends, and it goes on
           meaning "this path, named after itself"; the empty name is what the
           page reads that way. */
        let libs: Vec<(String, String)> = o
            .get("libs")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| match v {
                        Value::String(p) => Some((String::new(), p.clone())),
                        Value::Object(m) => m
                            .get("path")
                            .and_then(Value::as_str)
                            .map(|p| (text(m, "name"), p.to_string())),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        if let Some(old) = inc.old.as_deref() {
            if old != inc.key {
                tx.execute("DELETE FROM instance WHERE key = ?1", params![old])
                .map_err(|e| e.to_string())?;
            }
        }
        tx.execute(
            "INSERT INTO instance
               (key, ord, service_id, title, data, abs, hw, tpv, tpt, vpn, solver, extra)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(key) DO UPDATE SET
               ord=excluded.ord, service_id=excluded.service_id, title=excluded.title,
               data=excluded.data, abs=excluded.abs, hw=excluded.hw, tpv=excluded.tpv,
               tpt=excluded.tpt, vpn=excluded.vpn, solver=excluded.solver,
               extra=excluded.extra",
            params![
                inc.key,
                inc.ord,
                text(o, "id"),
                text(o, "title"),
                text(o, "data"),
                text(o, "abs"),
                hw,
                text(o, "tpv"),
                text(o, "tpt"),
                flag(o, "vpn"),
                flag(o, "solver"),
                extra,
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "DELETE FROM instance_lib WHERE instance_key = ?1",
            params![inc.key],
        )
        .map_err(|e| e.to_string())?;
        for (i, (name, path)) in libs.iter().enumerate() {
            tx.execute(
                "INSERT INTO instance_lib (instance_key, ord, path, name) VALUES (?1,?2,?3,?4)",
                params![inc.key, i as i64, path, name],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        crate::journal::detail(|| {
            let renamed = match inc.old.as_deref() {
                Some(old) if old != inc.key => format!(" (was {old})"),
                _ => String::new(),
            };
            format!(
                "db: instance {}{renamed}, ord {}, {} extra folder(s)",
                inc.key,
                inc.ord,
                libs.len()
            )
        });
        Ok(())
    }

    pub fn delete_instance(&self, key: &str) -> Result<(), String> {
        let n = self
            .lock()?
            .execute("DELETE FROM instance WHERE key = ?1", params![key])
            .map_err(|e| e.to_string())?;
        crate::journal::detail(|| format!("db: instance {key} deleted ({n} row)"));
        Ok(())
    }

    /// Aligns the database with the list the page has now: fixes the order and
    /// deletes what left. Returns what it deleted — it is the only API operation
    /// that removes an instance without anyone having clicked "Delete", so the
    /// caller records it in the log.
    pub fn reconcile(&self, keys: &[String]) -> Result<Vec<String>, String> {
        let list = serde_json::to_string(keys).map_err(|e| e.to_string())?;
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let left: Vec<String>;
        {
            left = tx
                .prepare(
                    "SELECT key FROM instance
                      WHERE key NOT IN (SELECT value FROM json_each(?1))
                      ORDER BY ord, key",
                )
                .and_then(|mut q| {
                    q.query_map(params![list], |r| r.get(0))?
                        .collect::<Result<Vec<String>, _>>()
                })
                .map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM instance
                  WHERE key NOT IN (SELECT value FROM json_each(?1))",
                params![list],
            )
            .map_err(|e| e.to_string())?;
            let mut up = tx
                .prepare("UPDATE instance SET ord = ?2 WHERE key = ?1")
                .map_err(|e| e.to_string())?;
            for (i, k) in keys.iter().enumerate() {
                up.execute(params![k, i as i64])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        crate::journal::detail(|| {
            format!(
                "db: list with {} key(s){}",
                keys.len(),
                if left.is_empty() {
                    String::new()
                } else {
                    format!(", deleted {}", left.join(", "))
                }
            )
        });
        Ok(left)
    }

    /// Rebuilds the page's `added`: in list order, with the `libs` back as an
    /// array and `extra` spread over the object.
    pub(crate) fn instances(&self) -> Result<Vec<Value>, String> {
        let conn = self.lock()?;
        let mut st = conn
            .prepare(
                "SELECT key, service_id, title, data, abs, hw, tpv, tpt, vpn, solver, extra
                   FROM instance ORDER BY ord, key",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, Value, String)> = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    json!({
                        "id":     r.get::<_, String>(1)?,
                        "title":  r.get::<_, String>(2)?,
                        "data":   r.get::<_, String>(3)?,
                        "abs":    r.get::<_, String>(4)?,
                        "hw":     r.get::<_, String>(5)?,
                        "tpv":    r.get::<_, String>(6)?,
                        "tpt":    r.get::<_, String>(7)?,
                        "vpn":    r.get::<_, i64>(8)? != 0,
                        "solver": r.get::<_, i64>(9)? != 0,
                    }),
                    r.get::<_, String>(10)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;

        let mut libs = conn
            .prepare(
                "SELECT path, name FROM instance_lib WHERE instance_key = ?1 ORDER BY ord",
            )
            .map_err(|e| e.to_string())?;

        let mut out = Vec::with_capacity(rows.len());
        for (key, mut v, extra) in rows {
            let paths: Vec<Value> = libs
                .query_map(params![key], |r| {
                    Ok(json!({"path": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?}))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<Value>, _>>()
                .map_err(|e| e.to_string())?;
            let o = v.as_object_mut().expect("montado como objeto acima");
            o.insert("libs".into(), Value::Array(paths));
            if let Ok(Value::Object(rest)) = serde_json::from_str::<Value>(&extra) {
                for (k, val) in rest {
                    o.insert(k, val);
                }
            }
            out.push(v);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inc(key: &str, old: Option<&str>, ord: i64, data: Value) -> InstanceIn {
        InstanceIn { key: key.into(), old: old.map(String::from), ord, data }
    }

    #[test]
    fn editing_with_a_rename_moves_the_row_instead_of_duplicating_it() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc("sonarr", None, 0, json!({"id":"sonarr","title":"Sonarr"})))
            .unwrap();
        db.put_instance(&inc("series", Some("sonarr"), 0, json!({"id":"sonarr","title":"Séries"})))
            .unwrap();
        let all = db.instances().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["title"], json!("Séries"));
    }

    #[test]
    fn the_jellyfin_folders_come_back_in_order() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc(
            "jellyfin",
            None,
            0,
            json!({"id":"jellyfin","title":"Jellyfin",
                   "libs":[{"name":"disco2","path":"/mnt/a"},{"name":"","path":"/mnt/b"}]}),
        ))
        .unwrap();
        let all = db.instances().unwrap();
        assert_eq!(
            all[0]["libs"],
            json!([{"path":"/mnt/a","name":"disco2"}, {"path":"/mnt/b","name":""}])
        );
    }

    /// A page from before the alias sends bare paths, and they go on meaning
    /// "this path, named after itself" — which is the empty alias.
    #[test]
    fn a_folder_that_came_as_a_bare_path_keeps_working() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc(
            "jellyfin",
            None,
            0,
            json!({"id":"jellyfin","title":"Jellyfin","libs":["/mnt/a"]}),
        ))
        .unwrap();
        assert_eq!(db.instances().unwrap()[0]["libs"], json!([{"path":"/mnt/a","name":""}]));
    }

    /// The column on a database that already had the table: without it every
    /// read of the folders fails, the page takes that for an empty database and
    /// the first save wipes the stack. Same trap as the Environment's.
    #[test]
    fn the_alias_column_is_added_to_a_database_that_predates_it() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE instance (key TEXT PRIMARY KEY, ord INTEGER NOT NULL DEFAULT 0,
               service_id TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
               data TEXT NOT NULL DEFAULT '', abs TEXT NOT NULL DEFAULT '',
               hw TEXT NOT NULL DEFAULT 'cpu', tpv TEXT NOT NULL DEFAULT 'std',
               tpt TEXT NOT NULL DEFAULT 'organizr', vpn INTEGER NOT NULL DEFAULT 0,
               solver INTEGER NOT NULL DEFAULT 0, extra TEXT NOT NULL DEFAULT '{}');
             CREATE TABLE instance_lib (instance_key TEXT NOT NULL, ord INTEGER NOT NULL,
               path TEXT NOT NULL, PRIMARY KEY (instance_key, ord));
             INSERT INTO instance (key, service_id, title) VALUES ('jellyfin','jellyfin','Jellyfin');
             INSERT INTO instance_lib (instance_key, ord, path) VALUES ('jellyfin',0,'/mnt/a');",
        )
        .unwrap();
        ensure_lib_cols(&conn).unwrap();
        // running it again does nothing, like every other open
        ensure_lib_cols(&conn).unwrap();
        let db = Db::from_conn(conn);
        assert_eq!(db.instances().unwrap()[0]["libs"], json!([{"path":"/mnt/a","name":""}]));
    }

    #[test]
    fn deleting_the_instance_takes_its_folders_along() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc("jellyfin", None, 0, json!({"id":"jellyfin","libs":["/mnt/a"]})))
            .unwrap();
        db.delete_instance("jellyfin").unwrap();
        db.put_instance(&inc("jellyfin", None, 0, json!({"id":"jellyfin"})))
            .unwrap();
        // without the CASCADE, the previous instance's folder would come back in this one
        assert_eq!(db.instances().unwrap()[0]["libs"], json!([]));
    }

    #[test]
    fn what_is_not_a_column_comes_back_whole() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc("qbittorrent", None, 0, json!({"id":"qbittorrent","flagNova":42})))
            .unwrap();
        let all = db.instances().unwrap();
        assert_eq!(all[0]["flagNova"], json!(42));
    }

    #[test]
    fn reconcile_deletes_what_left_and_fixes_the_order() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc("a", None, 0, json!({"id":"sonarr"}))).unwrap();
        db.put_instance(&inc("b", None, 1, json!({"id":"radarr"}))).unwrap();
        let left = db.reconcile(&["b".to_string()]).unwrap();
        // what left goes back to the caller, who is the one writing it to the log
        assert_eq!(left, vec!["a".to_string()]);
        let all = db.instances().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["id"], json!("radarr"));
    }

    /// A list identical to the stored one deletes nothing, and that is the common
    /// case — `saveSettings()` runs at the end of every `render()`.
    #[test]
    fn reconcile_with_no_news_deletes_nothing() {
        let db = Db::memory().unwrap();
        db.put_instance(&inc("a", None, 0, json!({"id":"sonarr"}))).unwrap();
        let left = db.reconcile(&["a".to_string()]).unwrap();
        assert!(left.is_empty());
        assert_eq!(db.instances().unwrap().len(), 1);
    }
}

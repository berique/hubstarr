/* The instances: one row per added service.

   The key is the page's `cname()` — the `container_name`, which is also the
   config folder and the name the Configuration uses to refer to the service.
   Editing the title changes the key along with it: hence `old`, which turns an
   edit into a rename instead of a new row.

   Every add, edit or delete touches a single row; `reconcile()` is the safety
   net for what does not go through the modal — gluetun and flaresolverr coming
   in on their own, the "Clear" that empties everything. */

use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{flag, text, Db};

/// The `added` fields that become a column or a table of their own. The rest
/// goes to `extra` and comes back spread over the object, as the page sent it.
const COLUMNS: [&str; 10] = [
    "id", "title", "data", "abs", "hw", "tpv", "tpt", "vpn", "solver", "libs",
];

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
        let libs: Vec<String> = o
            .get("libs")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
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
        for (i, path) in libs.iter().enumerate() {
            tx.execute(
                "INSERT INTO instance_lib (instance_key, ord, path) VALUES (?1,?2,?3)",
                params![inc.key, i as i64, path],
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
                "SELECT path FROM instance_lib WHERE instance_key = ?1 ORDER BY ord",
            )
            .map_err(|e| e.to_string())?;

        let mut out = Vec::with_capacity(rows.len());
        for (key, mut v, extra) in rows {
            let paths: Vec<Value> = libs
                .query_map(params![key], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(Value::String)
                .collect();
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
            json!({"id":"jellyfin","title":"Jellyfin","libs":["/mnt/a","/mnt/b"]}),
        ))
        .unwrap();
        let all = db.instances().unwrap();
        assert_eq!(all[0]["libs"], json!(["/mnt/a", "/mnt/b"]));
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

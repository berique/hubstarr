/* Migration from the many-stacks database to the single-stack one.

   The previous model had a `stack` table at the root and a `stack_id` in every
   other table. Today there is one stack — the one in the `--dir` folder — and
   no table carries an id. A database written by the old model does not work as
   it is: the old columns are `NOT NULL` and the `CREATE TABLE IF NOT EXISTS` in
   `schema.sql` does not touch a table that already exists.

   So, before `schema.sql`: the old tables get the `old_` prefix, the new schema
   is born beside them, one stack is copied into it and `old_` goes away.
   Copying more than one would make no sense — there is nowhere left to keep the
   second — so the lowest id wins, which is the one the page opened by default.
   The others are lost on purpose, and the `dir` of each is announced on the
   output for whoever wants to reuse the files already written. */

use rusqlite::{Connection, OptionalExtension};

/// The columns of each table, without `stack_id`. The order is the same on
/// both sides of the `INSERT ... SELECT`.
const TABLES: [(&str, &str); 7] = [
    (
        "instance",
        "key, ord, service_id, title, data, abs, hw, tpv, tpt, vpn, solver, extra",
    ),
    ("instance_lib", "instance_key, ord, path"),
    ("cfg_app", "arr_key, enabled"),
    ("cfg_client", "client_key, cdh_completed, cdh_failed"),
    ("cfg_client_arr", "client_key, arr_key, enabled, category"),
    (
        "cfg_mm",
        "service_id, hardlink, rename, perms, empty, chmod, chown",
    ),
    ("cfg_naming", "service_id, field, value"),
];

const ENV_COLS: &str = "restart, cfg, data, dl, http, https, puid, pgid, tz, api_key, \
     qbit_user, qbit_pass, qbit_key, tls, domain, cert, tls_key, vpn_prov, vpn_type, \
     wg_key, wg_addr, ovpn_user, ovpn_pass, countries";

/// What the migration found, for `main` to report to whoever is watching.
pub struct Migrated {
    /// the folder of the stack that survived
    pub kept: String,
    /// the folders of the stacks that did not fit the new model
    pub dropped: Vec<String>,
}

fn has_table(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .map(|o| o.is_some())
    .map_err(|e| e.to_string())
}

/// Runs before `schema.sql`. Returns `None` on a database that is already the
/// new model — or empty, which is the same path as the first time.
pub fn run(conn: &Connection) -> Result<Option<Migrated>, String> {
    if !has_table(conn, "stack")? {
        return Ok(None);
    }

    // the lowest id is the one the page opened by default; the others have
    // nowhere left to fit, and become just a warning on the output
    let keep: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, dir FROM stack ORDER BY id LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let dropped: Vec<String> = conn
        .prepare("SELECT dir FROM stack WHERE id <> ?1 ORDER BY id")
        .and_then(|mut st| {
            st.query_map([keep.as_ref().map_or(-1, |(id, _)| *id)], |r| r.get(0))?
                .collect()
        })
        .map_err(|e| e.to_string())?;

    // the rename must not rewrite the foreign keys of the other tables:
    // it is the old schema being set aside whole
    conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")
        .map_err(|e| e.to_string())?;

    let mut names: Vec<&str> = TABLES.iter().map(|(t, _)| *t).collect();
    names.push("stack_env");
    for t in &names {
        if has_table(conn, t)? {
            conn.execute_batch(&format!("ALTER TABLE {t} RENAME TO old_{t}"))
                .map_err(|e| e.to_string())?;
        }
    }
    conn.execute_batch("ALTER TABLE stack RENAME TO old_stack")
        .map_err(|e| e.to_string())?;

    // `schema.sql` turns `foreign_keys` back on; the old tables still point at
    // the names that now belong to the new schema, and with the check on neither
    // the SELECT nor the DROP on them would pass
    conn.execute_batch(include_str!("schema.sql"))
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|e| e.to_string())?;

    if let Some((id, _)) = &keep {
        if has_table(conn, "old_stack_env")? {
            conn.execute(
                &format!(
                    "INSERT INTO stack_env (id, {ENV_COLS})
                     SELECT 1, {ENV_COLS} FROM old_stack_env WHERE stack_id = ?1"
                ),
                [id],
            )
            .map_err(|e| e.to_string())?;
        }
        for (t, cols) in TABLES {
            if has_table(conn, &format!("old_{t}"))? {
                conn.execute(
                    &format!(
                        "INSERT INTO {t} ({cols})
                         SELECT {cols} FROM old_{t} WHERE stack_id = ?1"
                    ),
                    [id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    for t in names.iter().chain(["stack"].iter()) {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS old_{t}"))
            .map_err(|e| e.to_string())?;
    }
    conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;

    Ok(keep.map(|(_, kept)| Migrated { kept, dropped }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Db;

    /// The previous schema, in the exact shape it had at `ba54e1a`+1 — it is the
    /// database sitting on disk for whoever ran yesterday's version.
    const OLD: &str = "
      CREATE TABLE stack (
        id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
        dir TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now')));
      CREATE TABLE stack_env (
        stack_id INTEGER PRIMARY KEY REFERENCES stack(id) ON DELETE CASCADE,
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
        ovpn_pass TEXT NOT NULL DEFAULT '', countries TEXT NOT NULL DEFAULT '');
      CREATE TABLE instance (
        stack_id INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
        key TEXT NOT NULL, ord INTEGER NOT NULL DEFAULT 0,
        service_id TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
        data TEXT NOT NULL DEFAULT '', abs TEXT NOT NULL DEFAULT '',
        hw TEXT NOT NULL DEFAULT 'cpu', tpv TEXT NOT NULL DEFAULT 'std',
        tpt TEXT NOT NULL DEFAULT 'organizr', vpn INTEGER NOT NULL DEFAULT 0,
        solver INTEGER NOT NULL DEFAULT 0, extra TEXT NOT NULL DEFAULT '{}',
        PRIMARY KEY (stack_id, key));
      CREATE TABLE instance_lib (
        stack_id INTEGER NOT NULL, instance_key TEXT NOT NULL,
        ord INTEGER NOT NULL, path TEXT NOT NULL,
        PRIMARY KEY (stack_id, instance_key, ord),
        FOREIGN KEY (stack_id, instance_key)
          REFERENCES instance(stack_id, key) ON DELETE CASCADE);
      CREATE TABLE cfg_app (
        stack_id INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
        arr_key TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1,
        PRIMARY KEY (stack_id, arr_key));
      CREATE TABLE cfg_client (
        stack_id INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
        client_key TEXT NOT NULL, cdh_completed INTEGER, cdh_failed INTEGER,
        PRIMARY KEY (stack_id, client_key));
      CREATE TABLE cfg_client_arr (
        stack_id INTEGER NOT NULL, client_key TEXT NOT NULL, arr_key TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1, category TEXT NOT NULL DEFAULT '',
        PRIMARY KEY (stack_id, client_key, arr_key),
        FOREIGN KEY (stack_id, client_key)
          REFERENCES cfg_client(stack_id, client_key) ON DELETE CASCADE);
      CREATE TABLE cfg_mm (
        stack_id INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
        service_id TEXT NOT NULL, hardlink INTEGER NOT NULL DEFAULT 1,
        rename INTEGER NOT NULL DEFAULT 1, perms INTEGER NOT NULL DEFAULT 0,
        empty INTEGER NOT NULL DEFAULT 0, chmod TEXT NOT NULL DEFAULT '755',
        chown TEXT NOT NULL DEFAULT '', PRIMARY KEY (stack_id, service_id));
      CREATE TABLE cfg_naming (
        stack_id INTEGER NOT NULL, service_id TEXT NOT NULL,
        field TEXT NOT NULL, value TEXT NOT NULL,
        PRIMARY KEY (stack_id, service_id, field),
        FOREIGN KEY (stack_id, service_id)
          REFERENCES cfg_mm(stack_id, service_id) ON DELETE CASCADE);
    ";

    /// An old database with two stacks: number 1 full, number 2 only to be discarded.
    fn old_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(OLD).unwrap();
        conn.execute_batch(
            "INSERT INTO stack (id, slug, name, dir)
               VALUES (1, 'casa', 'Casa', '/srv/casa'), (2, 'sitio', 'Sítio', '/srv/sitio');
             INSERT INTO stack_env (stack_id, tz, puid, tls, cfg)
               VALUES (1, 'America/Sao_Paulo', '1000', 1, '/opt/appdata'),
                      (2, 'UTC', '0', 0, '/tmp');
             INSERT INTO instance (stack_id, key, ord, service_id, title, extra)
               VALUES (1, 'sonarr', 0, 'sonarr', 'Sonarr', '{\"flagNova\":42}'),
                      (1, 'jellyfin', 1, 'jellyfin', 'Jellyfin', '{}'),
                      (2, 'radarr', 0, 'radarr', 'Radarr', '{}');
             INSERT INTO instance_lib (stack_id, instance_key, ord, path)
               VALUES (1, 'jellyfin', 0, '/mnt/a'), (1, 'jellyfin', 1, '/mnt/b');
             INSERT INTO cfg_app (stack_id, arr_key, enabled) VALUES (1, 'sonarr', 1);
             INSERT INTO cfg_client (stack_id, client_key, cdh_completed, cdh_failed)
               VALUES (1, 'sabnzbd', 1, 0);
             INSERT INTO cfg_client_arr (stack_id, client_key, arr_key, enabled, category)
               VALUES (1, 'sabnzbd', 'sonarr', 1, 'tv-sonarr');
             INSERT INTO cfg_mm (stack_id, service_id, chmod) VALUES (1, 'sonarr', '755');
             INSERT INTO cfg_naming (stack_id, service_id, field, value)
               VALUES (1, 'sonarr', 'colon', '\"smart\"');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn the_lowest_id_stack_survives_whole() {
        let conn = old_db();
        let done = run(&conn).unwrap().expect("havia stack para migrar");
        assert_eq!(done.kept, "/srv/casa");
        assert_eq!(done.dropped, vec!["/srv/sitio".to_string()]);

        let db = Db::from_conn(conn);
        let st = db.load().unwrap().expect("o estado migrou");
        // the instances, in order, with `extra` and the Jellyfin folders
        assert_eq!(st["added"][0]["title"], serde_json::json!("Sonarr"));
        assert_eq!(st["added"][0]["flagNova"], serde_json::json!(42));
        assert_eq!(
            st["added"][1]["libs"],
            serde_json::json!(["/mnt/a", "/mnt/b"])
        );
        // the Environment, with the boolean back as a boolean
        assert_eq!(st["defaults"]["tz"], serde_json::json!("America/Sao_Paulo"));
        assert_eq!(st["defaults"]["tls"], serde_json::json!(true));
        // the whole Configuration
        assert_eq!(st["config"]["apps"]["sonarr"], serde_json::json!(true));
        assert_eq!(
            st["config"]["clients"]["sabnzbd"]["cats"]["sonarr"],
            serde_json::json!("tv-sonarr")
        );
        assert_eq!(
            st["config"]["mm"]["sonarr"]["naming"]["colon"],
            serde_json::json!("smart")
        );
    }

    #[test]
    fn the_second_stack_does_not_come_along() {
        let conn = old_db();
        run(&conn).unwrap();
        let db = Db::from_conn(conn);
        let st = db.load().unwrap().unwrap();
        let titles: Vec<&str> = st["added"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, vec!["Sonarr", "Jellyfin"]);
        assert_eq!(st["defaults"]["tz"], serde_json::json!("America/Sao_Paulo"));
    }

    #[test]
    fn no_table_from_the_old_model_is_left_behind() {
        let conn = old_db();
        run(&conn).unwrap();
        assert!(!has_table(&conn, "stack").unwrap());
        for t in ["stack_env", "instance", "cfg_mm"] {
            assert!(!has_table(&conn, &format!("old_{t}")).unwrap());
            // and the new table is in place
            assert!(has_table(&conn, t).unwrap());
        }
    }

    #[test]
    fn a_new_db_passes_straight_through() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        assert!(run(&conn).unwrap().is_none());
    }

    #[test]
    fn migrating_again_does_nothing() {
        let conn = old_db();
        run(&conn).unwrap();
        // second pass: it is already the new model, and the state is still where it was
        assert!(run(&conn).unwrap().is_none());
        let db = Db::from_conn(conn);
        assert_eq!(db.load().unwrap().unwrap()["added"][0]["title"], "Sonarr");
    }

    #[test]
    fn an_old_db_with_no_stack_only_swaps_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(OLD).unwrap();
        assert!(run(&conn).unwrap().is_none());
        assert!(!has_table(&conn, "stack").unwrap());
        assert!(Db::from_conn(conn).load().unwrap().is_none());
    }
}

/* Migração do banco de várias stacks para o de uma só.

   O modelo anterior tinha uma tabela `stack` na raiz e um `stack_id` em todas
   as outras. Hoje a stack é uma — a da pasta do `--dir` —, e nenhuma tabela
   leva id. Um banco escrito pelo modelo antigo não serve como está: as colunas
   velhas são `NOT NULL` e o `CREATE TABLE IF NOT EXISTS` do `schema.sql` não
   toca em tabela que já existe.

   Então, antes do `schema.sql`: as tabelas antigas ganham o prefixo `old_`, o
   esquema novo nasce ao lado, uma stack é copiada para dentro dele e o `old_`
   vai embora. Copiar mais de uma não faria sentido — não há mais onde guardar a
   segunda —, então vale a de menor id, que é a que a página abria por padrão.
   As outras são perdidas de propósito, e o `dir` de cada uma é anunciado na
   saída para quem quiser reaproveitar os arquivos já gravados. */

use rusqlite::{Connection, OptionalExtension};

/// As colunas de cada tabela, sem o `stack_id`. A ordem é a mesma nos dois
/// lados do `INSERT ... SELECT`.
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

/// O que a migração encontrou, para o `main` contar a quem está olhando.
pub struct Migrated {
    /// a pasta da stack que ficou
    pub kept: String,
    /// as pastas das stacks que não couberam no modelo novo
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

/// Roda antes do `schema.sql`. Devolve `None` num banco que já é do modelo
/// novo — ou vazio, que é o mesmo caminho da primeira vez.
pub fn run(conn: &Connection) -> Result<Option<Migrated>, String> {
    if !has_table(conn, "stack")? {
        return Ok(None);
    }

    // a de menor id é a que a página abria por padrão; as outras não têm mais
    // onde caber, e viram só um aviso na saída
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

    // o rename não pode reescrever as chaves estrangeiras das outras tabelas:
    // é o esquema antigo que está sendo posto de lado inteiro
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

    // o `schema.sql` religa o `foreign_keys`; as tabelas antigas ainda apontam
    // para os nomes que agora são do esquema novo, e com a checagem ligada nem
    // o SELECT delas nem o DROP passariam
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

    /// O esquema anterior, na forma exata em que ficou no `ba54e1a`+1 — é o
    /// banco que existe no disco de quem rodou a versão de ontem.
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

    /// Um banco antigo com duas stacks: a 1 cheia, a 2 só para ser descartada.
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
    fn a_stack_de_menor_id_atravessa_inteira() {
        let conn = old_db();
        let done = run(&conn).unwrap().expect("havia stack para migrar");
        assert_eq!(done.kept, "/srv/casa");
        assert_eq!(done.dropped, vec!["/srv/sitio".to_string()]);

        let db = Db::from_conn(conn);
        let st = db.load().unwrap().expect("o estado migrou");
        // as instâncias, na ordem, com o `extra` e as pastas do Jellyfin
        assert_eq!(st["added"][0]["title"], serde_json::json!("Sonarr"));
        assert_eq!(st["added"][0]["flagNova"], serde_json::json!(42));
        assert_eq!(
            st["added"][1]["libs"],
            serde_json::json!(["/mnt/a", "/mnt/b"])
        );
        // o Ambiente, com o booleano de volta como booleano
        assert_eq!(st["defaults"]["tz"], serde_json::json!("America/Sao_Paulo"));
        assert_eq!(st["defaults"]["tls"], serde_json::json!(true));
        // a Configuração inteira
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
    fn a_segunda_stack_nao_vem_junto() {
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
    fn nao_sobra_tabela_do_modelo_antigo() {
        let conn = old_db();
        run(&conn).unwrap();
        assert!(!has_table(&conn, "stack").unwrap());
        for t in ["stack_env", "instance", "cfg_mm"] {
            assert!(!has_table(&conn, &format!("old_{t}")).unwrap());
            // e a tabela nova ficou no lugar
            assert!(has_table(&conn, t).unwrap());
        }
    }

    #[test]
    fn banco_novo_passa_direto() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        assert!(run(&conn).unwrap().is_none());
    }

    #[test]
    fn migrar_de_novo_nao_faz_nada() {
        let conn = old_db();
        run(&conn).unwrap();
        // segunda passada: já é o modelo novo, e o estado continua onde estava
        assert!(run(&conn).unwrap().is_none());
        let db = Db::from_conn(conn);
        assert_eq!(db.load().unwrap().unwrap()["added"][0]["title"], "Sonarr");
    }

    #[test]
    fn banco_antigo_sem_stack_nenhuma_so_troca_de_esquema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(OLD).unwrap();
        assert!(run(&conn).unwrap().is_none());
        assert!(!has_table(&conn, "stack").unwrap());
        assert!(Db::from_conn(conn).load().unwrap().is_none());
    }
}

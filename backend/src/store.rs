/* Estado da página, em SQLite.

   Uma tabela por coisa que a página tem: `instance`, uma linha por serviço
   adicionado, e `setting`, chave/valor para o Ambiente e a Configuração. O
   `stack.db` fica junto dos arquivos gerados, para a stack inteira caber numa
   pasta só.

   Ao contrário do JSON que havia antes, aqui o servidor conhece o formato do
   `added` — é o preço de ter tabela em vez de um blob. Os campos que a página
   inventar depois caem em `extra`, que é o resto do objeto em JSON: assim uma
   flag nova no `SERVICES` não exige migração para voltar inteira à página.

   Cada adicionar, editar ou excluir mexe numa linha só. O `ord` guarda a
   ordem da lista, que é a ordem em que os serviços aparecem no compose. */

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Map, Value};

const NAME: &str = "stack.db";

/// Os campos do `added` que viram coluna. O resto vai para `extra`.
const COLUMNS: [&str; 7] = ["id", "title", "data", "abs", "hw", "vpn", "solver"];

pub struct Db(Arc<Mutex<Connection>>);

pub fn path(dir: &Path) -> PathBuf {
    dir.join(NAME)
}

/// O que a página manda ao adicionar ou editar um serviço.
#[derive(Deserialize)]
pub struct InstanceIn {
    /// chave da instância: o `container_name`, que é o `cname()` da página
    pub key: String,
    /// chave anterior, quando editar renomeou a instância
    #[serde(default)]
    pub old: Option<String>,
    /// posição na lista
    #[serde(default)]
    pub ord: i64,
    /// o objeto inteiro, como a página o guarda no `added`
    pub data: Value,
}

impl Db {
    /// Abre o banco e cria o que faltar. Rodar de novo num banco pronto não
    /// muda nada — é o mesmo caminho da primeira vez e das seguintes.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let conn = Connection::open(path(dir)).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS instance (
               key    TEXT PRIMARY KEY,
               ord    INTEGER NOT NULL DEFAULT 0,
               id     TEXT NOT NULL,
               title  TEXT NOT NULL DEFAULT '',
               data   TEXT NOT NULL DEFAULT '',
               abs    TEXT NOT NULL DEFAULT '',
               hw     TEXT NOT NULL DEFAULT 'cpu',
               vpn    INTEGER NOT NULL DEFAULT 0,
               solver INTEGER NOT NULL DEFAULT 0,
               extra  TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE IF NOT EXISTS setting (
               name  TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.0.lock().map_err(|_| "banco travado".to_string())
    }

    pub fn has_stack(&self) -> bool {
        self.lock()
            .and_then(|c| {
                c.query_row("SELECT count(*) FROM instance", [], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())
            })
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    /// Cria ou atualiza uma instância. Quando o editar mudou o título, a chave
    /// muda junto — por isso o `old`: é um renomear, não uma linha nova.
    pub fn put_instance(&self, inc: &InstanceIn) -> Result<(), String> {
        let obj = inc
            .data
            .as_object()
            .ok_or_else(|| "a instância não veio como objeto".to_string())?;
        let text = |k: &str| obj.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let flag = |k: &str| obj.get(k).and_then(|v| v.as_bool()).unwrap_or(false) as i64;

        // o que não virou coluna volta inteiro para a página no GET
        let extra: Map<String, Value> = obj
            .iter()
            .filter(|(k, _)| !COLUMNS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let extra = serde_json::to_string(&extra).map_err(|e| e.to_string())?;

        let conn = self.lock()?;
        if let Some(old) = inc.old.as_deref() {
            if old != inc.key {
                conn.execute("DELETE FROM instance WHERE key = ?1", params![old])
                    .map_err(|e| e.to_string())?;
            }
        }
        conn.execute(
            "INSERT INTO instance (key, ord, id, title, data, abs, hw, vpn, solver, extra)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(key) DO UPDATE SET
               ord=excluded.ord, id=excluded.id, title=excluded.title,
               data=excluded.data, abs=excluded.abs, hw=excluded.hw,
               vpn=excluded.vpn, solver=excluded.solver, extra=excluded.extra",
            params![
                inc.key,
                inc.ord,
                text("id"),
                text("title"),
                text("data"),
                text("abs"),
                if text("hw").is_empty() { "cpu".into() } else { text("hw") },
                flag("vpn"),
                flag("solver"),
                extra,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_instance(&self, key: &str) -> Result<(), String> {
        self.lock()?
            .execute("DELETE FROM instance WHERE key = ?1", params![key])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Alinha o banco com a lista que a página tem agora: acerta a ordem e
    /// apaga o que saiu. É a rede de segurança para as mudanças que não passam
    /// pelo modal — o gluetun que entra sozinho, o Heimdall que é reposto.
    pub fn reconcile(&self, keys: &[String]) -> Result<(), String> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        {
            let list = serde_json::to_string(keys).map_err(|e| e.to_string())?;
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
                up.execute(params![k, i as i64]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn put_setting(&self, name: &str, value: &Value) -> Result<(), String> {
        let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.lock()?
            .execute(
                "INSERT INTO setting (name, value) VALUES (?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET value = excluded.value",
                params![name, text],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn setting(&self, name: &str) -> Result<Option<Value>, String> {
        let conn = self.lock()?;
        let text: Option<String> = conn
            .query_row("SELECT value FROM setting WHERE name = ?1", params![name], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        match text {
            None => Ok(None),
            Some(t) => serde_json::from_str(&t).map(Some).map_err(|e| e.to_string()),
        }
    }

    /// Monta de volta o estado no formato que a página espera receber.
    pub fn load(&self) -> Result<Option<Value>, String> {
        let added = self.instances()?;
        let defaults = self.setting("defaults")?;
        let config = self.setting("config")?;
        if added.is_empty() && defaults.is_none() && config.is_none() {
            return Ok(None);
        }
        Ok(Some(json!({
            "added": added,
            "defaults": defaults.unwrap_or(Value::Null),
            "config": config.unwrap_or(Value::Null),
        })))
    }

    fn instances(&self) -> Result<Vec<Value>, String> {
        let conn = self.lock()?;
        let mut st = conn
            .prepare(
                "SELECT id, title, data, abs, hw, vpn, solver, extra
                   FROM instance ORDER BY ord, key",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                let extra: String = r.get(7)?;
                Ok(json!({
                    "id":     r.get::<_, String>(0)?,
                    "title":  r.get::<_, String>(1)?,
                    "data":   r.get::<_, String>(2)?,
                    "abs":    r.get::<_, String>(3)?,
                    "hw":     r.get::<_, String>(4)?,
                    "vpn":    r.get::<_, i64>(5)? != 0,
                    "solver": r.get::<_, i64>(6)? != 0,
                    "extra":  extra,
                }))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            let mut v = row.map_err(|e| e.to_string())?;
            // o `extra` volta espalhado no objeto, como a página o mandou
            let extra = v
                .as_object_mut()
                .and_then(|o| o.remove("extra"))
                .and_then(|e| e.as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()));
            if let (Some(obj), Some(Value::Object(rest))) = (v.as_object_mut(), extra) {
                for (k, val) in rest {
                    obj.insert(k, val);
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

    fn db() -> Db {
        let conn = Connection::open_in_memory().unwrap();
        let d = Db(Arc::new(Mutex::new(conn)));
        d.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE instance (key TEXT PRIMARY KEY, ord INTEGER NOT NULL DEFAULT 0,
                   id TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', data TEXT NOT NULL DEFAULT '',
                   abs TEXT NOT NULL DEFAULT '', hw TEXT NOT NULL DEFAULT 'cpu',
                   vpn INTEGER NOT NULL DEFAULT 0, solver INTEGER NOT NULL DEFAULT 0,
                   extra TEXT NOT NULL DEFAULT '{}');
                 CREATE TABLE setting (name TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .unwrap();
        d
    }

    fn inc(key: &str, old: Option<&str>, ord: i64, data: Value) -> InstanceIn {
        InstanceIn { key: key.into(), old: old.map(String::from), ord, data }
    }

    #[test]
    fn editar_renomeando_move_a_linha_em_vez_de_duplicar() {
        let d = db();
        d.put_instance(&inc("sonarr", None, 0, json!({"id":"sonarr","title":"Sonarr","libs":[]})))
            .unwrap();
        d.put_instance(&inc("series", Some("sonarr"), 0, json!({"id":"sonarr","title":"Séries","libs":[]})))
            .unwrap();
        let all = d.instances().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["title"], json!("Séries"));
    }

    #[test]
    fn o_que_nao_e_coluna_volta_inteiro() {
        let d = db();
        d.put_instance(&inc("jellyfin", None, 0,
            json!({"id":"jellyfin","title":"Jellyfin","libs":["/mnt/x"],"vpn":false})))
            .unwrap();
        let all = d.instances().unwrap();
        assert_eq!(all[0]["libs"], json!(["/mnt/x"]));
        assert_eq!(all[0]["id"], json!("jellyfin"));
    }

    #[test]
    fn reconcile_apaga_o_que_saiu_e_acerta_a_ordem() {
        let d = db();
        d.put_instance(&inc("a", None, 0, json!({"id":"sonarr"}))).unwrap();
        d.put_instance(&inc("b", None, 1, json!({"id":"radarr"}))).unwrap();
        d.reconcile(&["b".to_string()]).unwrap();
        let all = d.instances().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["id"], json!("radarr"));
    }
}

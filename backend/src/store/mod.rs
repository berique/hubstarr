/* Estado da página, em SQLite.

   Uma tabela por coisa que a página tem — as instâncias, o Ambiente, a
   Configuração. A stack é uma só, a da pasta do `--dir`, então nenhuma tabela
   leva id de stack. O banco fica num lugar só (`~/.hubstarr/hubstarr.db`); o
   que fica na pasta da stack são os arquivos gerados, e só eles.

   O servidor conhece o formato do estado da página — é o preço de ter tabela em
   vez de um blob. Os campos que a página inventar depois caem em `extra`, que é
   o resto do objeto em JSON: assim uma flag nova no `SERVICES` não exige
   migração para voltar inteira à página.

   `load()` remonta `{added, defaults, config}` exatamente na forma em que a
   página mandou. É esse o critério do modelo: ida e volta sem perda. */

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
    /// Abre o banco e cria o que faltar. Rodar de novo num banco pronto não
    /// muda nada — é o mesmo caminho da primeira vez e das seguintes. Um banco
    /// do modelo de várias stacks passa antes pela migração, que já monta o
    /// esquema novo; devolve junto o que ela encontrou, para o `main` contar.
    pub fn open(path: &Path) -> Result<(Self, Option<migrate::Migrated>), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let done = migrate::run(&conn)?;
        conn.execute_batch(include_str!("schema.sql"))
            .map_err(|e| e.to_string())?;
        // o schema não acrescenta coluna a tabela que já existe, e o Ambiente é
        // a tabela que cresce uma coluna por chave nova — ver `ensure_env_cols`
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

    /// O estado inteiro da stack, na forma que a página espera receber.
    pub fn load(&self) -> Result<Option<Value>, String> {
        let added = self.instances()?;
        let defaults = self.env()?;
        let config = self.config()?;
        // banco recém-criado: nada a restaurar, e a página fica com os padrões
        // dela em vez de receber de volta um estado vazio
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

/* ---------- utilidades de JSON ---------- */

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
    fn banco_vazio_nao_devolve_estado() {
        let db = Db::memory().unwrap();
        assert!(db.load().unwrap().is_none());
    }

    #[test]
    fn com_uma_instancia_o_estado_volta_inteiro() {
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

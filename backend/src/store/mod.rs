/* Estado da página, em SQLite.

   Uma tabela por coisa que a página tem — as instâncias, o Ambiente, a
   Configuração —, e uma tabela `stack` na raiz, porque um servidor guarda
   várias. O banco fica num lugar só (`~/.hubstarr/hubstarr.db`); o que fica na
   pasta de cada stack são os arquivos gerados, e só eles.

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
pub mod instance;
pub mod stack;

pub use instance::InstanceIn;

pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    /// Abre o banco e cria o que faltar. Rodar de novo num banco pronto não
    /// muda nada — é o mesmo caminho da primeira vez e das seguintes.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(include_str!("schema.sql"))
            .map_err(|e| e.to_string())?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    #[cfg(test)]
    pub fn memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        conn.execute_batch(include_str!("schema.sql"))
            .map_err(|e| e.to_string())?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.0.lock().map_err(|_| "banco travado".to_string())
    }

    /// O estado inteiro de uma stack, na forma que a página espera receber.
    pub fn load(&self, stack_id: i64) -> Result<Option<Value>, String> {
        if !self.stack_exists(stack_id)? {
            return Ok(None);
        }
        let added = self.instances(stack_id)?;
        let defaults = self.env(stack_id)?;
        let config = self.config(stack_id)?;
        // stack recém-criada: nada a restaurar, e a página fica com os padrões
        // dela em vez de receber de volta um estado vazio
        let empty = config["apps"].as_object().map_or(true, |m| m.is_empty())
            && config["clients"].as_object().map_or(true, |m| m.is_empty())
            && config["mm"].as_object().map_or(true, |m| m.is_empty());
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

/// Um slug no mesmo espírito do `slug()` da página: minúsculas, sem acento,
/// o que não for alfanumérico vira `-`.
pub fn slug(t: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in t.trim().to_lowercase().chars() {
        let c = deaccent(c);
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn deaccent(c: char) -> char {
    match c {
        'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_segue_o_da_pagina() {
        assert_eq!(slug("Séries 4K"), "series-4k");
        assert_eq!(slug("  --Ação!!  "), "acao");
        assert_eq!(slug("Sonarr"), "sonarr");
    }
}

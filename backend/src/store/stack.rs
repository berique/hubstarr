/* As stacks e o Ambiente de cada uma.

   O `DEFAULTS` da página vira uma linha em `stack_env`, uma coluna por chave.
   `ENV_COLS` é o de-para entre as duas grafias — camelCase na página,
   snake_case na tabela — e é o único lugar em que ele aparece: acrescentar uma
   chave ao Ambiente é acrescentar uma coluna no `schema.sql` e uma linha aqui. */

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{slug, Db};

/// (chave no `DEFAULTS`, coluna). O `tls` fica de fora: é o único booleano.
const ENV_COLS: [(&str, &str); 23] = [
    ("restart", "restart"),
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

#[derive(Serialize)]
pub struct StackRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub dir: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct NewStack {
    pub name: String,
}

impl Db {
    pub fn stacks(&self) -> Result<Vec<StackRow>, String> {
        let conn = self.lock()?;
        let mut st = conn
            .prepare("SELECT id, slug, name, dir, updated_at FROM stack ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                Ok(StackRow {
                    id: r.get(0)?,
                    slug: r.get(1)?,
                    name: r.get(2)?,
                    dir: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    pub fn stack_exists(&self, id: i64) -> Result<bool, String> {
        let conn = self.lock()?;
        conn.query_row("SELECT 1 FROM stack WHERE id = ?1", params![id], |_| Ok(()))
            .optional()
            .map(|o| o.is_some())
            .map_err(|e| e.to_string())
    }

    pub fn stack_dir(&self, id: i64) -> Result<String, String> {
        let conn = self.lock()?;
        conn.query_row("SELECT dir FROM stack WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("stack {id} não existe"))
    }

    /// Cria a stack. O slug sai do nome e é único: um sufixo numérico resolve o
    /// choque, porque é ele que nomeia a pasta dos arquivos gerados.
    pub fn create_stack(&self, name: &str, base: &std::path::Path) -> Result<StackRow, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a stack precisa de um nome".into());
        }
        let conn = self.lock()?;
        let root = slug(name);
        let root = if root.is_empty() { "stack".to_string() } else { root };
        let mut slug_ = root.clone();
        for n in 2.. {
            let taken: Option<i64> = conn
                .query_row("SELECT id FROM stack WHERE slug = ?1", params![slug_], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?;
            if taken.is_none() {
                break;
            }
            slug_ = format!("{root}-{n}");
        }
        let dir = base.join(&slug_).display().to_string();
        conn.execute(
            "INSERT INTO stack (slug, name, dir) VALUES (?1, ?2, ?3)",
            params![slug_, name, dir],
        )
        .map_err(|e| e.to_string())?;
        // o `stack_env` nasce no primeiro `put_env`: enquanto a stack não teve
        // o Ambiente gravado, o `load()` devolve `null` ali, e a página fica
        // com os próprios padrões em vez de recebê-los em branco de volta
        let id = conn.last_insert_rowid();
        Ok(StackRow {
            id,
            slug: slug_,
            name: name.to_string(),
            dir,
            updated_at: String::new(),
        })
    }

    /// Apaga a stack inteira — as instâncias e a Configuração vão junto, pelo
    /// `ON DELETE CASCADE`. Os arquivos gerados ficam onde estão: apagar do
    /// disco é decisão de quem os escreveu, não do banco.
    pub fn delete_stack(&self, id: i64) -> Result<(), String> {
        self.lock()?
            .execute("DELETE FROM stack WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub(crate) fn touch(&self, id: i64) -> Result<(), String> {
        self.lock()?
            .execute(
                "UPDATE stack SET updated_at = datetime('now') WHERE id = ?1",
                params![id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Grava o Ambiente. O que não vier no JSON fica como está.
    pub fn put_env(&self, stack_id: i64, v: &Value) -> Result<(), String> {
        let o = v
            .as_object()
            .ok_or_else(|| "o Ambiente não veio como objeto".to_string())?;
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR IGNORE INTO stack_env (stack_id) VALUES (?1)",
            params![stack_id],
        )
        .map_err(|e| e.to_string())?;

        for (key, col) in ENV_COLS {
            if let Some(val) = o.get(key) {
                let s = match val {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                conn.execute(
                    &format!("UPDATE stack_env SET {col} = ?2 WHERE stack_id = ?1"),
                    params![stack_id, s],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        if let Some(tls) = o.get("tls").and_then(|v| v.as_bool()) {
            conn.execute(
                "UPDATE stack_env SET tls = ?2 WHERE stack_id = ?1",
                params![stack_id, tls as i64],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// O `BASE_CONFIG` do Ambiente — a raiz das configurações que os
    /// containers montam. `None` enquanto a stack não teve o Ambiente gravado.
    pub fn config_base(&self, stack_id: i64) -> Result<Option<String>, String> {
        let conn = self.lock()?;
        let v: Option<String> = conn
            .query_row(
                "SELECT cfg FROM stack_env WHERE stack_id = ?1",
                params![stack_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(v.filter(|s| !s.trim().is_empty()))
    }

    pub(crate) fn env(&self, stack_id: i64) -> Result<Value, String> {
        let cols: Vec<&str> = ENV_COLS.iter().map(|(_, c)| *c).collect();
        let sql = format!(
            "SELECT {}, tls FROM stack_env WHERE stack_id = ?1",
            cols.join(", ")
        );
        let conn = self.lock()?;
        let row: Option<Map<String, Value>> = conn
            .query_row(&sql, params![stack_id], |r| {
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
    fn ambiente_volta_como_foi() {
        let db = Db::memory().unwrap();
        let s = db.create_stack("Casa", std::path::Path::new("/srv")).unwrap();
        db.put_env(
            s.id,
            &json!({"tz":"America/Sao_Paulo","puid":"1000","tls":true,"domain":"casa.example"}),
        )
        .unwrap();
        let back = db.env(s.id).unwrap();
        assert_eq!(back["tz"], json!("America/Sao_Paulo"));
        assert_eq!(back["tls"], json!(true));
        assert_eq!(back["domain"], json!("casa.example"));
        assert_eq!(back["cert"], json!(""));
    }

    #[test]
    fn slug_repetido_ganha_sufixo_e_a_pasta_acompanha() {
        let db = Db::memory().unwrap();
        let a = db.create_stack("Casa", std::path::Path::new("/srv")).unwrap();
        let b = db.create_stack("casa", std::path::Path::new("/srv")).unwrap();
        assert_eq!(a.slug, "casa");
        assert_eq!(b.slug, "casa-2");
        assert_eq!(b.dir, "/srv/casa-2");
    }

    #[test]
    fn stack_nova_nao_tem_ambiente_para_devolver() {
        let db = Db::memory().unwrap();
        let s = db.create_stack("Casa", std::path::Path::new("/srv")).unwrap();
        assert_eq!(db.env(s.id).unwrap(), json!(null));
    }

    #[test]
    fn apagar_a_stack_leva_o_ambiente_junto() {
        let db = Db::memory().unwrap();
        let s = db.create_stack("Casa", std::path::Path::new("/srv")).unwrap();
        db.delete_stack(s.id).unwrap();
        assert!(!db.stack_exists(s.id).unwrap());
        assert_eq!(db.env(s.id).unwrap(), json!(null));
    }
}

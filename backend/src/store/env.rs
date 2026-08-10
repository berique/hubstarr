/* O Ambiente da stack.

   O `DEFAULTS` da página vira a linha única de `stack_env`, uma coluna por
   chave. `ENV_COLS` é o de-para entre as duas grafias — camelCase na página,
   snake_case na tabela — e é o único lugar em que ele aparece: acrescentar uma
   chave ao Ambiente é acrescentar uma coluna no `schema.sql` e uma linha aqui. */

use rusqlite::{params, OptionalExtension};
use serde_json::{Map, Value};

use super::Db;

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

impl Db {
    /// Grava o Ambiente. O que não vier no JSON fica como está.
    pub fn put_env(&self, v: &Value) -> Result<(), String> {
        let o = v
            .as_object()
            .ok_or_else(|| "o Ambiente não veio como objeto".to_string())?;
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
        Ok(())
    }

    /// O `BASE_CONFIG` do Ambiente — a raiz das configurações que os
    /// containers montam. `None` enquanto o Ambiente não foi gravado.
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
    fn ambiente_volta_como_foi() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"tz":"America/Sao_Paulo","puid":"1000","tls":true,"domain":"casa.example"}))
            .unwrap();
        let back = db.env().unwrap();
        assert_eq!(back["tz"], json!("America/Sao_Paulo"));
        assert_eq!(back["tls"], json!(true));
        assert_eq!(back["domain"], json!("casa.example"));
        assert_eq!(back["cert"], json!(""));
    }

    #[test]
    fn gravar_de_novo_mantem_a_linha_unica() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"tz":"UTC"})).unwrap();
        db.put_env(&json!({"puid":"1000"})).unwrap();
        let back = db.env().unwrap();
        // o segundo put não criou outra linha nem apagou o que o primeiro pôs
        assert_eq!(back["tz"], json!("UTC"));
        assert_eq!(back["puid"], json!("1000"));
    }

    #[test]
    fn banco_novo_nao_tem_ambiente_para_devolver() {
        let db = Db::memory().unwrap();
        assert_eq!(db.env().unwrap(), json!(null));
    }
}

/* O Ambiente da stack.

   O `DEFAULTS` da página vira a linha única de `stack_env`, uma coluna por
   chave. `ENV_COLS` é o de-para entre as duas grafias — camelCase na página,
   snake_case na tabela — e é o único lugar em que ele aparece: acrescentar uma
   chave ao Ambiente é acrescentar uma coluna no `schema.sql` e uma linha aqui. */

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};

use super::Db;

/// (chave no `DEFAULTS`, coluna). O `tls` fica de fora: é o único booleano.
const ENV_COLS: [(&str, &str); 25] = [
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
    // o administrador do Jellyfin, que o Subir usa no assistente dele
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

/* As colunas do `ENV_COLS` que faltam num banco de uma versão anterior.

   Acrescentar uma chave ao Ambiente é acrescentar uma coluna ao `schema.sql` e
   uma linha ali em cima — só que o `CREATE TABLE IF NOT EXISTS` não mexe em
   tabela que já existe, então o banco de quem já tinha stack fica sem ela. E
   uma coluna faltando não é um detalhe: o `SELECT` do `env()` nomeia todas, e
   sem uma delas **toda** leitura do Ambiente falha — a página abre com a stack
   vazia e, no primeiro save, o `reconcile()` apaga as instâncias que ela não
   viu. Foi exatamente isso que o `jf_user`/`jf_pass` fez.

   Por isso a lista manda em quem já existe também: coluna que falta entra como
   TEXT vazio, que é o padrão do `schema.sql` e o que a página entende como "não
   preenchido". O `tls` fica de fora porque não está no `ENV_COLS` — é o único
   booleano, e nasceu com a tabela. */
pub(crate) fn ensure_env_cols(conn: &Connection) -> Result<(), String> {
    let mut tem = std::collections::HashSet::new();
    {
        let mut q = conn
            .prepare("SELECT name FROM pragma_table_info('stack_env')")
            .map_err(|e| e.to_string())?;
        let linhas = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for l in linhas {
            tem.insert(l.map_err(|e| e.to_string())?);
        }
    }
    // tabela que ainda não existe é assunto do schema.sql, não daqui
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
        /* Os nomes dos campos, nunca os valores: entre eles estão a chave da
           stack e as senhas do qBittorrent, do Jellyfin e da VPN. */
        crate::registro::detalhe(|| {
            let mut campos: Vec<&str> = ENV_COLS
                .iter()
                .filter(|(k, _)| o.contains_key(*k))
                .map(|(k, _)| *k)
                .collect();
            if o.contains_key("tls") {
                campos.push("tls");
            }
            format!("banco: Ambiente, {} campo(s): {}", campos.len(), campos.join(", "))
        });
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

    /// As credenciais do Jellyfin não vão para arquivo nenhum, então o banco é
    /// o único lugar em que elas sobrevivem ao reload — se caírem aqui, o Subir
    /// deixa de passar pelo assistente e ninguém entende por quê.
    #[test]
    fn as_credenciais_do_jellyfin_atravessam() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"jfUser": "henrique", "jfPass": "segredo"}))
            .unwrap();
        let back = db.env().unwrap();
        assert_eq!(back["jfUser"], json!("henrique"));
        assert_eq!(back["jfPass"], json!("segredo"));
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

    /// O banco de uma versão anterior não tem as colunas que o Ambiente ganhou
    /// depois. Sem o `ensure_env_cols`, o `SELECT` que as nomeia falha inteiro
    /// — e a página, que trata isso como "não há nada guardado", abre vazia e
    /// manda apagar o resto.
    #[test]
    fn coluna_que_faltava_no_banco_antigo_entra_na_abertura() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // o stack_env como era antes do jf_user/jf_pass
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
        // o que já estava guardado atravessa, e o que faltava vem vazio
        assert_eq!(back["tz"], json!("America/Sao_Paulo"));
        assert_eq!(back["jfUser"], json!(""));
        // e a coluna nova aceita escrita como qualquer outra
        db.put_env(&json!({"jfUser": "henrique"})).unwrap();
        assert_eq!(db.env().unwrap()["jfUser"], json!("henrique"));
    }

    /// Rodar de novo num banco já em dia não muda nada — é o mesmo caminho da
    /// primeira abertura e das seguintes.
    #[test]
    fn acrescentar_coluna_de_novo_nao_faz_nada() {
        let db = Db::memory().unwrap();
        db.put_env(&json!({"jfPass": "segredo"})).unwrap();
        let conn = db.lock().unwrap();
        ensure_env_cols(&conn).unwrap();
        ensure_env_cols(&conn).unwrap();
        drop(conn);
        assert_eq!(db.env().unwrap()["jfPass"], json!("segredo"));
    }

    #[test]
    fn banco_novo_nao_tem_ambiente_para_devolver() {
        let db = Db::memory().unwrap();
        assert_eq!(db.env().unwrap(), json!(null));
    }
}

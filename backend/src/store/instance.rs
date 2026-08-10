/* As instâncias: uma linha por serviço adicionado.

   A chave é o `cname()` da página — o `container_name`, que é também a pasta de
   config e o nome pelo qual a Configuração se refere ao serviço. Editar o
   título muda a chave junto: por isso o `old`, que faz do editar um renomear e
   não uma linha nova.

   Cada adicionar, editar ou excluir mexe numa linha só; o `reconcile()` é a
   rede de segurança para o que não passa pelo modal — o gluetun que entra
   sozinho, o Heimdall que é reposto, o "Limpar" que esvazia tudo. */

use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{flag, text, Db};

/// Os campos do `added` que viram coluna ou tabela própria. O resto vai para
/// `extra` e volta espalhado no objeto, como a página o mandou.
const COLUMNS: [&str; 10] = [
    "id", "title", "data", "abs", "hw", "tpv", "tpt", "vpn", "solver", "libs",
];

/// O que a página manda ao adicionar ou editar um serviço.
#[derive(Deserialize)]
pub struct InstanceIn {
    /// chave da instância: o `container_name`, que é o `cname()` da página
    pub key: String,
    /// chave anterior, quando editar renomeou a instância
    #[serde(default)]
    pub old: Option<String>,
    /// posição na lista, que é a ordem no compose
    #[serde(default)]
    pub ord: i64,
    /// o objeto inteiro, como a página o guarda no `added`
    pub data: Value,
}

impl Db {
    pub fn put_instance(&self, stack_id: i64, inc: &InstanceIn) -> Result<(), String> {
        let o = inc
            .data
            .as_object()
            .ok_or_else(|| "a instância não veio como objeto".to_string())?;
        if inc.key.trim().is_empty() {
            return Err("a instância veio sem chave".into());
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
                tx.execute(
                    "DELETE FROM instance WHERE stack_id = ?1 AND key = ?2",
                    params![stack_id, old],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        tx.execute(
            "INSERT INTO instance
               (stack_id, key, ord, service_id, title, data, abs, hw, tpv, tpt, vpn, solver, extra)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(stack_id, key) DO UPDATE SET
               ord=excluded.ord, service_id=excluded.service_id, title=excluded.title,
               data=excluded.data, abs=excluded.abs, hw=excluded.hw, tpv=excluded.tpv,
               tpt=excluded.tpt, vpn=excluded.vpn, solver=excluded.solver,
               extra=excluded.extra",
            params![
                stack_id,
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
            "DELETE FROM instance_lib WHERE stack_id = ?1 AND instance_key = ?2",
            params![stack_id, inc.key],
        )
        .map_err(|e| e.to_string())?;
        for (i, path) in libs.iter().enumerate() {
            tx.execute(
                "INSERT INTO instance_lib (stack_id, instance_key, ord, path)
                 VALUES (?1,?2,?3,?4)",
                params![stack_id, inc.key, i as i64, path],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn delete_instance(&self, stack_id: i64, key: &str) -> Result<(), String> {
        self.lock()?
            .execute(
                "DELETE FROM instance WHERE stack_id = ?1 AND key = ?2",
                params![stack_id, key],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Alinha o banco com a lista que a página tem agora: acerta a ordem e
    /// apaga o que saiu.
    pub fn reconcile(&self, stack_id: i64, keys: &[String]) -> Result<(), String> {
        let list = serde_json::to_string(keys).map_err(|e| e.to_string())?;
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        {
            tx.execute(
                "DELETE FROM instance
                  WHERE stack_id = ?1
                    AND key NOT IN (SELECT value FROM json_each(?2))",
                params![stack_id, list],
            )
            .map_err(|e| e.to_string())?;
            let mut up = tx
                .prepare("UPDATE instance SET ord = ?3 WHERE stack_id = ?1 AND key = ?2")
                .map_err(|e| e.to_string())?;
            for (i, k) in keys.iter().enumerate() {
                up.execute(params![stack_id, k, i as i64])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// Remonta o `added` da página: na ordem da lista, com as `libs` de volta
    /// como array e o `extra` espalhado no objeto.
    pub(crate) fn instances(&self, stack_id: i64) -> Result<Vec<Value>, String> {
        let conn = self.lock()?;
        let mut st = conn
            .prepare(
                "SELECT key, service_id, title, data, abs, hw, tpv, tpt, vpn, solver, extra
                   FROM instance WHERE stack_id = ?1 ORDER BY ord, key",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, Value, String)> = st
            .query_map(params![stack_id], |r| {
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
                "SELECT path FROM instance_lib
                  WHERE stack_id = ?1 AND instance_key = ?2 ORDER BY ord",
            )
            .map_err(|e| e.to_string())?;

        let mut out = Vec::with_capacity(rows.len());
        for (key, mut v, extra) in rows {
            let paths: Vec<Value> = libs
                .query_map(params![stack_id, key], |r| r.get::<_, String>(0))
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
    use std::path::Path;

    fn db() -> (Db, i64) {
        let db = Db::memory().unwrap();
        let s = db.create_stack("Casa", Path::new("/srv")).unwrap();
        (db, s.id)
    }

    fn inc(key: &str, old: Option<&str>, ord: i64, data: Value) -> InstanceIn {
        InstanceIn { key: key.into(), old: old.map(String::from), ord, data }
    }

    #[test]
    fn editar_renomeando_move_a_linha_em_vez_de_duplicar() {
        let (db, s) = db();
        db.put_instance(s, &inc("sonarr", None, 0, json!({"id":"sonarr","title":"Sonarr"})))
            .unwrap();
        db.put_instance(
            s,
            &inc("series", Some("sonarr"), 0, json!({"id":"sonarr","title":"Séries"})),
        )
        .unwrap();
        let all = db.instances(s).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["title"], json!("Séries"));
    }

    #[test]
    fn as_pastas_do_jellyfin_voltam_na_ordem() {
        let (db, s) = db();
        db.put_instance(
            s,
            &inc(
                "jellyfin",
                None,
                0,
                json!({"id":"jellyfin","title":"Jellyfin","libs":["/mnt/a","/mnt/b"]}),
            ),
        )
        .unwrap();
        let all = db.instances(s).unwrap();
        assert_eq!(all[0]["libs"], json!(["/mnt/a", "/mnt/b"]));
    }

    #[test]
    fn o_que_nao_e_coluna_volta_inteiro() {
        let (db, s) = db();
        db.put_instance(
            s,
            &inc("qbittorrent", None, 0, json!({"id":"qbittorrent","flagNova":42})),
        )
        .unwrap();
        let all = db.instances(s).unwrap();
        assert_eq!(all[0]["flagNova"], json!(42));
    }

    #[test]
    fn reconcile_apaga_o_que_saiu_e_acerta_a_ordem() {
        let (db, s) = db();
        db.put_instance(s, &inc("a", None, 0, json!({"id":"sonarr"}))).unwrap();
        db.put_instance(s, &inc("b", None, 1, json!({"id":"radarr"}))).unwrap();
        db.reconcile(s, &["b".to_string()]).unwrap();
        let all = db.instances(s).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["id"], json!("radarr"));
    }

    #[test]
    fn stacks_diferentes_nao_se_misturam() {
        let (db, s) = db();
        let outra = db.create_stack("Sítio", Path::new("/srv")).unwrap().id;
        db.put_instance(s, &inc("sonarr", None, 0, json!({"id":"sonarr"}))).unwrap();
        db.put_instance(outra, &inc("sonarr", None, 0, json!({"id":"sonarr"}))).unwrap();
        db.delete_instance(s, "sonarr").unwrap();
        assert_eq!(db.instances(s).unwrap().len(), 0);
        assert_eq!(db.instances(outra).unwrap().len(), 1);
    }
}

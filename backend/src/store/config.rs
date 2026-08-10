/* A Configuração: as ligações entre as instâncias.

   O `CONFIG` da página é remontado pelo `syncConfig()` a cada abertura do modal
   e chega aqui inteiro, então gravar é trocar o que está guardado — não há
   edição parcial a preservar. As três partes viram cinco tabelas:

     apps    → cfg_app                       (o que o Prowlarr configura)
     clients → cfg_client + cfg_client_arr   (quem recebe, com que categoria)
     mm      → cfg_mm + cfg_naming           (por família, não por instância)

   O valor de cada campo da nomenclatura vai em JSON porque o `NAMING_FIELDS`
   mistura caixa de seleção com formato de texto. */

use rusqlite::params;
use serde_json::{json, Map, Value};

use super::{flag, obj, text, Db};

impl Db {
    pub fn put_config(&self, stack_id: i64, v: &Value) -> Result<(), String> {
        let c = v
            .as_object()
            .ok_or_else(|| "a Configuração não veio como objeto".to_string())?;
        let apps = obj(c.get("apps"));
        let clients = obj(c.get("clients"));
        let mm = obj(c.get("mm"));

        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for table in ["cfg_app", "cfg_client", "cfg_mm"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE stack_id = ?1"),
                params![stack_id],
            )
            .map_err(|e| e.to_string())?;
        }

        for (key, on) in &apps {
            tx.execute(
                "INSERT INTO cfg_app (stack_id, arr_key, enabled) VALUES (?1,?2,?3)",
                params![stack_id, key, on.as_bool().unwrap_or(false) as i64],
            )
            .map_err(|e| e.to_string())?;
        }

        for (key, val) in &clients {
            let cl = obj(Some(val));
            let cdh = cl.get("cdh").and_then(|v| v.as_object()).cloned();
            tx.execute(
                "INSERT INTO cfg_client (stack_id, client_key, cdh_completed, cdh_failed)
                 VALUES (?1,?2,?3,?4)",
                params![
                    stack_id,
                    key,
                    cdh.as_ref().map(|m| flag(m, "completed")),
                    cdh.as_ref().map(|m| flag(m, "failed")),
                ],
            )
            .map_err(|e| e.to_string())?;

            let arrs = obj(cl.get("arrs"));
            let cats = obj(cl.get("cats"));
            // a categoria pode existir para um arr que não está no `arrs`, e
            // vice-versa: a linha é a união dos dois
            let mut keys: Vec<&String> = arrs.keys().chain(cats.keys()).collect();
            keys.sort();
            keys.dedup();
            for arr in keys {
                tx.execute(
                    "INSERT INTO cfg_client_arr (stack_id, client_key, arr_key, enabled, category)
                     VALUES (?1,?2,?3,?4,?5)",
                    params![
                        stack_id,
                        key,
                        arr,
                        arrs.get(arr).and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                        text(&cats, arr),
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        for (sid, val) in &mm {
            let m = obj(Some(val));
            tx.execute(
                "INSERT INTO cfg_mm
                   (stack_id, service_id, hardlink, rename, perms, empty, chmod, chown)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    stack_id,
                    sid,
                    flag(&m, "hardlink"),
                    flag(&m, "rename"),
                    flag(&m, "perms"),
                    flag(&m, "empty"),
                    text(&m, "chmod"),
                    text(&m, "chown"),
                ],
            )
            .map_err(|e| e.to_string())?;

            for (field, value) in &obj(m.get("naming")) {
                tx.execute(
                    "INSERT INTO cfg_naming (stack_id, service_id, field, value)
                     VALUES (?1,?2,?3,?4)",
                    params![stack_id, sid, field, value.to_string()],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub(crate) fn config(&self, stack_id: i64) -> Result<Value, String> {
        let conn = self.lock()?;

        let mut apps = Map::new();
        conn.prepare("SELECT arr_key, enabled FROM cfg_app WHERE stack_id = ?1")
            .and_then(|mut st| {
                st.query_map(params![stack_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|e| e.to_string())?
            .into_iter()
            .for_each(|(k, on)| {
                apps.insert(k, Value::Bool(on));
            });

        let mut clients = Map::new();
        let rows = conn
            .prepare(
                "SELECT client_key, cdh_completed, cdh_failed FROM cfg_client WHERE stack_id = ?1",
            )
            .and_then(|mut st| {
                st.query_map(params![stack_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|e| e.to_string())?;
        let mut per_arr = conn
            .prepare(
                "SELECT arr_key, enabled, category FROM cfg_client_arr
                  WHERE stack_id = ?1 AND client_key = ?2 ORDER BY arr_key",
            )
            .map_err(|e| e.to_string())?;
        for (key, done, failed) in rows {
            let mut arrs = Map::new();
            let mut cats = Map::new();
            for row in per_arr
                .query_map(params![stack_id, key], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)? != 0,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| e.to_string())?
            {
                let (arr, on, cat) = row.map_err(|e| e.to_string())?;
                arrs.insert(arr.clone(), Value::Bool(on));
                cats.insert(arr, Value::String(cat));
            }
            let mut cl = Map::new();
            cl.insert("arrs".into(), Value::Object(arrs));
            cl.insert("cats".into(), Value::Object(cats));
            if let (Some(d), Some(f)) = (done, failed) {
                cl.insert("cdh".into(), json!({"completed": d != 0, "failed": f != 0}));
            }
            clients.insert(key, Value::Object(cl));
        }
        drop(per_arr);

        let mut mm = Map::new();
        let fams = conn
            .prepare(
                "SELECT service_id, hardlink, rename, perms, empty, chmod, chown
                   FROM cfg_mm WHERE stack_id = ?1",
            )
            .and_then(|mut st| {
                st.query_map(params![stack_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        json!({
                            "hardlink": r.get::<_, i64>(1)? != 0,
                            "rename":   r.get::<_, i64>(2)? != 0,
                            "perms":    r.get::<_, i64>(3)? != 0,
                            "empty":    r.get::<_, i64>(4)? != 0,
                            "chmod":    r.get::<_, String>(5)?,
                            "chown":    r.get::<_, String>(6)?,
                        }),
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|e| e.to_string())?;
        let mut per_field = conn
            .prepare(
                "SELECT field, value FROM cfg_naming
                  WHERE stack_id = ?1 AND service_id = ?2",
            )
            .map_err(|e| e.to_string())?;
        for (sid, mut fam) in fams {
            let mut naming = Map::new();
            for row in per_field
                .query_map(params![stack_id, sid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?
            {
                let (field, raw) = row.map_err(|e| e.to_string())?;
                naming.insert(
                    field,
                    serde_json::from_str(&raw).unwrap_or(Value::String(raw)),
                );
            }
            fam.as_object_mut()
                .expect("montado como objeto acima")
                .insert("naming".into(), Value::Object(naming));
            mm.insert(sid, fam);
        }

        Ok(json!({"apps": apps, "clients": clients, "mm": mm}))
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

    fn sample() -> Value {
        json!({
            "apps": {"sonarr": true, "series": false},
            "clients": {
                "qbittorrent": {
                    "arrs": {"sonarr": true, "radarr": false},
                    "cats": {"sonarr": "tv-sonarr", "radarr": "radarr"}
                },
                "sabnzbd": {
                    "arrs": {"sonarr": true},
                    "cats": {"sonarr": "tv-sonarr"},
                    "cdh": {"completed": true, "failed": false}
                }
            },
            "mm": {
                "sonarr": {
                    "hardlink": true, "rename": true, "perms": false, "empty": false,
                    "chmod": "755", "chown": "",
                    "naming": {"illegal": true, "colon": "smart",
                               "seasonFolder": "Season {season:00}"}
                }
            }
        })
    }

    #[test]
    fn a_configuracao_volta_como_foi() {
        let (db, s) = db();
        db.put_config(s, &sample()).unwrap();
        assert_eq!(db.config(s).unwrap(), sample());
    }

    #[test]
    fn gravar_de_novo_troca_em_vez_de_somar() {
        let (db, s) = db();
        db.put_config(s, &sample()).unwrap();
        db.put_config(s, &json!({"apps": {"radarr": true}, "clients": {}, "mm": {}}))
            .unwrap();
        let back = db.config(s).unwrap();
        assert_eq!(back["apps"], json!({"radarr": true}));
        assert_eq!(back["clients"], json!({}));
        assert_eq!(back["mm"], json!({}));
    }

    #[test]
    fn sem_cdh_a_chave_nao_aparece() {
        let (db, s) = db();
        db.put_config(s, &sample()).unwrap();
        let back = db.config(s).unwrap();
        assert!(back["clients"]["qbittorrent"].get("cdh").is_none());
        assert_eq!(back["clients"]["sabnzbd"]["cdh"]["completed"], json!(true));
    }
}

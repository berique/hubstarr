/* Configuração dos apps pela API deles.

   É o único pedaço da Configuração que não cabe em arquivo: Prowlarr, clientes
   de download e Media Management vivem no banco de cada app, e só entram por
   HTTP depois que ele subiu. Até aqui o `CONFIG` da página não chegava a lugar
   nenhum — este módulo é o consumidor dele.

   Quem monta o plano é a página: ela é que sabe o `container_name`, o subpath e
   como cada campo da interface se chama na API. Aqui só resta executar, e por
   isso os campos chegam com o nome que a API usa. Os apps são alcançados pelo
   nginx, no endereço que a stack publica no host — o mesmo caminho que o
   navegador usa, então o subpath já está certo.

   Tudo é idempotente: o que já existe com aquele nome é atualizado, não
   duplicado. Rodar de novo depois de mexer na Configuração é o uso normal. */

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::jobs::Log;

#[derive(Deserialize, Clone)]
pub struct Plan {
    /// endereço em que a stack responde no host, com o nginx na frente
    pub base: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default)]
    pub arrs: Vec<Arr>,
    #[serde(default)]
    pub prowlarrs: Vec<Prowlarr>,
    #[serde(default)]
    pub clients: Vec<Client>,
    /// quanto esperar cada app responder, em segundos
    #[serde(default = "default_wait")]
    pub wait: u64,
}

fn default_wait() -> u64 {
    180
}

#[derive(Deserialize, Clone)]
pub struct Arr {
    /// título da instância, só para o log
    pub name: String,
    /// subpath no nginx, como `/sonarr`
    pub route: String,
    /// versão da API do app: `v3` nos *arr, `v1` no Lidarr e no Prowlarr
    pub api: String,
    /// campos de `config/mediamanagement`, já com o nome que a API usa
    #[serde(default)]
    pub mm: Map<String, Value>,
    /// campos de `config/naming`
    #[serde(default)]
    pub naming: Map<String, Value>,
}

#[derive(Deserialize, Clone)]
pub struct Prowlarr {
    pub name: String,
    pub route: String,
    pub api: String,
    /// os *arr que ele vai manter sincronizados
    #[serde(default)]
    pub apps: Vec<AppLink>,
}

#[derive(Deserialize, Clone)]
pub struct AppLink {
    /// nome do recurso dentro do Prowlarr
    pub name: String,
    /// `Sonarr`, `Radarr`, `Lidarr` — como a API do Prowlarr chama
    pub implementation: String,
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default)]
    pub props: Map<String, Value>,
}

#[derive(Deserialize, Clone)]
pub struct Client {
    /// `QBittorrent`, `Sabnzbd`
    pub implementation: String,
    /// título do cliente, só para o log
    pub label: String,
    #[serde(default)]
    pub targets: Vec<Target>,
}

/// Um cliente dentro de um *arr: mesmo cliente, categoria por app.
#[derive(Deserialize, Clone)]
pub struct Target {
    /// subpath do *arr que recebe o cliente
    pub arr: String,
    /// nome do recurso dentro daquele app
    pub name: String,
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default)]
    pub props: Map<String, Value>,
}

pub async fn apply(plan: Plan, log: Log) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let api = Api {
        http,
        base: plan.base.trim_end_matches('/').to_string(),
        key: plan.api_key.clone(),
    };

    // 1) esperar todo mundo de pé; sem isso o resto falha em cascata
    for a in plan.arrs.iter().map(|a| (&a.name, &a.route, &a.api)).chain(
        plan.prowlarrs
            .iter()
            .map(|p| (&p.name, &p.route, &p.api)),
    ) {
        api.wait_ready(a.0, a.1, a.2, plan.wait, &log).await?;
    }

    // 2) Media Management e nomenclatura, um PUT por app
    for a in &plan.arrs {
        if !a.mm.is_empty() {
            api.patch_config(&a.route, &a.api, "mediamanagement", &a.mm, &log)
                .await
                .map_err(|e| format!("{}: media management: {e}", a.name))?;
        }
        if !a.naming.is_empty() {
            api.patch_config(&a.route, &a.api, "naming", &a.naming, &log)
                .await
                .map_err(|e| format!("{}: nomenclatura: {e}", a.name))?;
        }
    }

    // 3) clientes de download, um recurso por par cliente × app
    for c in &plan.clients {
        for tg in &c.targets {
            let Some(arr) = plan.arrs.iter().find(|a| a.route == tg.arr) else {
                log.line(format!("{}: {} não está na stack, pulei", c.label, tg.arr));
                continue;
            };
            api.upsert(
                &arr.route,
                &arr.api,
                "downloadclient",
                &tg.name,
                &c.implementation,
                &tg.fields,
                &tg.props,
                &log,
            )
            .await
            .map_err(|e| format!("{} em {}: {e}", c.label, arr.name))?;
        }
    }

    // 4) o Prowlarr apontando para cada *arr
    for p in &plan.prowlarrs {
        for app in &p.apps {
            api.upsert(
                &p.route,
                &p.api,
                "applications",
                &app.name,
                &app.implementation,
                &app.fields,
                &app.props,
                &log,
            )
            .await
            .map_err(|e| format!("{}: {}: {e}", p.name, app.name))?;
        }
    }

    log.line("configuração aplicada.");
    Ok(())
}

struct Api {
    http: reqwest::Client,
    base: String,
    key: String,
}

impl Api {
    fn url(&self, route: &str, api: &str, path: &str) -> String {
        let route = route.trim_end_matches('/');
        format!("{}{}/api/{}/{}", self.base, route, api, path)
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<Value, String> {
        let res = req
            .header("X-Api-Key", &self.key)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            // o corpo do erro dos *arr é curto e diz o campo que recusaram
            let short: String = body.chars().take(300).collect();
            return Err(format!("HTTP {status}: {short}"));
        }
        if body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    async fn get(&self, route: &str, api: &str, path: &str) -> Result<Value, String> {
        self.send(self.http.get(self.url(route, api, path))).await
    }

    /// Espera o app responder o `system/status`. Recém-subido ele demora — o
    /// container sobe muito antes de a aplicação atender.
    async fn wait_ready(
        &self,
        name: &str,
        route: &str,
        api: &str,
        secs: u64,
        log: &Log,
    ) -> Result<(), String> {
        log.line(format!("esperando o {name} responder…"));
        let deadline = std::time::Instant::now() + Duration::from_secs(secs);
        // só interessa o último erro, e ele só é lido quando o prazo acaba
        let mut last;
        loop {
            match self.get(route, api, "system/status").await {
                Ok(_) => {
                    log.line(format!("{name}: de pé"));
                    return Ok(());
                }
                Err(e) => last = e,
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!("{name} não respondeu em {secs}s ({last})"));
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    /// Lê a configuração, sobrescreve os campos pedidos e devolve inteira: os
    /// *arr recusam PUT parcial, querem o recurso completo com o `id`.
    async fn patch_config(
        &self,
        route: &str,
        api: &str,
        what: &str,
        fields: &Map<String, Value>,
        log: &Log,
    ) -> Result<(), String> {
        let path = format!("config/{what}");
        let mut cur = self.get(route, api, &path).await?;
        let Some(obj) = cur.as_object_mut() else {
            return Err(format!("{path} não veio como objeto"));
        };
        for (k, v) in fields {
            obj.insert(k.clone(), v.clone());
        }
        let id = obj.get("id").and_then(|v| v.as_i64()).unwrap_or(1);
        self.send(
            self.http
                .put(self.url(route, api, &format!("{path}/{id}")))
                .json(&cur),
        )
        .await?;
        log.line(format!("{route}: {what} aplicado"));
        Ok(())
    }

    /// Cria ou atualiza um recurso com `name`, montando-o a partir do schema
    /// que o próprio app publica — é ele que traz a lista de campos certa para
    /// aquela implementação e aquela versão.
    #[allow(clippy::too_many_arguments)]
    async fn upsert(
        &self,
        route: &str,
        api: &str,
        kind: &str,
        name: &str,
        implementation: &str,
        fields: &Map<String, Value>,
        props: &Map<String, Value>,
        log: &Log,
    ) -> Result<(), String> {
        let existing = self.get(route, api, kind).await?;
        let found = existing
            .as_array()
            .and_then(|a| a.iter().find(|r| r.get("name").and_then(|n| n.as_str()) == Some(name)))
            .cloned();

        let mut res = match &found {
            Some(r) => r.clone(),
            None => {
                let schema = self.get(route, api, &format!("{kind}/schema")).await?;
                schema
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .find(|s| {
                                s.get("implementation").and_then(|v| v.as_str())
                                    == Some(implementation)
                            })
                            .cloned()
                    })
                    .ok_or_else(|| format!("{implementation} não existe no schema de {kind}"))?
            }
        };

        {
            let obj = res
                .as_object_mut()
                .ok_or_else(|| format!("{kind} não veio como objeto"))?;
            obj.insert("name".into(), Value::String(name.into()));
            for (k, v) in props {
                obj.insert(k.clone(), v.clone());
            }
        }
        set_fields(&mut res, fields);

        match found {
            Some(old) => {
                let id = old.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
                self.send(
                    self.http
                        .put(self.url(route, api, &format!("{kind}/{id}")))
                        .json(&res),
                )
                .await?;
                log.line(format!("{route}: {kind} “{name}” atualizado"));
            }
            None => {
                self.send(self.http.post(self.url(route, api, kind)).json(&res))
                    .await?;
                log.line(format!("{route}: {kind} “{name}” criado"));
            }
        }
        Ok(())
    }
}

/// Os *arr guardam a configuração de cada recurso numa lista `fields`, com
/// `{name, value}`. Sobrescreve o que existe e acrescenta o que faltar.
fn set_fields(res: &mut Value, fields: &Map<String, Value>) {
    if fields.is_empty() {
        return;
    }
    let Some(obj) = res.as_object_mut() else { return };
    let Some(list) = obj
        .entry("fields")
        .or_insert_with(|| Value::Array(vec![]))
        .as_array_mut()
    else {
        return;
    };
    for (k, v) in fields {
        match list
            .iter_mut()
            .find(|f| f.get("name").and_then(|n| n.as_str()) == Some(k.as_str()))
        {
            Some(f) => {
                if let Some(o) = f.as_object_mut() {
                    o.insert("value".into(), v.clone());
                }
            }
            None => list.push(serde_json::json!({"name": k, "value": v})),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn campo_existente_e_sobrescrito_e_o_que_falta_entra() {
        let mut res = json!({"fields": [{"name": "host", "value": "localhost"}]});
        let mut f = Map::new();
        f.insert("host".into(), json!("qbittorrent"));
        f.insert("tvCategory".into(), json!("tv-sonarr"));
        set_fields(&mut res, &f);
        let list = res["fields"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["value"], json!("qbittorrent"));
        assert_eq!(list[1]["name"], json!("tvCategory"));
    }
}

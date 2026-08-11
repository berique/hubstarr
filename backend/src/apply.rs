/* Aplica a Configuração na stack que já está no ar (v0.3).

   Até aqui o servidor só gravava arquivo e chamava o docker: o que a página
   montava, ele escrevia. Um cliente de download não é arquivo — o *arr guarda
   isso no banco dele e só aceita pela API —, então esta é a primeira coisa que
   o servidor monta: o corpo JSON de cada `downloadclient`.

   O que ele *não* faz é decidir. Endereço, porta, categoria, quem recebe o
   quê, o nome do campo de categoria de cada família: tudo isso vem pronto da
   página, que é onde o `SERVICES` e o `CONFIG` vivem. Aqui só entra o formato
   da API dos *arr — implementação, contrato e a lista de `fields`.

   Os apps são alcançados pelo nginx, não por container: o servidor roda no
   host e a rede `starrnet` não existe para ele, mas o nginx publica porta e
   serve cada app no subpath dele. É a página quem manda o endereço em `base`,
   porque é ela que sabe se a stack subiu com TLS e em que portas.

   Aplicar de novo não duplica: cada cliente é procurado pelo nome na lista do
   *arr, e o que já está lá é atualizado no lugar. */

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::jobs::Log;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Req {
    /// como o servidor alcança o nginx da stack, ex. `http://127.0.0.1:8080`
    base: String,
    /// a `STARR_APIKEY` do `.env`, que é a mesma em todos os *arr
    api_key: String,
    arrs: Vec<Arr>,
    clients: Vec<Client>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Arr {
    /// `container_name`, que é a chave da instância na página
    key: String,
    name: String,
    /// subpath em que o nginx o serve, ex. `/sonarr`
    route: String,
    /// versão da API dele: `v3` no Sonarr e no Radarr, `v1` no Lidarr
    api: String,
    /// nome do campo de categoria da família: `tvCategory`, `movieCategory`, …
    cat_field: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    name: String,
    /// `qbittorrent` ou `sabnzbd` — é o que escolhe implementação e contrato
    kind: String,
    /// endereço pela rede da stack, do ponto de vista do *arr (o gluetun,
    /// quando o cliente roteia pela VPN)
    host: String,
    port: u16,
    #[serde(default)]
    user: String,
    #[serde(default)]
    pass: String,
    /// a chave da API do SABnzbd; o qBittorrent não a usa aqui
    #[serde(default)]
    api_key: String,
    /// categoria por instância de *arr, pela chave dela
    #[serde(default)]
    cats: Map<String, Value>,
    /// remover da fila o que concluiu e o que falhou; ausente = não mexer
    #[serde(default)]
    cdh: Option<Cdh>,
}

#[derive(Deserialize)]
struct Cdh {
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    failed: bool,
}

fn field(name: &str, value: Value) -> Value {
    json!({"name": name, "value": value})
}

impl Client {
    /// O recurso `downloadclient` como o *arr o espera. O que muda de um
    /// cliente para o outro é a implementação, o protocolo e os campos de
    /// credencial; o resto é igual nas três famílias.
    fn body(&self, arr: &Arr) -> Result<Value, String> {
        let cat = self
            .cats
            .get(&arr.key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let mut fields = vec![
            field("host", json!(self.host)),
            field("port", json!(self.port)),
            field("useSsl", json!(false)),
            field("urlBase", json!("")),
            field(&arr.cat_field, json!(cat)),
        ];
        let (implementation, contract, protocol) = match self.kind.as_str() {
            "qbittorrent" => {
                fields.push(field("username", json!(self.user)));
                fields.push(field("password", json!(self.pass)));
                ("QBittorrent", "QBittorrentSettings", "torrent")
            }
            "sabnzbd" => {
                fields.push(field("apiKey", json!(self.api_key)));
                ("Sabnzbd", "SabnzbdSettings", "usenet")
            }
            other => return Err(format!("cliente de download desconhecido: {other}")),
        };
        Ok(json!({
            "name": self.name,
            "enable": true,
            "protocol": protocol,
            "priority": 1,
            "removeCompletedDownloads": self.cdh.as_ref().map(|c| c.completed).unwrap_or(false),
            "removeFailedDownloads": self.cdh.as_ref().map(|c| c.failed).unwrap_or(false),
            "implementation": implementation,
            "implementationName": self.name,
            "configContract": contract,
            "fields": fields,
        }))
    }
}

/// Uma passada por cada par (*arr, cliente). Um app fora do ar não derruba os
/// outros: o erro dele vira uma linha no log e a volta continua — quem aplica
/// isso costuma ter acabado de subir a stack, e um app ainda subindo não é
/// motivo para não configurar o resto.
pub async fn download_clients(req: Req, log: Log) -> Result<(), String> {
    if req.arrs.is_empty() {
        return Err("nenhum *arr na stack para configurar".into());
    }
    if req.clients.is_empty() {
        return Err("nenhum cliente de download na stack para registrar".into());
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        // a stack é a da própria máquina e o certificado dela costuma ser o
        // que o dono pôs à mão: aqui o que vale é alcançar o app, não provar
        // quem ele é
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;

    let base = req.base.trim_end_matches('/').to_string();
    let mut falhas = 0;
    for arr in &req.arrs {
        let url = format!("{base}{}/api/{}/downloadclient", arr.route, arr.api);
        let atuais = match list(&http, &url, &req.api_key).await {
            Ok(v) => v,
            Err(e) => {
                log.line(format!("{}: {e}", arr.name));
                falhas += req.clients.len();
                continue;
            }
        };
        for client in &req.clients {
            let body = match client.body(arr) {
                Ok(b) => b,
                Err(e) => {
                    log.line(format!("{} → {}: {e}", arr.name, client.name));
                    falhas += 1;
                    continue;
                }
            };
            let existente = atuais
                .iter()
                .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(client.name.as_str()))
                .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
            match send(&http, &url, &req.api_key, existente, body).await {
                Ok(()) => log.line(format!(
                    "{} → {}: {}",
                    arr.name,
                    client.name,
                    if existente.is_some() {
                        "atualizado"
                    } else {
                        "registrado"
                    }
                )),
                Err(e) => {
                    log.line(format!("{} → {}: {e}", arr.name, client.name));
                    falhas += 1;
                }
            }
        }
    }
    if falhas > 0 {
        return Err(format!("{falhas} ligação(ões) não passaram"));
    }
    log.line("Configuração aplicada.");
    Ok(())
}

/// Os clientes que o *arr já tem, para saber o que é registro novo e o que é
/// atualização.
async fn list(http: &reqwest::Client, url: &str, key: &str) -> Result<Vec<Value>, String> {
    let r = http
        .get(url)
        .header("X-Api-Key", key)
        .send()
        .await
        .map_err(|e| format!("não respondeu ({e})"))?;
    let st = r.status();
    let txt = r.text().await.unwrap_or_default();
    if !st.is_success() {
        return Err(erro(st, &txt));
    }
    match serde_json::from_str::<Value>(&txt) {
        Ok(Value::Array(a)) => Ok(a),
        _ => Err("resposta não foi a lista de clientes".into()),
    }
}

async fn send(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    id: Option<i64>,
    mut body: Value,
) -> Result<(), String> {
    // atualizar exige o id no corpo *e* no caminho: o *arr recusa se divergirem
    let alvo = match id {
        Some(id) => {
            body["id"] = json!(id);
            format!("{url}/{id}")
        }
        None => url.to_string(),
    };
    let req = if id.is_some() {
        http.put(&alvo)
    } else {
        http.post(&alvo)
    };
    let r = req
        .header("X-Api-Key", key)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("não respondeu ({e})"))?;
    let st = r.status();
    if st.is_success() {
        return Ok(());
    }
    Err(erro(st, &r.text().await.unwrap_or_default()))
}

/// A mensagem do *arr, quando ele manda uma: a lista de validação dele diz
/// bem mais do que o número do status — "categoria não existe", "não consegui
/// falar com o cliente", e por aí.
fn erro(st: reqwest::StatusCode, corpo: &str) -> String {
    let detalhe = match serde_json::from_str::<Value>(corpo) {
        Ok(Value::Array(a)) => a
            .iter()
            .filter_map(|e| e.get("errorMessage").and_then(|m| m.as_str()))
            .collect::<Vec<_>>()
            .join("; "),
        Ok(v) => v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string(),
        Err(_) => String::new(),
    };
    if detalhe.is_empty() {
        format!("HTTP {}", st.as_u16())
    } else {
        format!("HTTP {} — {detalhe}", st.as_u16())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arr(key: &str, cat_field: &str) -> Arr {
        Arr {
            key: key.into(),
            name: key.into(),
            route: format!("/{key}"),
            api: "v3".into(),
            cat_field: cat_field.into(),
        }
    }

    fn qbit() -> Client {
        Client {
            name: "qBittorrent".into(),
            kind: "qbittorrent".into(),
            host: "gluetun".into(),
            port: 8181,
            user: "admin".into(),
            pass: "senha".into(),
            api_key: String::new(),
            cats: serde_json::from_str(r#"{"sonarr":"tv-sonarr"}"#).unwrap(),
            cdh: Some(Cdh {
                completed: true,
                failed: false,
            }),
        }
    }

    fn val(body: &Value, name: &str) -> Value {
        body["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("sem o campo {name}"))["value"]
            .clone()
    }

    #[test]
    fn o_qbittorrent_sai_com_o_contrato_e_as_credenciais_dele() {
        let b = qbit().body(&arr("sonarr", "tvCategory")).unwrap();
        assert_eq!(b["implementation"], "QBittorrent");
        assert_eq!(b["configContract"], "QBittorrentSettings");
        assert_eq!(b["protocol"], "torrent");
        assert_eq!(val(&b, "host"), "gluetun");
        assert_eq!(val(&b, "port"), 8181);
        assert_eq!(val(&b, "username"), "admin");
        assert_eq!(val(&b, "tvCategory"), "tv-sonarr");
        assert_eq!(b["removeCompletedDownloads"], true);
        assert_eq!(b["removeFailedDownloads"], false);
    }

    #[test]
    fn o_sabnzbd_troca_usuario_e_senha_pela_chave_da_api() {
        let mut c = qbit();
        c.kind = "sabnzbd".into();
        c.name = "SABnzbd".into();
        c.api_key = "chave".into();
        let b = c.body(&arr("radarr", "movieCategory")).unwrap();
        assert_eq!(b["implementation"], "Sabnzbd");
        assert_eq!(b["protocol"], "usenet");
        assert_eq!(val(&b, "apiKey"), "chave");
        assert!(b["fields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["name"] != "password"));
    }

    #[test]
    fn a_categoria_e_a_da_instancia_e_some_quando_ela_nao_tem() {
        let c = qbit();
        // o `cats` só traz o sonarr: o radarr entra sem categoria, que é o
        // que o *arr entende como "a raiz do cliente"
        assert_eq!(val(&c.body(&arr("radarr", "movieCategory")).unwrap(), "movieCategory"), "");
    }

    #[test]
    fn cliente_de_tipo_desconhecido_para_naquele_par_e_nao_no_meio_do_corpo() {
        let mut c = qbit();
        c.kind = "transmission".into();
        assert!(c.body(&arr("sonarr", "tvCategory")).is_err());
    }

    #[test]
    fn a_mensagem_de_validacao_do_arr_chega_ao_log() {
        let e = erro(
            reqwest::StatusCode::BAD_REQUEST,
            r#"[{"errorMessage":"Unable to connect to qBittorrent"}]"#,
        );
        assert!(e.contains("400"));
        assert!(e.contains("Unable to connect"));
        assert_eq!(erro(reqwest::StatusCode::NOT_FOUND, "não é json"), "HTTP 404");
    }
}

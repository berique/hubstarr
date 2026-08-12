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
    /// o Prowlarr da stack, quando ele está nela
    #[serde(default)]
    prowlarr: Option<Prowlarr>,
    /// o resolvedor de desafios da Cloudflare, quando ele está na stack
    #[serde(default)]
    solver: Option<Solver>,
    /// Media Management e nomenclatura, por família — como na página, é por
    /// app e não por instância
    #[serde(default)]
    mm: Map<String, Value>,
    /// há um Configarr na stack? Os perfis de qualidade não passam por aqui —
    /// eles estão no `config.yml` que a página gerou —, mas é este `apply` que
    /// já esperou os apps responderem, e é depois dele que o Configarr roda.
    #[serde(default)]
    configarr: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Solver {
    /// o título da instância, que é o nome do registro no Prowlarr
    name: String,
    /// como o Prowlarr o alcança na rede da stack: `http://flaresolverr:8191`
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Prowlarr {
    /// subpath em que o nginx o serve, por onde o servidor fala com ele
    route: String,
    /// como o *arr enxerga o Prowlarr dentro da rede da stack, já com a base
    /// URL dele: `http://prowlarr:9696/prowlarr`
    url: String,
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
    /// a família — `sonarr`, `radarr` ou `lidarr` —, que no Prowlarr escolhe a
    /// implementação e as categorias sincronizadas
    family: String,
    /// como o Prowlarr o alcança dentro da rede da stack, já com a base URL:
    /// `http://sonarr:8989/sonarr`
    #[serde(default)]
    internal_url: String,
    /// o Prowlarr deve sincronizar com ele? é a caixa da Configuração
    #[serde(default)]
    sync: bool,
    /// campos da nomenclatura que esta instância não recebe: a página os deixou
    /// de fora, e não mandar a chave é o que faz o app manter a dele
    #[serde(default)]
    skip_naming: Vec<String>,
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
    /// como o servidor alcança a interface dele: pelo nginx, ou direto na porta
    /// publicada quando ele não tem rota. É por aqui que as categorias do
    /// SABnzbd são criadas, e é o que a espera consulta
    #[serde(default)]
    web_url: String,
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

impl Req {
    /// Há alguma ligação para fazer? O Subir pergunta antes de chamar: numa
    /// stack sem nada a ligar, não aplicar não é erro nenhum.
    ///
    /// São duas metades independentes. Os *arr precisam de algo para receber —
    /// cliente, Media Management ou o registro no Prowlarr. E o Prowlarr tem
    /// trabalho próprio: os clientes dele e o resolvedor de desafios, que valem
    /// mesmo numa stack onde ainda não há *arr nenhum.
    pub fn tem_o_que_fazer(&self) -> bool {
        let pelos_arrs = !self.arrs.is_empty()
            && (!self.clients.is_empty() || self.prowlarr.is_some() || !self.mm.is_empty());
        let pelo_prowlarr = self.prowlarr.is_some()
            && (!self.clients.is_empty() || self.solver.is_some() || !self.arrs.is_empty());
        pelos_arrs || pelo_prowlarr
    }

    /// Rodar o Configarr depois? Independe do `tem_o_que_fazer()`: uma stack sem
    /// cliente de download e sem Prowlarr ainda pode querer os perfis.
    pub fn quer_configarr(&self) -> bool {
        self.configarr && !self.arrs.is_empty()
    }
}

/// O Prowlarr como alvo: mesma API dos *arr, versão v1, e o campo de categoria
/// dele não é por família — é um `category` só.
fn alvo_prowlarr(p: &Prowlarr) -> Arr {
    Arr {
        key: "prowlarr".into(),
        name: "Prowlarr".into(),
        route: p.route.clone(),
        api: "v1".into(),
        cat_field: "category".into(),
        family: "prowlarr".into(),
        internal_url: String::new(),
        sync: false,
        skip_naming: Vec::new(),
    }
}

impl Arr {
    /// Cópia para a volta dos clientes de download, que não usa o resto.
    fn clone_alvo(&self) -> Arr {
        Arr {
            key: self.key.clone(),
            name: self.name.clone(),
            route: self.route.clone(),
            api: self.api.clone(),
            cat_field: self.cat_field.clone(),
            family: self.family.clone(),
            internal_url: String::new(),
            sync: false,
            skip_naming: self.skip_naming.clone(),
        }
    }
}

fn field(name: &str, value: Value) -> Value {
    json!({"name": name, "value": value})
}

impl Client {
    /// A categoria com que uma instância de *arr usa este cliente.
    fn cat(&self, key: &str) -> String {
        self.cats
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Todas as categorias que este cliente recebe, sem repetir — é o que
    /// precisa existir dentro dele.
    fn categorias(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for v in self.cats.values() {
            let c = v.as_str().unwrap_or("").trim().to_string();
            if !c.is_empty() && !out.contains(&c) {
                out.push(c);
            }
        }
        out.sort();
        out
    }

    /// O recurso `downloadclient` como o app o espera. O que muda de um
    /// cliente para o outro é a implementação, o protocolo e os campos de
    /// credencial; o resto é igual em todos.
    ///
    /// `nome`, `cat_field` e `cat` entram de fora porque o mesmo cliente é
    /// registrado de duas maneiras: uma vez em cada *arr, com a categoria dele,
    /// e uma vez por instância no Prowlarr, onde o campo se chama `category` e
    /// o nome precisa distinguir os registros.
    fn body_como(
        &self,
        nome: &str,
        cat_field: &str,
        cat: &str,
        schema: Option<&Vec<Value>>,
    ) -> Result<Value, String> {
        let mut nossos: Vec<(String, Value)> = vec![
            ("host".into(), json!(self.host)),
            ("port".into(), json!(self.port)),
            ("useSsl".into(), json!(false)),
            ("urlBase".into(), json!("")),
            (cat_field.into(), json!(cat)),
        ];
        let (implementation, contract, protocol) = match self.kind.as_str() {
            "qbittorrent" => {
                /* Pela API key, não pela senha: ela não expira com a troca da
                   senha da interface e é o que a conf do próprio qBittorrent
                   recebe. Usuário e senha só entram no app que não conhece o
                   campo — quem diz isso é o schema dele. */
                let tem_api_key = schema
                    .map(|f| f.iter().any(|x| x["name"] == "apiKey"))
                    .unwrap_or(false);
                if tem_api_key {
                    nossos.push(("apiKey".into(), json!(self.api_key)));
                } else {
                    nossos.push(("username".into(), json!(self.user)));
                    nossos.push(("password".into(), json!(self.pass)));
                }
                ("QBittorrent", "QBittorrentSettings", "torrent")
            }
            "sabnzbd" => {
                nossos.push(("apiKey".into(), json!(self.api_key)));
                ("Sabnzbd", "SabnzbdSettings", "usenet")
            }
            other => return Err(format!("cliente de download desconhecido: {other}")),
        };

        /* Os campos saem do schema que o app publica, com os nossos por cima.
           Mandar só os nossos deixa o resto nulo, e o app estoura ao testar a
           conexão — o Prowlarr responde um "Object reference not set" que não
           diz nada. Sem schema (app que não o serve), vão só os nossos. */
        let mut fields: Vec<Value> = match schema {
            Some(base) => base
                .iter()
                .map(|f| {
                    let nome = f.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let valor = nossos
                        .iter()
                        .find(|(k, _)| k == nome)
                        .map(|(_, v)| v.clone())
                        .or_else(|| f.get("value").cloned())
                        .unwrap_or(Value::Null);
                    field(nome, valor)
                })
                .collect(),
            None => Vec::new(),
        };
        // o que é nosso e o schema não tinha entra no fim
        for (k, v) in &nossos {
            if !fields.iter().any(|f| f["name"] == k.as_str()) {
                fields.push(field(k, v.clone()));
            }
        }
        Ok(json!({
            "name": nome,
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

    /// O registro deste cliente num *arr: o nome é o do cliente, e a categoria
    /// é a que a Configuração deu àquela instância.
    fn body(&self, arr: &Arr, schema: Option<&Vec<Value>>) -> Result<Value, String> {
        self.body_como(&self.name, &arr.cat_field, &self.cat(&arr.key), schema)
    }
}

/// Uma passada por cada par (*arr, cliente). Um app fora do ar não derruba os
/// outros: o erro dele vira uma linha no log e a volta continua — quem aplica
/// isso costuma ter acabado de subir a stack, e um app ainda subindo não é
/// motivo para não configurar o resto.
/// Só espera os apps responderem, sem aplicar nada. É o caso da stack que tem
/// Configarr e mais nada para configurar: os perfis precisam de app de pé
/// tanto quanto os clientes de download, e sem isto ninguém teria esperado.
pub async fn esperar(req: &Req, log: &Log) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;
    let base = req.base.trim_end_matches('/').to_string();
    let alvos: Vec<Arr> = req.arrs.iter().map(Arr::clone_alvo).collect();
    esperar_apps(&http, &base, &alvos, log).await;
    Ok(())
}

pub async fn download_clients(req: Req, log: Log) -> Result<(), String> {
    if !req.tem_o_que_fazer() {
        return Err("nada para aplicar nesta stack".into());
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

    let alvos: Vec<Arr> = req.arrs.iter().map(Arr::clone_alvo).collect();

    let mut esperar_por: Vec<Arr> = alvos.iter().map(Arr::clone_alvo).collect();
    if let Some(p) = &req.prowlarr {
        esperar_por.push(alvo_prowlarr(p));
    }
    esperar_apps(&http, &base, &esperar_por, &log).await;
    esperar_clientes(&http, &req.clients, &log).await;

    for arr in &alvos {
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
            let esquema = schema_de(&http, &url, &req.api_key, implementacao(&client.kind)).await;
            let body = match client.body(arr, esquema.as_ref()) {
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
    if let Some(p) = &req.prowlarr {
        falhas += clientes_do_prowlarr(&http, &base, &req, p, &log).await;
        falhas += applications(&http, &base, &req, p, &log).await;
        if let Some(sv) = &req.solver {
            falhas += indexer_proxy(&http, &base, &req, p, sv, &log).await;
        }
    }
    // as categorias que os *arr e o Prowlarr vão pedir precisam existir dentro
    // do cliente
    for client in &req.clients {
        falhas += categorias_do_cliente(&http, &req, client, &log).await;
    }
    for arr in &req.arrs {
        falhas += media_management(&http, &base, &req, arr, &log).await;
    }
    if falhas > 0 {
        return Err(format!("{falhas} ligação(ões) não passaram"));
    }
    log.line("Configuração aplicada.");
    Ok(())
}

/* As categorias do newznab que o Prowlarr sincroniza com cada família. São o
   padrão dele, e vão explícitas de propósito: o campo aceita ficar vazio, e um
   `syncCategories` vazio é um Prowlarr que sincroniza indexer nenhum — falha
   silenciosa, que é a pior de todas aqui. Mexer nisso é mexer no que o app
   considera "série", "filme" e "música". */
fn sync_categories(family: &str) -> Vec<u32> {
    match family {
        "sonarr" => vec![5000, 5010, 5020, 5030, 5040, 5045, 5050, 5090],
        "radarr" => vec![2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060, 2070, 2080, 2090],
        "lidarr" => vec![3000, 3010, 3020, 3030, 3040, 3050, 3060],
        _ => vec![],
    }
}

/// O recurso `applications` do Prowlarr: um *arr para ele sincronizar. Os dois
/// endereços aqui são internos — de container para container, na rede da
/// stack —, não os do nginx: quem vai falar com o *arr é o Prowlarr, não o
/// servidor.
fn app_body(arr: &Arr, prowlarr_url: &str, api_key: &str) -> Result<Value, String> {
    let (implementation, contract) = match arr.family.as_str() {
        "sonarr" => ("Sonarr", "SonarrSettings"),
        "radarr" => ("Radarr", "RadarrSettings"),
        "lidarr" => ("Lidarr", "LidarrSettings"),
        other => return Err(format!("família sem aplicação no Prowlarr: {other}")),
    };
    if arr.internal_url.is_empty() {
        return Err("sem o endereço interno para o Prowlarr chamar".into());
    }
    Ok(json!({
        "name": arr.name,
        // o Prowlarr manda os indexers e também tira o que saiu; `addOnly`
        // deixaria lixo para trás a cada mudança
        "syncLevel": "fullSync",
        "implementation": implementation,
        "implementationName": implementation,
        "configContract": contract,
        "fields": [
            field("prowlarrUrl", json!(prowlarr_url)),
            field("baseUrl", json!(arr.internal_url)),
            field("apiKey", json!(api_key)),
            field("syncCategories", json!(sync_categories(&arr.family))),
        ],
        "tags": [],
    }))
}

/* O Prowlarr também tem Settings → Download Clients, e é por eles que sai o
   download da busca feita nele. Entra **um registro por cliente**, todos na
   mesma categoria: o que o Prowlarr pega é avulso — não veio de um *arr —,
   então fica junto, separado do que cada instância baixa.

   O nome é o do cliente, e é por ele que o reaplicar encontra o que já está
   lá. */
const CAT_PROWLARR: &str = "prowlarr";

async fn clientes_do_prowlarr(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    prowlarr: &Prowlarr,
    log: &Log,
) -> usize {
    let alvo = alvo_prowlarr(prowlarr);
    let url = format!("{base}{}/api/v1/downloadclient", prowlarr.route);
    let atuais = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(format!("Prowlarr: {e}"));
            return req.clients.len();
        }
    };
    let mut falhas = 0;
    for client in &req.clients {
        let esquema = schema_de(http, &url, &req.api_key, implementacao(&client.kind)).await;
        let body = match client.body_como(&client.name, &alvo.cat_field, CAT_PROWLARR, esquema.as_ref()) {
            /* O cliente do Prowlarr tem uma propriedade que os *arr não têm: o
               `categories`, que mapeia categoria do newznab para categoria do
               cliente. Vazia significa "vale para tudo" — mas **ausente** vira
               nula, e o teste de conexão dele estoura num
               `NullReferenceException` dentro do `ValidateCategories`, que não
               diz nada sobre a causa. */
            Ok(mut b) => {
                b["categories"] = json!([]);
                b
            }
            Err(e) => {
                log.line(format!("Prowlarr → {}: {e}", client.name));
                falhas += 1;
                continue;
            }
        };
        let existente = atuais
            .iter()
            .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(client.name.as_str()))
            .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
        match send(http, &url, &req.api_key, existente, body).await {
            Ok(()) => log.line(format!(
                "Prowlarr → {} [{CAT_PROWLARR}]: {}",
                client.name,
                if existente.is_some() { "atualizado" } else { "registrado" }
            )),
            Err(e) => {
                log.line(format!("Prowlarr → {}: {e}", client.name));
                falhas += 1;
            }
        }
    }
    falhas
}

/* O resolvedor de desafios no Prowlarr, em Settings → Indexers → Indexer
   Proxies. Sem isso, indexador atrás do desafio anti-bot da Cloudflare volta
   vazio ou com erro, e a página só sabia dizer como configurá-lo à mão.

   O Prowlarr casa proxy com indexador **por etiqueta**: o proxy vale para os
   indexadores que tiverem a etiqueta dele. Então aqui a etiqueta
   `flaresolverr` é criada (ou reaproveitada) e o registro nasce com ela — o que
   sobra para quem usa é marcar a etiqueta nos indexadores que precisam, que é
   justamente a escolha que o Hubstarr não tem como fazer por ninguém. */
const TAG_SOLVER: &str = "flaresolverr";

async fn indexer_proxy(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    prowlarr: &Prowlarr,
    solver: &Solver,
    log: &Log,
) -> usize {
    let tag_url = format!("{base}{}/api/v1/tag", prowlarr.route);
    let tag = match etiqueta(http, &tag_url, &req.api_key, TAG_SOLVER).await {
        Ok(id) => id,
        Err(e) => {
            log.line(format!("Prowlarr → etiqueta {TAG_SOLVER}: {e}"));
            return 1;
        }
    };

    let url = format!("{base}{}/api/v1/indexerproxy", prowlarr.route);
    let atuais = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(format!("Prowlarr → {}: {e}", solver.name));
            return 1;
        }
    };
    let body = json!({
        "name": solver.name,
        "implementation": "FlareSolverr",
        "implementationName": "FlareSolverr",
        "configContract": "FlareSolverrSettings",
        "fields": [
            field("host", json!(solver.url)),
            // o desafio leva alguns segundos; o padrão do Prowlarr é 60
            field("requestTimeout", json!(60)),
        ],
        "tags": [tag],
    });
    let existente = atuais
        .iter()
        .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(solver.name.as_str()))
        .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
    match send(http, &url, &req.api_key, existente, body).await {
        Ok(()) => {
            log.line(format!(
                "Prowlarr → {} [{TAG_SOLVER}]: {}",
                solver.name,
                if existente.is_some() { "atualizado" } else { "registrado" }
            ));
            0
        }
        Err(e) => {
            log.line(format!("Prowlarr → {}: {e}", solver.name));
            1
        }
    }
}

/// O id da etiqueta com este rótulo, criando-a se ainda não existir. O Prowlarr
/// guarda o rótulo em minúsculas, e é assim que ele é procurado.
async fn etiqueta(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    rotulo: &str,
) -> Result<i64, String> {
    let atuais = list(http, url, key).await?;
    if let Some(id) = atuais
        .iter()
        .find(|t| {
            t.get("label")
                .and_then(|l| l.as_str())
                .map(|l| l.eq_ignore_ascii_case(rotulo))
                .unwrap_or(false)
        })
        .and_then(|t| t.get("id").and_then(|i| i.as_i64()))
    {
        return Ok(id);
    }
    let r = http
        .post(url)
        .header("X-Api-Key", key)
        .header("Content-Type", "application/json")
        .body(json!({"label": rotulo}).to_string())
        .send()
        .await
        .map_err(|e| format!("não respondeu ({e})"))?;
    let st = r.status();
    let txt = r.text().await.unwrap_or_default();
    if !st.is_success() {
        return Err(erro(st, &txt));
    }
    serde_json::from_str::<Value>(&txt)
        .ok()
        .and_then(|v| v.get("id").and_then(|i| i.as_i64()))
        .ok_or_else(|| "o Prowlarr não devolveu o id da etiqueta".to_string())
}

/* As categorias dentro do próprio cliente de download.

   No qBittorrent elas já vão no `categories.json`, que o servidor escreve na
   pasta dele — nada a fazer aqui. No SABnzbd não existe arquivo equivalente
   que dê para mexer com ele no ar: as categorias moram no `sabnzbd.ini`, que
   ele reescreve, e a maneira sancionada de mexer é a API dele.

   Cada categoria ganha uma pasta com o nome dela dentro do diretório de
   downloads concluídos — mesma partição do que os *arr enxergam, que é o que
   preserva o hardlink na importação. */
async fn categorias_do_cliente(
    http: &reqwest::Client,
    req: &Req,
    client: &Client,
    log: &Log,
) -> usize {
    if client.kind != "sabnzbd" {
        return 0;
    }
    if client.web_url.is_empty() || client.api_key.is_empty() {
        log.line(format!(
            "{}: sem a API key dele não dá para criar as categorias — cole a chave no modal do serviço",
            client.name
        ));
        return 1;
    }
    let mut falhas = 0;
    let mut todas = client.categorias();
    // a do Prowlarr não vem do `cats` da página: ela é daqui
    if req.prowlarr.is_some() && !todas.iter().any(|c| c == CAT_PROWLARR) {
        todas.push(CAT_PROWLARR.into());
    }
    for cat in todas {
        let url = format!(
            "{}api?mode=set_config&section=categories&keyword={cat}&dir={cat}&output=json&apikey={}",
            client.web_url, client.api_key
        );
        match http.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                log.line(format!("{} → categoria {cat}: pronta", client.name))
            }
            Ok(r) => {
                log.line(format!("{} → categoria {cat}: HTTP {}", client.name, r.status().as_u16()));
                falhas += 1;
            }
            Err(e) => {
                log.line(format!("{} → categoria {cat}: não respondeu ({e})", client.name));
                falhas += 1;
            }
        }
    }
    falhas
}

/// O Prowlarr sincronizando cada *arr marcado na Configuração. Vale a mesma
/// regra dos clientes: procura pelo nome, atualiza no lugar, e o que falha
/// vira linha no log em vez de derrubar o resto.
async fn applications(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    prowlarr: &Prowlarr,
    log: &Log,
) -> usize {
    let url = format!("{base}{}/api/v1/applications", prowlarr.route);
    let atuais = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(format!("Prowlarr: {e}"));
            return req.arrs.iter().filter(|a| a.sync).count();
        }
    };
    let mut falhas = 0;
    for arr in req.arrs.iter().filter(|a| a.sync) {
        let body = match app_body(arr, &prowlarr.url, &req.api_key) {
            Ok(b) => b,
            Err(e) => {
                log.line(format!("Prowlarr → {}: {e}", arr.name));
                falhas += 1;
                continue;
            }
        };
        let existente = atuais
            .iter()
            .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(arr.name.as_str()))
            .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
        match send(http, &url, &req.api_key, existente, body).await {
            Ok(()) => log.line(format!(
                "Prowlarr → {}: {}",
                arr.name,
                if existente.is_some() {
                    "atualizado"
                } else {
                    "registrado"
                }
            )),
            Err(e) => {
                log.line(format!("Prowlarr → {}: {e}", arr.name));
                falhas += 1;
            }
        }
    }
    falhas
}

/* ---------- Media Management ---------- */

/* As opções da página, com o nome que cada uma tem na API. `naming` e
   `mediamanagement` são recursos únicos e cheios de campo que a página não
   mostra, então nada é montado do zero aqui: o recurso é lido, só estas
   chaves são trocadas, e o resto volta como estava.

   O que a página chama de `rename` é o "renomear ao importar" de cada família;
   o `useExisting` do Lidarr não entra, é a caixa que mostra ou esconde os
   formatos na interface, e o que ela descreve — importar com o nome que o
   arquivo já tinha — é o próprio `renameTracks` desligado. */
fn naming_map(family: &str) -> &'static [(&'static str, &'static str)] {
    match family {
        "sonarr" => &[
            ("rename", "renameEpisodes"),
            ("illegal", "replaceIllegalCharacters"),
            ("colon", "colonReplacementFormat"),
            ("multiEp", "multiEpisodeStyle"),
            ("standardEp", "standardEpisodeFormat"),
            ("dailyEp", "dailyEpisodeFormat"),
            ("animeEp", "animeEpisodeFormat"),
            ("seriesFolder", "seriesFolderFormat"),
            ("seasonFolder", "seasonFolderFormat"),
            ("specialsFolder", "specialsFolderFormat"),
        ],
        "radarr" => &[
            ("rename", "renameMovies"),
            ("illegal", "replaceIllegalCharacters"),
            ("colon", "colonReplacementFormat"),
            ("standardMovie", "standardMovieFormat"),
            ("movieFolder", "movieFolderFormat"),
        ],
        "lidarr" => &[
            ("rename", "renameTracks"),
            ("illegal", "replaceIllegalCharacters"),
            ("standardTrack", "standardTrackFormat"),
            ("multiDiscTrack", "multiDiscTrackFormat"),
            ("artistFolder", "artistFolderFormat"),
            ("albumFolder", "albumFolderFormat"),
        ],
        _ => &[],
    }
}

const MEDIA_MANAGEMENT: &[(&str, &str)] = &[
    ("hardlink", "copyUsingHardlinks"),
    ("perms", "setPermissionsLinux"),
    ("chmod", "chmodFolder"),
    ("chown", "chownGroup"),
    ("empty", "deleteEmptyFolders"),
    // o bloco avançado da Configuração: mesmos nomes nas três famílias
    ("rescan", "rescanAfterRefresh"),
    // o valor deste muda por família — quem escolhe as opções é a página
    ("fileDate", "fileDate"),
    ("recycleBin", "recycleBin"),
    ("recycleDays", "recycleBinCleanupDays"),
    ("extraFiles", "importExtraFiles"),
    ("extraExts", "extraFileExtensions"),
    ("skipFree", "skipFreeSpaceCheckWhenImporting"),
    ("minFree", "minimumFreeSpaceWhenImporting"),
];

/// Campos que a interface digita como texto e a API quer como número.
const NUMERICOS: &[&str] = &["recycleBinCleanupDays", "minimumFreeSpaceWhenImporting"];

/* Os dois campos de lista da nomenclatura viajam pelo nome e chegam à API
   como número: a ordem aqui é a do enum do *arr, e é a mesma do `COLON` e do
   `MULTIEP` da página. Nome que não estiver na lista vira erro, não zero —
   se as duas pontas saírem de sincronia, é melhor uma linha no log do que o
   app configurado com a primeira opção sem ninguém perceber. */
const COLON: &[&str] = &["delete", "dash", "spaceDash", "spaceDashSpace", "smart"];
const MULTI_EP: &[&str] = &["extend", "duplicate", "repeat", "scene", "range", "prefixedRange"];

fn enum_value(campo: &str, v: &Value) -> Result<Value, String> {
    if NUMERICOS.contains(&campo) {
        // campo de texto na página, número na API: vazio vira 0, e o que não
        // for número vira erro em vez de virar zero calado
        let txt = match v {
            Value::String(t) => t.trim().to_string(),
            outro => return Ok(outro.clone()),
        };
        if txt.is_empty() {
            return Ok(json!(0));
        }
        return txt
            .parse::<i64>()
            .map(|n| json!(n))
            .map_err(|_| format!("{campo}: {txt} não é um número"));
    }
    let lista = match campo {
        "colonReplacementFormat" => COLON,
        "multiEpisodeStyle" => MULTI_EP,
        _ => return Ok(v.clone()),
    };
    let nome = v.as_str().unwrap_or("");
    lista
        .iter()
        .position(|x| *x == nome)
        .map(|i| json!(i))
        .ok_or_else(|| format!("{campo}: opção desconhecida ({nome})"))
}

/// Troca no recurso lido só o que a página governa, deixando o resto intacto.
fn merge(atual: &mut Value, de: &Map<String, Value>, mapa: &[(&str, &str)]) -> Result<(), String> {
    for (pagina, api) in mapa {
        if let Some(v) = de.get(*pagina) {
            atual[*api] = enum_value(api, v)?;
        }
    }
    Ok(())
}

/// O Media Management e a nomenclatura de uma instância. São dois recursos
/// únicos (sem id na rota, mas com id no corpo), então cada um é lido, mexido
/// e devolvido inteiro.
async fn media_management(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    arr: &Arr,
    log: &Log,
) -> usize {
    let Some(mm) = req.mm.get(&arr.family).and_then(|v| v.as_object()) else {
        return 0;
    };
    let naming = mm.get("naming").and_then(|v| v.as_object()).cloned();
    /* O bloco avançado viaja dentro do `naming` — é a tabela que guarda JSON
       livre —, mas é campo do `mediamanagement`: sai de lá e entra aqui. */
    let mut campos_mm = mm.clone();
    if let Some(adv) = naming
        .as_ref()
        .and_then(|n| n.get("adv"))
        .and_then(|v| v.as_object())
    {
        for (k, v) in adv {
            campos_mm.insert(k.clone(), v.clone());
        }
    }
    // o "renomear" fica no `mm` da página, mas na API ele é campo da
    // nomenclatura: entra junto com ela
    let mut campos_naming = naming.unwrap_or_default();
    if let Some(r) = mm.get("rename") {
        campos_naming.insert("rename".into(), r.clone());
    }
    // o que a Configuração tirou desta instância não vai como chave nenhuma
    for k in &arr.skip_naming {
        campos_naming.remove(k);
    }

    let mut falhas = 0;
    for (recurso, campos, mapa) in [
        (
            "naming",
            campos_naming,
            naming_map(&arr.family).to_vec(),
        ),
        (
            "mediamanagement",
            campos_mm,
            MEDIA_MANAGEMENT.to_vec(),
        ),
    ] {
        let url = format!("{base}{}/api/{}/config/{recurso}", arr.route, arr.api);
        match put_config(http, &url, &req.api_key, &campos, &mapa).await {
            Ok(()) => log.line(format!("{} → {recurso}: aplicado", arr.name)),
            Err(e) => {
                log.line(format!("{} → {recurso}: {e}", arr.name));
                falhas += 1;
            }
        }
    }
    falhas
}

async fn put_config(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    campos: &Map<String, Value>,
    mapa: &[(&str, &str)],
) -> Result<(), String> {
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
    let mut atual: Value =
        serde_json::from_str(&txt).map_err(|_| "resposta não foi a configuração".to_string())?;
    merge(&mut atual, campos, mapa)?;

    let r = http
        .put(url)
        .header("X-Api-Key", key)
        .header("Content-Type", "application/json")
        .body(atual.to_string())
        .send()
        .await
        .map_err(|e| format!("não respondeu ({e})"))?;
    let st = r.status();
    if st.is_success() {
        return Ok(());
    }
    Err(erro(st, &r.text().await.unwrap_or_default()))
}

/* Aplicado logo depois do `up`, nenhum app respondeu ainda: eles levam de
   alguns segundos a um minuto para abrir a API. O `/ping` não pede chave e é
   o primeiro caminho que eles servem, então é por ele que se pergunta.

   Quem não responder a tempo não interrompe nada — segue como estava, e o erro
   dele aparece linha a linha no registro de cada ligação. */
async fn esperar_apps(http: &reqwest::Client, base: &str, alvos: &[Arr], log: &Log) {
    for alvo in alvos {
        esperar_url(http, &format!("{base}{}/ping", alvo.route), &alvo.name, log).await;
    }
}

/* O mesmo para os clientes de download. Vale menos pelo primeiro `up` e mais
   pelo que vem antes desta volta: escrever a conf do qBittorrent **reinicia**
   o container dele, então quando os *arr forem registrá-lo ele está subindo há
   segundos — e o teste de conexão que eles fazem ao salvar falha.

   Qualquer resposta serve, inclusive 401 e 403: o que se quer saber é se há
   alguém escutando, não se a credencial está certa. As do próprio nginx é que
   não servem — ele responde 502 enquanto o container atrás dele não subiu, e
   tomar isso por "pronto" é o mesmo que não esperar. */
async fn esperar_clientes(http: &reqwest::Client, clients: &[Client], log: &Log) {
    for c in clients {
        if c.web_url.is_empty() {
            continue;
        }
        esperar_url(http, &c.web_url, &c.name, log).await;
    }
}

async fn esperar_url(http: &reqwest::Client, url: &str, nome: &str, log: &Log) {
    let mut avisou = false;
    for _ in 0..45 {
        match http.get(url).send().await {
            // 5xx aqui é o nginx dizendo que o de trás ainda não subiu
            Ok(r) if !r.status().is_server_error() => break,
            _ => {
                if !avisou {
                    log.line(format!("esperando o {nome} responder…"));
                    avisou = true;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Os campos que o app diz que um recurso tem, com os valores de fábrica dele.
/// Falha aqui não é erro: quem não serve o schema recebe só os nossos campos.
async fn schema_de(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    implementation: &str,
) -> Option<Vec<Value>> {
    let itens = list(http, &format!("{url}/schema"), key).await.ok()?;
    itens
        .iter()
        .find(|i| i.get("implementation").and_then(|v| v.as_str()) == Some(implementation))
        .and_then(|i| i.get("fields"))
        .and_then(|f| f.as_array())
        .cloned()
}

/// A implementação de cada cliente, que é como o schema o identifica.
fn implementacao(kind: &str) -> &'static str {
    match kind {
        "sabnzbd" => "Sabnzbd",
        _ => "QBittorrent",
    }
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
            family: key.split('-').next().unwrap().into(),
            internal_url: format!("http://{key}:8989/{key}"),
            sync: true,
            skip_naming: Vec::new(),
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
            api_key: "qbt_chave".into(),
            web_url: "http://127.0.0.1/qbittorrent/".into(),
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
        let b = qbit().body(&arr("sonarr", "tvCategory"), None).unwrap();
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
    fn o_qbittorrent_entra_pela_api_key_quando_o_app_a_conhece() {
        let c = qbit();
        let a = arr("sonarr", "tvCategory");
        // o app que tem o campo recebe a chave, e não a senha
        let com = vec![json!({"name":"apiKey","value":null}),
                       json!({"name":"username","value":null}),
                       json!({"name":"password","value":null}),
                       json!({"name":"host","value":"localhost"})];
        let b = c.body(&a, Some(&com)).unwrap();
        assert_eq!(val(&b, "apiKey"), "qbt_chave");
        assert_eq!(val(&b, "username"), Value::Null);
        assert_eq!(val(&b, "password"), Value::Null);

        // o que não conhece o campo continua pelo usuário e senha
        let sem = vec![json!({"name":"username","value":null}),
                       json!({"name":"password","value":null})];
        let b = c.body(&a, Some(&sem)).unwrap();
        assert_eq!(val(&b, "username"), "admin");
        assert_eq!(val(&b, "password"), "senha");
    }

    #[test]
    fn o_sabnzbd_troca_usuario_e_senha_pela_chave_da_api() {
        let mut c = qbit();
        c.kind = "sabnzbd".into();
        c.name = "SABnzbd".into();
        c.api_key = "chave".into();
        let b = c.body(&arr("radarr", "movieCategory"), None).unwrap();
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
        assert_eq!(val(&c.body(&arr("radarr", "movieCategory"), None).unwrap(), "movieCategory"), "");
    }

    #[test]
    fn cliente_de_tipo_desconhecido_para_naquele_par_e_nao_no_meio_do_corpo() {
        let mut c = qbit();
        c.kind = "transmission".into();
        assert!(c.body(&arr("sonarr", "tvCategory"), None).is_err());
    }

    #[test]
    fn a_aplicacao_do_prowlarr_sai_com_os_dois_enderecos_internos() {
        let a = arr("sonarr-anime", "tvCategory");
        let b = app_body(&a, "http://prowlarr:9696/prowlarr", "chave").unwrap();
        assert_eq!(b["implementation"], "Sonarr");
        assert_eq!(b["configContract"], "SonarrSettings");
        assert_eq!(b["syncLevel"], "fullSync");
        // o nome é o da instância, que é o que distingue duas do mesmo app
        assert_eq!(b["name"], "sonarr-anime");
        assert_eq!(val(&b, "prowlarrUrl"), "http://prowlarr:9696/prowlarr");
        assert_eq!(val(&b, "baseUrl"), "http://sonarr-anime:8989/sonarr-anime");
        assert_eq!(val(&b, "apiKey"), "chave");
        // sincronizar categoria nenhuma é um Prowlarr que não sincroniza nada
        assert!(!val(&b, "syncCategories").as_array().unwrap().is_empty());
    }

    #[test]
    fn sem_endereco_interno_a_aplicacao_para_antes_de_ir_a_rede() {
        let mut a = arr("radarr", "movieCategory");
        a.internal_url = String::new();
        assert!(app_body(&a, "http://prowlarr:9696", "chave").is_err());
    }

    #[test]
    fn cada_familia_tem_as_categorias_dela_e_nao_as_da_outra() {
        assert!(sync_categories("sonarr").iter().all(|c| (5000..6000).contains(c)));
        assert!(sync_categories("radarr").iter().all(|c| (2000..3000).contains(c)));
        assert!(sync_categories("lidarr").iter().all(|c| (3000..4000).contains(c)));
        assert!(sync_categories("bazarr").is_empty());
    }

    #[test]
    fn o_merge_troca_so_o_que_a_pagina_governa() {
        let mut atual = json!({"id": 1, "renameEpisodes": false,
                               "standardEpisodeFormat": "velho",
                               "campoQueANaoConheco": "fica"});
        let de: Map<String, Value> = serde_json::from_str(
            r#"{"rename": true, "standardEp": "novo", "colon": "smart"}"#,
        )
        .unwrap();
        merge(&mut atual, &de, naming_map("sonarr")).unwrap();
        assert_eq!(atual["renameEpisodes"], true);
        assert_eq!(atual["standardEpisodeFormat"], "novo");
        // dois-pontos vai como número, na ordem do enum do app
        assert_eq!(atual["colonReplacementFormat"], 4);
        // o id e o que a página não mostra voltam como estavam
        assert_eq!(atual["id"], 1);
        assert_eq!(atual["campoQueANaoConheco"], "fica");
        // campo que a página não mandou não é inventado
        assert!(atual.get("animeEpisodeFormat").is_none());
    }

    #[test]
    fn o_bloco_avancado_vira_campo_do_mediamanagement() {
        let mut atual = json!({"id": 1, "recycleBin": "", "autoRenameFolders": true});
        let de: Map<String, Value> = serde_json::from_str(
            r#"{"rescan":"never","fileDate":"localAirDate",
                "recycleBin":"/mnt/media/.lixeira","recycleDays":"14",
                "extraFiles":true,"extraExts":"srt,sub","skipFree":false,"minFree":"250"}"#,
        )
        .unwrap();
        merge(&mut atual, &de, MEDIA_MANAGEMENT).unwrap();
        assert_eq!(atual["rescanAfterRefresh"], "never");
        assert_eq!(atual["fileDate"], "localAirDate");
        assert_eq!(atual["recycleBin"], "/mnt/media/.lixeira");
        assert_eq!(atual["importExtraFiles"], true);
        assert_eq!(atual["extraFileExtensions"], "srt,sub");
        assert_eq!(atual["skipFreeSpaceCheckWhenImporting"], false);
        // texto na página, número na API
        assert_eq!(atual["recycleBinCleanupDays"], 14);
        assert_eq!(atual["minimumFreeSpaceWhenImporting"], 250);
        // e o que é do app continua lá
        assert_eq!(atual["autoRenameFolders"], true);
    }

    #[test]
    fn numero_vazio_vira_zero_e_texto_torto_vira_erro() {
        assert_eq!(enum_value("recycleBinCleanupDays", &json!("")).unwrap(), json!(0));
        assert_eq!(enum_value("minimumFreeSpaceWhenImporting", &json!(" 100 ")).unwrap(), json!(100));
        assert!(enum_value("recycleBinCleanupDays", &json!("uma semana")).is_err());
        // número que já veio número passa direto
        assert_eq!(enum_value("minimumFreeSpaceWhenImporting", &json!(7)).unwrap(), json!(7));
    }

    #[test]
    fn opcao_de_lista_fora_do_enum_falha_em_vez_de_virar_a_primeira() {
        assert_eq!(
            enum_value("multiEpisodeStyle", &json!("prefixedRange")).unwrap(),
            json!(5)
        );
        assert!(enum_value("colonReplacementFormat", &json!("outra")).is_err());
        // campo que não é de lista passa como veio
        assert_eq!(
            enum_value("standardEpisodeFormat", &json!("{Series Title}")).unwrap(),
            json!("{Series Title}")
        );
    }

    #[test]
    fn a_instancia_de_fora_do_formato_nao_recebe_aquela_chave() {
        // o merge só troca o que vem; sem a chave, o app mantém o formato dele,
        // que é o que o campo obrigatório do Sonarr exige
        let mut atual = json!({"id": 1, "animeEpisodeFormat": "o do app",
                               "standardEpisodeFormat": "velho"});
        let mut de: Map<String, Value> = serde_json::from_str(
            r#"{"standardEp": "novo", "animeEp": "nosso"}"#,
        )
        .unwrap();
        let a = Arr { skip_naming: vec!["animeEp".into()], ..arr("sonarr", "tvCategory") };
        for k in &a.skip_naming {
            de.remove(k);
        }
        merge(&mut atual, &de, naming_map("sonarr")).unwrap();
        assert_eq!(atual["standardEpisodeFormat"], "novo");
        assert_eq!(atual["animeEpisodeFormat"], "o do app");
    }

    #[test]
    fn cada_familia_renomeia_com_o_nome_que_a_api_dela_usa() {
        let rename = |f| {
            naming_map(f)
                .iter()
                .find(|(p, _)| *p == "rename")
                .map(|(_, a)| *a)
        };
        assert_eq!(rename("sonarr"), Some("renameEpisodes"));
        assert_eq!(rename("radarr"), Some("renameMovies"));
        assert_eq!(rename("lidarr"), Some("renameTracks"));
        // o Lidarr não tem dois-pontos, e a página também não o oferece lá
        assert!(naming_map("lidarr").iter().all(|(p, _)| *p != "colon"));
    }

    #[test]
    fn no_prowlarr_o_cliente_entra_uma_vez_so_na_categoria_dele() {
        let c = qbit();
        // um registro por cliente, com o nome dele e a categoria do Prowlarr —
        // o que ele pega é avulso, não é de instância nenhuma
        let b = c.body_como(&c.name, "category", CAT_PROWLARR, None).unwrap();
        assert_eq!(b["name"], "qBittorrent");
        assert_eq!(val(&b, "category"), "prowlarr");
        // e o resto do cliente é o mesmo que vai para os *arr
        assert_eq!(b["implementation"], "QBittorrent");
        assert_eq!(val(&b, "host"), "gluetun");
        assert_eq!(val(&b, "username"), "admin");
    }

    #[test]
    fn as_categorias_do_cliente_saem_sem_repetir_e_sem_vazio() {
        let mut c = qbit();
        c.cats = serde_json::from_str(
            r#"{"sonarr":"tv-sonarr","sonarr-anime":"tv-sonarr","radarr":"radarr","lidarr":"  "}"#,
        )
        .unwrap();
        assert_eq!(c.categorias(), vec!["radarr", "tv-sonarr"]);
    }

    #[test]
    fn o_prowlarr_sozinho_com_o_resolvedor_ja_e_trabalho() {
        // stack de Prowlarr + FlareSolverr, sem *arr nenhum: há o que aplicar,
        // porque o proxy de indexador é do Prowlarr, não dos *arr
        let mut req = Req {
            base: "http://127.0.0.1".into(),
            api_key: "k".into(),
            arrs: vec![],
            clients: vec![],
            prowlarr: Some(Prowlarr { route: "/prowlarr".into(), url: "http://p:9696".into() }),
            solver: Some(Solver { name: "FlareSolverr".into(), url: "http://f:8191".into() }),
            mm: Map::new(),
            configarr: false,
        };
        assert!(req.tem_o_que_fazer());
        // sem o resolvedor e sem cliente, o Prowlarr sozinho não tem o que fazer
        req.solver = None;
        assert!(!req.tem_o_que_fazer());
    }

    #[test]
    fn stack_sem_arr_nao_tem_o_que_aplicar() {
        let req = |arrs: Vec<Arr>, clients: Vec<Client>| Req {
            base: "http://127.0.0.1".into(),
            api_key: "k".into(),
            arrs,
            clients,
            prowlarr: None,
            solver: None,
            mm: Map::new(),
            configarr: false,
        };
        assert!(!req(vec![], vec![qbit()]).tem_o_que_fazer());
        assert!(!req(vec![arr("sonarr", "tvCategory")], vec![]).tem_o_que_fazer());
        assert!(req(vec![arr("sonarr", "tvCategory")], vec![qbit()]).tem_o_que_fazer());
    }

    /// Só o Configarr na stack já é motivo para a volta acontecer: os perfis não
    /// dependem de cliente de download nem de Prowlarr.
    #[test]
    fn so_o_configarr_ja_e_motivo_para_rodar() {
        let mut r = Req {
            base: "http://127.0.0.1".into(),
            api_key: "k".into(),
            arrs: vec![arr("sonarr", "tvCategory")],
            clients: vec![],
            prowlarr: None,
            solver: None,
            mm: Map::new(),
            configarr: true,
        };
        assert!(!r.tem_o_que_fazer());
        assert!(r.quer_configarr());
        // sem *arr não há em que aplicar perfil, mesmo com o Configarr na stack
        r.arrs = vec![];
        assert!(!r.quer_configarr());
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

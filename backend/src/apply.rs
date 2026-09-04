/* Applies the Configuration to the stack that is already up (v0.3).

   Until now the server only wrote files and called docker: whatever the page
   built, it wrote. A download client is not a file — the *arr keeps that in its
   own database and only takes it through the API — so this is the first thing
   the server builds: the JSON body of each `downloadclient`.

   What it does *not* do is decide. Address, port, category, who receives what,
   the name of each family's category field: all of that arrives ready from the
   page, which is where `SERVICES` and `CONFIG` live. Only the *arr API format
   lives here — implementation, contract and the list of `fields`.

   The apps are reached through nginx, not by container: the server runs on the
   host and the `starrnet` network does not exist for it, but nginx publishes a
   port and serves each app on its subpath. It is the page that sends the
   address in `base`, because it is the one that knows whether the stack came up
   with TLS and on which ports.

   Applying again does not duplicate: each client is looked up by name in the
   *arr's list, and whatever is already there is updated in place. */

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::jobs::Log;
use crate::msg;
use crate::msg::Msg;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Req {
    /// how the server reaches the stack's nginx, e.g. `http://127.0.0.1:8080`
    base: String,
    /// the `.env`'s `STARR_APIKEY`, which is the same in every *arr
    api_key: String,
    arrs: Vec<Arr>,
    clients: Vec<Client>,
    /// the stack's Prowlarr, when it is in it
    #[serde(default)]
    prowlarr: Option<Prowlarr>,
    /// the Cloudflare challenge solver, when it is in the stack
    #[serde(default)]
    solver: Option<Solver>,
    /// the stack's Jellyfin: the initial wizard and the libraries
    #[serde(default)]
    jellyfin: Option<Jellyfin>,
    /// Media Management and naming, per family — as on the page, it is per app
    /// and not per instance
    #[serde(default)]
    mm: Map<String, Value>,
    /// what running Configarr needs: paths, network and user. The profiles do not
    /// come through here — they are in the `config.yml` the page generated — but it
    /// is this `apply` that has already waited for the apps to answer, and it runs
    /// after it. Absent or null: there is no profile to apply.
    #[serde(default)]
    configarr: Option<crate::deploy::Configarr>,
    /// the search language (v0.7). Already resolved into what each app expects,
    /// because deciding is the page's job. `None` means "do not touch any app's
    /// language", which is what a stack that never asked for one keeps.
    #[serde(default)]
    search_lang: Option<SearchLang>,
    /// the stack's Bazarr instances that carry an API key of their own, for the
    /// subtitle half of the search language
    #[serde(default)]
    bazarr: Option<Vec<Bazarr>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchLang {
    /// the page's own tag (`pt-BR`), for the log
    code: String,
    /// the language **name** as Radarr publishes it in `/api/v3/language`; the
    /// number is looked up in the app itself, like every other option name.
    /// Empty means the app does not know that language.
    #[serde(default)]
    arr: String,
    /// Bazarr's two-letter code. Jellyfin's is not here: it reaches the server
    /// inside the `jellyfin` object, which is the only thing that speaks its API.
    #[serde(default)]
    bz2: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Bazarr {
    /// the instance title, for the log
    name: String,
    /// how the server reaches it: through nginx, on its subpath
    url: String,
    /// the key the app generated itself, typed into its modal
    api_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Jellyfin {
    /// the server name, which is the instance title
    name: String,
    /// how the server reaches it: through nginx, on its subpath
    url: String,
    /// the administrator to create in the wizard; empty means "do not touch it"
    #[serde(default)]
    user: String,
    #[serde(default)]
    pass: String,
    /// the interface language, in .NET form (`pt-BR`, `en-US`)
    #[serde(default)]
    culture: String,
    /// the language the **metadata** is fetched in, which since v0.7 is a
    /// separate question from the interface: it comes from the Environment's
    /// search language. Empty falls back to `culture`, which is what the stack
    /// of whoever picked no language always did.
    #[serde(default)]
    meta_lang: Option<String>,
    libs: Vec<JfLib>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JfLib {
    name: String,
    /// `tvshows`, `movies`, `music` — empty is a mixed library
    #[serde(default)]
    #[serde(rename = "type")]
    kind: String,
    /// the path **from inside the container**, which is what the app sees
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Solver {
    /// the instance title, which is the name of the registration in Prowlarr
    name: String,
    /// how Prowlarr reaches it on the stack network: `http://flaresolverr:8191`
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Prowlarr {
    /// subpath where nginx serves it, through which the server talks to it
    route: String,
    /// how the *arr sees Prowlarr inside the stack network, base URL included:
    /// `http://prowlarr:9696/prowlarr`
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Arr {
    /// `container_name`, which is the instance key on the page
    key: String,
    name: String,
    /// subpath where nginx serves it, e.g. `/sonarr`
    route: String,
    /// its API version: `v3` in Sonarr and Radarr, `v1` in Lidarr
    api: String,
    /// the family's category field name: `tvCategory`, `movieCategory`, …
    cat_field: String,
    /// the family — `sonarr`, `radarr` or `lidarr` — which in Prowlarr picks the
    /// implementation and the synced categories
    family: String,
    /// how Prowlarr reaches it inside the stack network, base URL included:
    /// `http://sonarr:8989/sonarr`
    #[serde(default)]
    internal_url: String,
    /// should Prowlarr sync with it? it is the Configuration checkbox
    #[serde(default)]
    sync: bool,
    /// naming fields this instance does not receive: the page left them out, and
    /// not sending the key is what makes the app keep its own
    #[serde(default)]
    skip_naming: Vec<String>,
    /// the app's root folders, as the container sees them — the page takes them
    /// from the binds it built in the compose itself
    #[serde(default)]
    root_folders: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    name: String,
    /// `qbittorrent` or `sabnzbd` — it is what picks implementation and contract
    kind: String,
    /// address on the stack network, from the *arr's point of view (gluetun,
    /// when the client routes through the VPN)
    host: String,
    port: u16,
    #[serde(default)]
    user: String,
    #[serde(default)]
    pass: String,
    /// SABnzbd's API key; qBittorrent does not use it here
    #[serde(default)]
    api_key: String,
    /// how the server reaches its interface: through nginx, or straight at the
    /// published port when it has no route. This is where SABnzbd's categories
    /// are created, and it is what the wait asks
    #[serde(default)]
    web_url: String,
    /// category per *arr instance, by its key
    #[serde(default)]
    cats: Map<String, Value>,
    /// remove from the queue what completed and what failed; absent = do not touch
    #[serde(default)]
    cdh: Option<Cdh>,
    /// what to send to qBittorrent's `app/setPreferences`, ready: the page is the
    /// one that decides what goes here, and the server only delivers it
    #[serde(default)]
    prefs: Option<Map<String, Value>>,
    /// qBittorrent's categories with each one's folder, the way the page builds
    /// them — the path is the one inside the container
    #[serde(default)]
    categories: Vec<Category>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Category {
    name: String,
    #[serde(default)]
    save_path: String,
}

#[derive(Deserialize)]
struct Cdh {
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    failed: bool,
}

impl Req {
    /// Is there any link to make? Deploy asks before calling: in a stack with
    /// nothing to link, not applying is no error at all.
    ///
    /// There are two independent halves. The *arr apps need something to receive —
    /// a client, Media Management or the Prowlarr registration. And Prowlarr has
    /// work of its own: its clients and the challenge solver, which count even in a
    /// stack where there is no *arr yet.
    pub fn has_work(&self) -> bool {
        let by_arrs = !self.arrs.is_empty()
            && (!self.clients.is_empty() || self.prowlarr.is_some() || !self.mm.is_empty());
        let by_prowlarr = self.prowlarr.is_some()
            && (!self.clients.is_empty() || self.solver.is_some() || !self.arrs.is_empty());
        // Jellyfin counts on its own: a stack with only it still has the wizard and
        // the libraries to do
        let by_jellyfin = self.jellyfin.is_some();
        /* And the download client too, as long as it has a setting of its own to
           receive: qBittorrent's preferences do not depend on any *arr, and a
           stack with qBittorrent alone was left without them — with no error and
           no line in the log, which is the worst way of doing nothing. */
        let by_clients = self
            .clients
            .iter()
            .any(|c| c.prefs.as_ref().is_some_and(|p| !p.is_empty()));
        /* And Bazarr on its own, for the same reason: a stack of subtitles and a
           search language has work to do, with no *arr in sight. */
        let by_bazarr = self.search_lang.is_some()
            && self.bazarr.as_ref().is_some_and(|l| !l.is_empty());
        by_arrs || by_prowlarr || by_jellyfin || by_clients || by_bazarr
    }

    /// Run Configarr afterwards? It does not depend on `has_work()`: a stack with
    /// no download client and no Prowlarr may still want the profiles.
    pub fn configarr(&self) -> Option<crate::deploy::Configarr> {
        if self.arrs.is_empty() {
            return None;
        }
        self.configarr.clone()
    }
}

/// Prowlarr as a target: the same API as the *arr apps, version v1, and its
/// category field is not per family — it is a single `category`.
fn prowlarr_target(p: &Prowlarr) -> Arr {
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
        root_folders: Vec::new(),
    }
}

impl Arr {
    /// A copy for the download-client round, which does not use the rest.
    fn clone_target(&self) -> Arr {
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
            root_folders: self.root_folders.clone(),
        }
    }
}

fn field(name: &str, value: Value) -> Value {
    json!({"name": name, "value": value})
}

impl Client {
    /// The category with which one *arr instance uses this client.
    fn cat(&self, key: &str) -> String {
        self.cats
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Every category this client receives, without repeats — it is what has to
    /// exist inside it.
    fn categories(&self) -> Vec<String> {
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

    /// The `downloadclient` resource as the app expects it. What changes from one
    /// client to the other is the implementation, the protocol and the credential
    /// fields; the rest is the same in all of them.
    ///
    /// `name`, `cat_field` and `cat` come from outside because the same client is
    /// registered in two ways: once in each *arr, with its category, and once per
    /// instance in Prowlarr, where the field is called `category` and the name has
    /// to tell the registrations apart.
    fn body_as(
        &self,
        name: &str,
        cat_field: &str,
        cat: &str,
        schema: Option<&Vec<Value>>,
    ) -> Result<Value, Msg> {
        let mut ours: Vec<(String, Value)> = vec![
            ("host".into(), json!(self.host)),
            ("port".into(), json!(self.port)),
            ("useSsl".into(), json!(false)),
            ("urlBase".into(), json!("")),
            (cat_field.into(), json!(cat)),
        ];
        let (implementation, contract, protocol) = match self.kind.as_str() {
            "qbittorrent" => {
                /* By API key, not by password: it does not expire when the interface
                   password changes and it is what qBittorrent's own conf
                   receives. User and password only go to an app that does not
                   know the field — and it is its schema that says so. */
                let tem_api_key = schema
                    .map(|f| f.iter().any(|x| x["name"] == "apiKey"))
                    .unwrap_or(false);
                if tem_api_key {
                    ours.push(("apiKey".into(), json!(self.api_key)));
                } else {
                    ours.push(("username".into(), json!(self.user)));
                    ours.push(("password".into(), json!(self.pass)));
                }
                ("QBittorrent", "QBittorrentSettings", "torrent")
            }
            "sabnzbd" => {
                ours.push(("apiKey".into(), json!(self.api_key)));
                ("Sabnzbd", "SabnzbdSettings", "usenet")
            }
            other => return Err(msg!("job.apply.unknownClient", other)),
        };

        /* The fields come from the schema the app publishes, with ours on top.
           Sending only ours leaves the rest null, and the app blows up when
           testing the connection — Prowlarr answers an "Object reference not
           set" that says nothing. With no schema (an app that does not serve
           one), only ours go. */
        let mut fields: Vec<Value> = match schema {
            Some(base) => base
                .iter()
                .map(|f| {
                    let name = f.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let value = ours
                        .iter()
                        .find(|(k, _)| k == name)
                        .map(|(_, v)| v.clone())
                        .or_else(|| f.get("value").cloned())
                        .unwrap_or(Value::Null);
                    field(name, value)
                })
                .collect(),
            None => Vec::new(),
        };
        // what is ours and the schema did not have lands at the end
        for (k, v) in &ours {
            if !fields.iter().any(|f| f["name"] == k.as_str()) {
                fields.push(field(k, v.clone()));
            }
        }
        Ok(json!({
            "name": name,
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

    /// This client's registration in an *arr: the name is the client's, and the
    /// category is the one the Configuration gave that instance.
    fn body(&self, arr: &Arr, schema: Option<&Vec<Value>>) -> Result<Value, Msg> {
        self.body_as(&self.name, &arr.cat_field, &self.cat(&arr.key), schema)
    }
}

/// One pass per (*arr, client) pair. An app that is down does not bring the
/// others down: its error becomes a line in the log and the round goes on —
/// whoever applies this has usually just brought the stack up, and an app still
/// starting is no reason not to configure the rest.
/// Only waits for the apps to answer, applying nothing. It is the case of the
/// stack that has Configarr and nothing else to configure: the profiles need
/// the apps up as much as the download clients do, and without this nobody would have waited.
pub async fn wait(req: &Req, running: &[String], log: &Log) -> Result<(), Msg> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| msg!("job.apply.httpClientError", e.to_string()))?;
    let base = req.base.trim_end_matches('/').to_string();
    let targets: Vec<Arr> = req.arrs.iter().map(Arr::clone_target).collect();
    wait_apps(&http, &base, &req.api_key, &targets, running, log).await;
    Ok(())
}

pub async fn download_clients(mut req: Req, running: Vec<String>, log: Log) -> Result<(), Msg> {
    if !req.has_work() {
        return Err(Msg::k("job.apply.nothingToDo"));
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        // the stack is on this very machine and its certificate is usually the one
        // the owner put there by hand: what matters here is reaching the app, not
        // proving who it is
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| msg!("job.apply.httpClientError", e.to_string()))?;

    let base = req.base.trim_end_matches('/').to_string();
    let mut failures = 0;

    let targets: Vec<Arr> = req.arrs.iter().map(Arr::clone_target).collect();

    let mut to_wait_for: Vec<Arr> = targets.iter().map(Arr::clone_target).collect();
    if let Some(p) = &req.prowlarr {
        to_wait_for.push(prowlarr_target(p));
    }
    wait_apps(&http, &base, &req.api_key, &to_wait_for, &running, &log).await;
    wait_clients(&http, &req.clients, &log).await;
    /* Before registering the client anywhere: the key that counts is the one the
       app has, not the one the page proposed. `patch.rs` does not overwrite an
       API key already in its conf, so the app rules here — and registering the
       *arr with ours would leave the two speaking different keys. */
    adopt_api_key(&http, &mut req, &log).await;

    // the categories the *arr apps and Prowlarr are going to ask for have to
    // exist inside the client *before* anyone registers it — the *arr's
    // connection test validates the category against the client and fails
    // with "category does not exist" otherwise
    for client in &req.clients {
        failures += client_categories(&http, &req, client, &log).await;
        failures += client_preferences(&http, client, &log).await;
    }

    for arr in &targets {
        let url = format!("{base}{}/api/{}/downloadclient", arr.route, arr.api);
        let current = match list(&http, &url, &req.api_key).await {
            Ok(v) => v,
            Err(e) => {
                log.line(msg!("job.apply.item", arr.name.clone(), e));
                failures += req.clients.len();
                continue;
            }
        };
        for client in &req.clients {
            let schema = schema_of(&http, &url, &req.api_key, implementation_of(&client.kind)).await;
            let body = match client.body(arr, schema.as_ref()) {
                Ok(b) => b,
                Err(e) => {
                    log.line(msg!("job.apply.link", arr.name.clone(), client.name.clone(), e));
                    failures += 1;
                    continue;
                }
            };
            let existing = current
                .iter()
                .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(client.name.as_str()))
                .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
            match send(&http, &url, &req.api_key, existing, body).await {
                Ok(()) => log.line(msg!(
                    "job.apply.link",
                    arr.name.clone(),
                    client.name.clone(),
                    Msg::k(if existing.is_some() { "job.apply.updated" } else { "job.apply.registered" })
                )),
                Err(e) => {
                    log.line(msg!("job.apply.link", arr.name.clone(), client.name.clone(), e));
                    failures += 1;
                }
            }
        }
    }
    if let Some(p) = &req.prowlarr {
        failures += prowlarr_clients(&http, &base, &req, p, &log).await;
        failures += applications(&http, &base, &req, p, &log).await;
        if let Some(sv) = &req.solver {
            failures += indexer_proxy(&http, &base, &req, p, sv, &log).await;
        }
    }
    for arr in &req.arrs {
        failures += ensure_root_folders(&http, &base, &req, arr, &log).await;
        failures += media_management(&http, &base, &req, arr, &log).await;
        failures += metadata_language(&http, &base, &req, arr, &log).await;
    }
    // Jellyfin is no *arr and does not speak their API: its own round, at the end
    if let Some(jf) = &req.jellyfin {
        failures += jellyfin(&http, jf, &log).await;
    }
    // Bazarr, likewise: its own API, its own round, and only when a language was picked
    if let (Some(lang), Some(list)) = (&req.search_lang, &req.bazarr) {
        for bz in list {
            failures += bazarr(&http, bz, lang, &log).await;
        }
    }
    if failures > 0 {
        return Err(msg!("job.apply.tooManyFailures", failures));
    }
    log.line(Msg::k("job.apply.done"));
    Ok(())
}

/* The newznab categories Prowlarr syncs with each family. They are its own
   defaults, and they go explicitly on purpose: the field accepts being empty,
   and an empty `syncCategories` is a Prowlarr that syncs no indexer at all — a
   silent failure, which is the worst kind here. Touching this is touching what
   the app considers "series", "movie" and "music". */
fn sync_categories(family: &str) -> Vec<u32> {
    match family {
        "sonarr" => vec![5000, 5010, 5020, 5030, 5040, 5045, 5050, 5090],
        "radarr" => vec![2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060, 2070, 2080, 2090],
        "lidarr" => vec![3000, 3010, 3020, 3030, 3040, 3050, 3060],
        _ => vec![],
    }
}

/// Prowlarr's `applications` resource: an *arr for it to sync. Both addresses
/// here are internal — container to container, on the stack network — not the
/// nginx ones: the one that will talk to the *arr is Prowlarr, not the
/// server.
fn app_body(arr: &Arr, prowlarr_url: &str, api_key: &str) -> Result<Value, Msg> {
    let (implementation, contract) = match arr.family.as_str() {
        "sonarr" => ("Sonarr", "SonarrSettings"),
        "radarr" => ("Radarr", "RadarrSettings"),
        "lidarr" => ("Lidarr", "LidarrSettings"),
        other => return Err(msg!("job.apply.unknownProwlarrFamily", other)),
    };
    if arr.internal_url.is_empty() {
        return Err(Msg::k("job.apply.noInternalUrl"));
    }
    Ok(json!({
        "name": arr.name,
        // Prowlarr sends the indexers and also removes what left; `addOnly`
        // would leave rubbish behind on every change
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

/* Prowlarr also has Settings → Download Clients, and that is where the
   downloads of searches made in it come out. **One registration per client**
   goes in, all in the same category: what Prowlarr grabs is loose — it did not
   come from an *arr — so it stays together, apart from what each instance
   downloads.

   The name is the client's, and it is by it that reapplying finds what is
   already there. */
const CAT_PROWLARR: &str = "prowlarr";

async fn prowlarr_clients(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    prowlarr: &Prowlarr,
    log: &Log,
) -> usize {
    let target = prowlarr_target(prowlarr);
    let url = format!("{base}{}/api/v1/downloadclient", prowlarr.route);
    let current = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(msg!("job.apply.item", "Prowlarr", e));
            return req.clients.len();
        }
    };
    let mut failures = 0;
    for client in &req.clients {
        let schema = schema_of(http, &url, &req.api_key, implementation_of(&client.kind)).await;
        let body = match client.body_as(&client.name, &target.cat_field, CAT_PROWLARR, schema.as_ref()) {
            /* Prowlarr's client has a property the *arr apps do not have:
               `categories`, which maps a newznab category to a client category.
               Empty means "holds for everything" — but **absent** becomes null,
               and its connection test blows up in a `NullReferenceException`
               inside `ValidateCategories`, which says nothing about the cause. */
            Ok(mut b) => {
                b["categories"] = json!([]);
                b
            }
            Err(e) => {
                log.line(msg!("job.apply.link", "Prowlarr", client.name.clone(), e));
                failures += 1;
                continue;
            }
        };
        let existing = current
            .iter()
            .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(client.name.as_str()))
            .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
        match send(http, &url, &req.api_key, existing, body).await {
            Ok(()) => log.line(msg!(
                "job.apply.prowlarrClientResult",
                client.name.clone(),
                Msg::k(if existing.is_some() { "job.apply.updated" } else { "job.apply.registered" })
            )),
            Err(e) => {
                log.line(msg!("job.apply.link", "Prowlarr", client.name.clone(), e));
                failures += 1;
            }
        }
    }
    failures
}

/* The challenge solver in Prowlarr, under Settings → Indexers → Indexer
   Proxies. Without it, an indexer behind Cloudflare's anti-bot challenge comes
   back empty or with an error, and the page could only say how to configure it
   by hand.

   Prowlarr matches proxy to indexer **by tag**: the proxy holds for the
   indexers carrying its tag. So here the `flaresolverr` tag is created (or
   reused) and the registration is born with it — what is left for whoever uses
   it is tagging the indexers that need it, which is exactly the choice Hubstarr
   cannot make for anyone. */
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
    let tag = match tag(http, &tag_url, &req.api_key, TAG_SOLVER).await {
        Ok(id) => id,
        Err(e) => {
            log.line(msg!("job.apply.link", "Prowlarr", Msg::k("job.apply.solverTagLabel"), e));
            return 1;
        }
    };

    let url = format!("{base}{}/api/v1/indexerproxy", prowlarr.route);
    let current = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(msg!("job.apply.link", "Prowlarr", solver.name.clone(), e));
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
            // the challenge takes a few seconds; Prowlarr's default is 60
            field("requestTimeout", json!(60)),
        ],
        "tags": [tag],
    });
    let existing = current
        .iter()
        .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(solver.name.as_str()))
        .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
    match send(http, &url, &req.api_key, existing, body).await {
        Ok(()) => {
            log.line(msg!(
                "job.apply.solverResult",
                solver.name.clone(),
                Msg::k(if existing.is_some() { "job.apply.updated" } else { "job.apply.registered" })
            ));
            0
        }
        Err(e) => {
            log.line(msg!("job.apply.link", "Prowlarr", solver.name.clone(), e));
            1
        }
    }
}

/// The id of the tag with this label, creating it if it does not exist yet.
/// Prowlarr stores the label in lowercase, and that is how it is looked up.
async fn tag(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    label: &str,
) -> Result<i64, Msg> {
    let current = list(http, url, key).await?;
    if let Some(id) = current
        .iter()
        .find(|t| {
            t.get("label")
                .and_then(|l| l.as_str())
                .map(|l| l.eq_ignore_ascii_case(label))
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
        .body(json!({"label": label}).to_string())
        .send()
        .await
        .map_err(|e| msg!("job.apply.tagCallError", e.to_string()))?;
    let st = r.status();
    api("POST", url, st);
    let txt = r.text().await.unwrap_or_default();
    if !st.is_success() {
        return Err(error(st, &txt));
    }
    serde_json::from_str::<Value>(&txt)
        .ok()
        .and_then(|v| v.get("id").and_then(|i| i.as_i64()))
        .ok_or_else(|| Msg::k("job.apply.noTagId"))
}

/* The categories inside the download client itself.

   In both cases through the **app's API**, and not by writing its file:
   qBittorrent's `categories.json` and `sabnzbd.ini` belong to whoever created
   them, and touching them requires stopping the container — while both have an
   endpoint for this, with the app up. `categories.json` still comes out in the
   `.zip`, which is the way out for whoever has no server and therefore no API
   to call.

   Each category gets a folder of its own name inside the completed-downloads
   directory — the same partition as what the *arr apps see, which is what
   preserves the hardlink on import. */
async fn client_categories(
    http: &reqwest::Client,
    req: &Req,
    client: &Client,
    log: &Log,
) -> usize {
    if client.kind == "qbittorrent" {
        return qbit_categories(http, client, log).await;
    }
    if client.kind != "sabnzbd" {
        return 0;
    }
    if client.web_url.is_empty() || client.api_key.is_empty() {
        log.line(msg!("job.apply.item", client.name.clone(), Msg::k("job.apply.noApiKeyForCategories")));
        return 1;
    }
    let mut failures = 0;
    let mut all = client.categories();
    // Prowlarr's does not come from the page's `cats`: it belongs here
    if req.prowlarr.is_some() && !all.iter().any(|c| c == CAT_PROWLARR) {
        all.push(CAT_PROWLARR.into());
    }
    for cat in all {
        let url = format!(
            "{}api?mode=set_config&section=categories&keyword={cat}&dir={cat}&output=json&apikey={}",
            client.web_url, client.api_key
        );
        match retry("GET", &without_query(&url), || http.get(&url).send()).await {
            Ok(r) if r.status().is_success() => {
                api("GET", &without_query(&url), r.status());
                log.line(msg!(
                    "job.apply.link",
                    client.name.clone(),
                    msg!("job.apply.catName", cat.clone()),
                    Msg::k("job.apply.readyF")
                ));
            }
            Ok(r) => {
                log.line(msg!(
                    "job.apply.link",
                    client.name.clone(),
                    msg!("job.apply.catName", cat.clone()),
                    msg!("job.apply.httpStatus", r.status().as_u16())
                ));
                failures += 1;
            }
            Err(e) => {
                log.line(msg!("job.apply.link", client.name.clone(), msg!("job.apply.catName", cat.clone()), e));
                failures += 1;
            }
        }
    }
    failures
}

/* qBittorrent's preferences, through its API.

   The conf `patch.rs` writes is what it reads when it is **born**; this is the
   same set of decisions applied to a qBittorrent that already exists — and, in
   the case of automatic torrent management, something the conf does not even
   cover. The whole body arrives ready from the page, in `prefs`: nothing is
   decided here, it is only delivered.

   Two things about its API that cost a round if you do not know them:
   `setPreferences` takes a **form** with a `json` field, not a JSON body; and
   the authentication is the API key when the app takes it (see `qbit_auth`) —
   which is what lets this round change the interface password without locking
   itself out of the next one. */
async fn client_preferences(http: &reqwest::Client, client: &Client, log: &Log) -> usize {
    if client.kind != "qbittorrent" {
        return 0;
    }
    let Some(prefs) = client.prefs.as_ref().filter(|p| !p.is_empty()) else {
        return 0;
    };
    if client.web_url.is_empty() {
        return 0;
    }
    let base = client.web_url.trim_end_matches('/').to_string();

    let auth = match qbit_auth(http, &base, client).await {
        Ok(a) => a,
        Err(e) => {
            log.line(msg!("job.apply.link", client.name.clone(), Msg::k("job.apply.enterForPrefs"), e));
            return 1;
        }
    };
    crate::journal::detail(|| format!("{}: preferences via {}", client.name, auth.kind()));

    let body = format!("json={}", enc(&Value::Object(prefs.clone()).to_string()));
    let target = format!("{base}/api/v2/app/setPreferences");
    let sent = retry("POST", &target, || {
        auth.apply(
            http.post(&target)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body.clone()),
        )
        .send()
    })
    .await;
    match sent {
        Ok(r) if r.status().is_success() => {
            api("POST", &target, r.status());
            // the keys go to the log, the values do not: the password is among them
            let keys: Vec<&str> = prefs.keys().map(String::as_str).collect();
            log.line(msg!("job.apply.link", client.name.clone(), Msg::k("job.apply.prefsLabel"), keys.join(", ")));
            check_api_key(http, &base, client, &auth, log).await
        }
        Ok(r) => {
            let st = r.status();
            let txt = r.text().await.unwrap_or_default();
            log.line(msg!("job.apply.link", client.name.clone(), Msg::k("job.apply.prefsLabel"), error(st, &txt)));
            1
        }
        Err(e) => {
            log.line(msg!("job.apply.link", client.name.clone(), Msg::k("job.apply.prefsLabel"), e));
            1
        }
    }
}

/* The API key that counts is the app's.

   Its conf only receives ours when it does not have one yet — that is the rule
   of `keep_keys` in `patch.rs`, so as not to cut off whoever was already
   talking to it. The consequence is this: whoever has a key of their own keeps
   it, and it is with that key that the *arr apps have to be registered.

   Reading is cheap and settles both cases: a key of its own becomes the key of
   the whole round, and an app with no key keeps ours, which is the one
   `patch.rs` has just written. A failed read is no error — we go on with the
   page's key, which is what happened before this existed. */
async fn adopt_api_key(http: &reqwest::Client, req: &mut Req, log: &Log) {
    for i in 0..req.clients.len() {
        let (kind, web_url) = {
            let c = &req.clients[i];
            (c.kind.clone(), c.web_url.clone())
        };
        if kind != "qbittorrent" || web_url.is_empty() {
            continue;
        }
        let base = web_url.trim_end_matches('/').to_string();
        let Ok(auth) = qbit_auth(http, &base, &req.clients[i]).await else {
            continue;
        };
        /* The key opened the app: it is ours, and there is nothing to adopt.
           The read below only makes sense on the other branch, where we got in
           with the password precisely because our key did not work. */
        if matches!(auth, QbitAuth::Key(_)) {
            continue;
        }
        let Some(dele) = read_api_key(http, &base, &auth).await else {
            continue;
        };
        let c = &mut req.clients[i];
        if dele.is_empty() || dele == c.api_key {
            continue;
        }
        log.line(msg!("job.apply.item", c.name.clone(), Msg::k("job.apply.keptKey")));
        c.api_key = dele;
    }
}

/// The preferences' `web_ui_api_key`: it is the read-only mirror of the conf's
/// `WebUI\APIKey`. `None` when it could not be read — the caller goes on with what it had.
async fn read_api_key(http: &reqwest::Client, base: &str, auth: &QbitAuth) -> Option<String> {
    let url = format!("{base}/api/v2/app/preferences");
    let r = retry("GET", &url, || auth.apply(http.get(&url)).send())
        .await
        .ok()?;
    api("GET", &url, r.status());
    if !r.status().is_success() {
        return None;
    }
    serde_json::from_str::<Value>(&r.text().await.ok()?)
        .ok()?
        .get("web_ui_api_key")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/* Is the app's API key the Environment's one?

   It is **not** written through here, and that was measured on 5.2.3:
   `setPreferences` accepts `web_ui_api_key`, answers 200 and changes nothing —
   the property is a read-only mirror of the conf's `WebUI\APIKey`, which is
   where `patch.rs` writes it. There is no endpoint of its own to create it
   (`apiKeys`, `generateApiKey` and the like answer 404).

   So the useful thing left to do is **check**: it is by that key that the *arr
   apps talk to it, and seeing it differ from ours is the warning that the conf
   was written from outside — or that the app was born before Hubstarr arrived.
   Better a line in the log saying so than a client registered with a key that
   opens nothing.

   Checking is not failing: the conf rules over the key, and the app may be
   deliberately used with its own. */
async fn check_api_key(
    http: &reqwest::Client,
    base: &str,
    client: &Client,
    auth: &QbitAuth,
    log: &Log,
) -> usize {
    if client.api_key.is_empty() {
        return 0;
    }
    /* A key outside the format is worse than a wrong key, because it raises no
       error anywhere: 5.2.3's `isValid()` demands `qbt_` and 32 characters in
       total, and what does not pass is **discarded silently** at startup
       (`if isValid(apiKey) m_apiKey = apiKey`) — the app is left with no key,
       key authentication never comes in, and the *arr gets a 403 on the first
       connection test with nothing saying why. */
    if !api_key_valid(&client.api_key) {
        log.line(msg!("job.apply.item", client.name.clone(), Msg::k("job.apply.badKeyFormat")));
        return 1;
    }
    let Some(dele) = read_api_key(http, base, auth).await else {
        return 0;
    };
    if dele == client.api_key {
        crate::journal::detail(|| format!("{}: the app's API key is the one the *arr got", client.name));
    } else if dele.is_empty() {
        log.line(msg!("job.apply.item", client.name.clone(), Msg::k("job.apply.appHasNoKey")));
    } else {
        /* It should not happen: `adopt_api_key()` runs before everything and would
           already have taken the app's key. If the two differ here, it changed
           mid-round — someone touching its interface, or a conf rewritten — and
           the *arr apps were left with the previous one. */
        log.line(msg!("job.apply.item", client.name.clone(), Msg::k("job.apply.keyChangedMidRound")));
    }
    0
}

/// What qBittorrent 5.2.3's `Utils::APIKey::isValid()` demands: the `qbt_`
/// prefix and 32 characters in total. The alphabet of whoever generates it is
/// narrower, but the app does not check that — so neither do we.
fn api_key_valid(k: &str) -> bool {
    k.starts_with("qbt_") && k.chars().count() == 32
}

/* How the server authenticates with qBittorrent.

   The **API key** comes first: it is the same one the *arr apps are registered
   with, it does not expire when the interface password changes and it needs no
   session. That last part is what matters here — the round also *changes* the
   interface password (`web_ui_password` goes in the preferences), and a login
   with the password we are about to replace is a round that stops working the
   second time it runs.

   The session from `auth/login` stays as the fallback, for the app that does
   not take the key: one from before 5.2, or one whose conf never received a
   key. Both ends are checked against the app itself, not guessed — the key is
   probed on `app/version` before anything is changed with it. */
enum QbitAuth {
    Key(String),
    Cookie(String),
}

impl QbitAuth {
    fn apply(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            QbitAuth::Key(k) => req.header("Authorization", format!("Bearer {k}")),
            QbitAuth::Cookie(c) if !c.is_empty() => req.header("Cookie", c.clone()),
            QbitAuth::Cookie(_) => req,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            QbitAuth::Key(_) => "API key",
            QbitAuth::Cookie(_) => "username and password",
        }
    }
}

/// The key when the app answers to it, the session when it does not. The probe
/// is `app/version`, which changes nothing and needs no body: a 200 means the
/// key authenticates, and a 401/403 means this app only knows the password.
async fn qbit_auth(
    http: &reqwest::Client,
    base: &str,
    client: &Client,
) -> Result<QbitAuth, Msg> {
    if api_key_valid(&client.api_key) {
        let url = format!("{base}/api/v2/app/version");
        let key = client.api_key.clone();
        if let Ok(r) = retry("GET", &url, || {
            http.get(&url).header("Authorization", format!("Bearer {key}")).send()
        })
        .await
        {
            api("GET", &url, r.status());
            if r.status().is_success() {
                return Ok(QbitAuth::Key(client.api_key.clone()));
            }
        }
        crate::journal::detail(|| {
            format!(
                "{}: the API key did not open the app ({}) — logging in with username and password",
                client.name, base
            )
        });
    }
    qbit_login(http, base, client).await.map(QbitAuth::Cookie)
}

/// The qBittorrent session: `auth/login` returns the cookie in `Set-Cookie`,
/// and it is what authorizes the following calls. It returns empty when the app
/// accepts with no session at all (the case of `LocalHostAuth` turned off),
/// because then there is no cookie to send and the call goes through anyway.
async fn qbit_login(
    http: &reqwest::Client,
    base: &str,
    client: &Client,
) -> Result<String, Msg> {
    let body = format!(
        "username={}&password={}",
        enc(&client.user),
        enc(&client.pass)
    );
    let target = format!("{base}/api/v2/auth/login");
    let r = retry("POST", &target, || {
        http.post(&target)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send()
    })
    .await?;
    let st = r.status();
    api("POST", &target, st);
    let cookie = r
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| v.split(';').next().map(str::to_string))
        .unwrap_or_default();
    if !st.is_success() {
        let txt = r.text().await.unwrap_or_default();
        return Err(error(st, &txt));
    }
    // a "Fails." body with a 200 is how it says the credential is no good
    let txt = r.text().await.unwrap_or_default();
    if txt.trim().eq_ignore_ascii_case("fails.") {
        return Err(Msg::k("job.apply.badCredentials"));
    }
    Ok(cookie)
}

/* qBittorrent's categories, through `torrents/createCategory`.

   The body is a **form** (`category=…&savePath=…`), like the rest of its API,
   and the authentication is the one from `qbit_auth()`. A category that already
   exists returns **409**, and then the way is `editCategory`, with the same
   body: that way reapplying fixes the folder of whoever was already there
   instead of failing — it is the same rule as the rest of the round, look up
   and update in place.

   A category is **never removed**: what is there may have a torrent pointing at
   it, and taking it away would move what has already been downloaded. */
async fn qbit_categories(http: &reqwest::Client, client: &Client, log: &Log) -> usize {
    if client.categories.is_empty() || client.web_url.is_empty() {
        return 0;
    }
    let base = client.web_url.trim_end_matches('/').to_string();
    let auth = match qbit_auth(http, &base, client).await {
        Ok(a) => a,
        Err(e) => {
            log.line(msg!("job.apply.link", client.name.clone(), Msg::k("job.apply.enterForCategories"), e));
            return 1;
        }
    };
    crate::journal::detail(|| format!("{}: categories via {}", client.name, auth.kind()));

    let mut failures = 0;
    for cat in &client.categories {
        let body = format!(
            "category={}&savePath={}",
            enc(&cat.name),
            enc(&cat.save_path)
        );
        let mut done = false;
        for action in ["createCategory", "editCategory"] {
            let target = format!("{base}/api/v2/torrents/{action}");
            let r = retry("POST", &target, || {
                auth.apply(
                    http.post(&target)
                        .header("Content-Type", "application/x-www-form-urlencoded")
                        .body(body.clone()),
                )
                .send()
            })
            .await;
            match r {
                Ok(r) if r.status().is_success() => {
                    api("POST", &target, r.status());
                    log.line(msg!(
                        "job.apply.link",
                        client.name.clone(),
                        msg!("job.apply.catFolder", cat.name.clone(), cat.save_path.clone()),
                        Msg::k("job.apply.readyF")
                    ));
                    done = true;
                    break;
                }
                // 409 is "already exists": the second pass of the loop edits it in place
                Ok(r) if r.status() == reqwest::StatusCode::CONFLICT
                    && action == "createCategory" =>
                {
                    api("POST", &target, r.status());
                }
                Ok(r) => {
                    let st = r.status();
                    let txt = r.text().await.unwrap_or_default();
                    log.line(msg!(
                        "job.apply.link",
                        client.name.clone(),
                        msg!("job.apply.catName", cat.name.clone()),
                        error(st, &txt)
                    ));
                    break;
                }
                Err(e) => {
                    log.line(msg!("job.apply.link", client.name.clone(), msg!("job.apply.catName", cat.name.clone()), e));
                    break;
                }
            }
        }
        if !done {
            failures += 1;
        }
    }
    failures
}

/// Prowlarr syncing each *arr ticked in the Configuration. The same rule as
/// the clients holds: look up by name, update in place, and whatever fails
/// becomes a line in the log instead of bringing the rest down.
async fn applications(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    prowlarr: &Prowlarr,
    log: &Log,
) -> usize {
    let url = format!("{base}{}/api/v1/applications", prowlarr.route);
    let current = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(msg!("job.apply.item", "Prowlarr", e));
            return req.arrs.iter().filter(|a| a.sync).count();
        }
    };
    let mut failures = 0;
    for arr in req.arrs.iter().filter(|a| a.sync) {
        let body = match app_body(arr, &prowlarr.url, &req.api_key) {
            Ok(b) => b,
            Err(e) => {
                log.line(msg!("job.apply.link", "Prowlarr", arr.name.clone(), e));
                failures += 1;
                continue;
            }
        };
        let existing = current
            .iter()
            .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(arr.name.as_str()))
            .and_then(|c| c.get("id").and_then(|i| i.as_i64()));
        match send(http, &url, &req.api_key, existing, body).await {
            Ok(()) => log.line(msg!(
                "job.apply.link",
                "Prowlarr",
                arr.name.clone(),
                Msg::k(if existing.is_some() { "job.apply.updated" } else { "job.apply.registered" })
            )),
            Err(e) => {
                log.line(msg!("job.apply.link", "Prowlarr", arr.name.clone(), e));
                failures += 1;
            }
        }
    }
    failures
}

/* ---------- Media Management ---------- */

/* The page's options, with the name each one has in the API. `naming` and
   `mediamanagement` are single resources full of fields the page does not
   show, so nothing is built from scratch here: the resource is read, only these
   keys are replaced, and the rest goes back as it was.

   What the page calls `rename` is each family's "rename on import"; Lidarr's
   `useExisting` does not come in, it is the checkbox that shows or hides the
   formats in the interface, and what it describes — importing with the name the
   file already had — is `renameTracks` turned off. */
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
    // the Configuration's advanced block: the same names in all three families
    ("rescan", "rescanAfterRefresh"),
    // this one's value changes per family — the page is what picks the options
    ("fileDate", "fileDate"),
    ("recycleBin", "recycleBin"),
    ("recycleDays", "recycleBinCleanupDays"),
    ("extraFiles", "importExtraFiles"),
    ("extraExts", "extraFileExtensions"),
    ("skipFree", "skipFreeSpaceCheckWhenImporting"),
    ("minFree", "minimumFreeSpaceWhenImporting"),
];

/// Fields the interface types as text and the API wants as a number.
const NUMERIC: &[&str] = &["recycleBinCleanupDays", "minimumFreeSpaceWhenImporting"];

/* The two list fields of the naming travel by name and reach the API as a
   number: the order here is the *arr enum's, and it is the same as the page's
   `COLON` and `MULTIEP`. A name that is not in the list becomes an error, not a
   zero — if the two ends fall out of sync, better a line in the log than the app
   configured with the first option and nobody noticing. */
const COLON: &[&str] = &["delete", "dash", "spaceDash", "spaceDashSpace", "smart"];
const MULTI_EP: &[&str] = &["extend", "duplicate", "repeat", "scene", "range", "prefixedRange"];

fn enum_value(field: &str, v: &Value) -> Result<Value, Msg> {
    if NUMERIC.contains(&field) {
        // a text field on the page, a number in the API: empty becomes 0, and what
        // is not a number becomes an error instead of a silent zero
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
            .map_err(|_| msg!("job.apply.notANumber", field, txt.clone()));
    }
    let list = match field {
        "colonReplacementFormat" => COLON,
        "multiEpisodeStyle" => MULTI_EP,
        _ => return Ok(v.clone()),
    };
    let name = v.as_str().unwrap_or("");
    list
        .iter()
        .position(|x| *x == name)
        .map(|i| json!(i))
        .ok_or_else(|| msg!("job.apply.unknownOption", field, name))
}

/// Replaces in the resource that was read only what the page governs, leaving the rest intact.
fn merge(current: &mut Value, de: &Map<String, Value>, map: &[(&str, &str)]) -> Result<(), Msg> {
    for (pagina, api) in map {
        if let Some(v) = de.get(*pagina) {
            current[*api] = enum_value(api, v)?;
        }
    }
    Ok(())
}

/* Jellyfin: the initial wizard and the libraries.

   It does not use the *arr apps' `X-Api-Key`, so these calls build their own
   requests — as SABnzbd's categories already do. There are two paths, and the
   one that picks is `StartupWizardCompleted` from `/System/Info/Public`, which
   answers with no authentication at all:

   - **Wizard open**: Jellyfin's `FirstTimeSetupOrElevated` policy allows
     creating a user and a library **without a token**, as long as it has not
     finished. That is why the order matters — the libraries go in *before*
     `Complete`, and after it the door closes.
   - **Wizard closed**: a token is needed, and it comes from the modal's user and
     password. Without them there is nothing to do: it becomes a line in the log,
     not a failure of the stack.

   `Complete` is only called when there was an administrator to create: with no
   account at all, a Jellyfin with a closed wizard lets nobody in.

   A library that already exists is not touched, and none is removed: deleting
   would take along what the owner organized. */
const JF_CLIENT: &str = "Hubstarr";

async fn jellyfin(http: &reqwest::Client, jf: &Jellyfin, log: &Log) -> usize {
    let base = jf.url.trim_end_matches('/').to_string();
    wait_url(http, &format!("{base}/System/Info/Public"), &jf.name, log).await;

    let info = format!("{base}/System/Info/Public");
    let ready = match retry("GET", &info, || http.get(&info).send()).await {
        Ok(r) => {
            api("GET", &info, r.status());
            let txt = r.text().await.unwrap_or_default();
            serde_json::from_str::<Value>(&txt)
                .ok()
                .and_then(|v| v["StartupWizardCompleted"].as_bool())
                // not knowing, the safe path is the one that asks for a token: touching
                // the wizard of someone who already finished it is what would do damage
                .unwrap_or(true)
        }
        Err(e) => {
            log.line(msg!("job.apply.item", jf.name.clone(), e));
            return 1;
        }
    };

    let mut failures = 0;
    let mut token = String::new();
    let mut admin_ok = false;

    if !ready {
        let (f, ok) = wizard(http, &base, jf, log).await;
        failures += f;
        admin_ok = ok;
    } else if !jf.user.is_empty() && !jf.pass.is_empty() {
        match authenticate(http, &base, jf).await {
            Ok(t) => token = t,
            Err(e) => {
                log.line(msg!("job.apply.link", jf.name.clone(), msg!("job.apply.loginAsUser", jf.user.clone()), e));
                return failures + 1;
            }
        }
    } else {
        /* Wizard already finished and no credential: the libraries require a
           token, and guessing is not an option. Saying so is better than a
           round that silently does nothing. */
        log.line(msg!("job.apply.item", jf.name.clone(), Msg::k("job.apply.wizardNeedsCreds")));
        return failures;
    }

    /* The wizard-already-finished path used to apply no language at all: the
       block above only runs inside `wizard()`, so a Jellyfin that was already
       set up quietly ignored the field. With a token there is an endpoint for
       it, and it is the same decision written to the server's own config. */
    if ready && !jf.meta_lang.as_deref().unwrap_or("").is_empty() {
        failures += server_language(http, &base, jf, &token, log).await;
    }

    failures += libraries(http, &base, jf, &token, log).await;

    /* Last: closing the wizard seals what came in without a token. Except that
       closing it without having created any administrator hands over a Jellyfin
       with no account to log in with — so, with no credential in the modal, it
       stays open on purpose, with the libraries already there, for whoever
       deployed it to finish in the browser. */
    /* What decides this is the administrator that **exists**, not the one that
       was asked for: the step can fail — and did, with `POST /Startup/User`
       answering 404 on a Jellyfin whose first account had not been brought into
       being — and closing the wizard on top of that hands over a server nobody
       can log into, with no way back through the interface. */
    let asked_admin = !jf.user.is_empty() && !jf.pass.is_empty();
    if !ready && !admin_ok {
        let why = if asked_admin {
            Msg::k("job.apply.wizardKeptOpen")
        } else {
            Msg::k("job.apply.libsReadyWizardOpen")
        };
        log.line(msg!("job.apply.item", jf.name.clone(), why));
    }
    if !ready && admin_ok {
        let target = format!("{base}/Startup/Complete");
        let r = retry("POST", &target, || {
            http.post(&target).header("Content-Length", "0").send()
        })
        .await;
        if let Ok(r) = &r {
            api("POST", &target, r.status());
        }
        match r {
            Ok(r) if r.status().is_success() => {
                log.line(msg!("job.apply.step", jf.name.clone(), Msg::k("job.apply.wizardDone")))
            }
            Ok(r) => {
                log.line(msg!(
                    "job.apply.link",
                    jf.name.clone(),
                    Msg::k("job.apply.wizardLabel"),
                    msg!("job.apply.httpStatus", r.status().as_u16())
                ));
                failures += 1;
            }
            Err(e) => {
                log.line(msg!("job.apply.link", jf.name.clone(), Msg::k("job.apply.wizardLabel"), e));
                failures += 1;
            }
        }
    }
    failures
}

/// The wizard's first user, read before it is written.
///
/// `POST /Startup/User` does not create an account: it **renames** the one that
/// is already there, and answers `404` when there is none — a status its own
/// OpenAPI does not even list, so it reads like a wrong address instead of what
/// it is. In Jellyfin 10.11 that account does not exist until something asks
/// for it, and this GET is what asks: it is the call the browser wizard makes
/// before showing the form. Measured on 10.11.11: the POST alone answers 404
/// and no user is born; a GET before it, and the POST answers 204 with an
/// account that logs in. On a Jellyfin that already has the user, the GET is
/// simply what it always was — the one that is there.
async fn first_user(http: &reqwest::Client, url: &str) -> Result<(), Msg> {
    let r = retry("GET", url, || http.get(url).send()).await?;
    let st = r.status();
    api("GET", url, st);
    if st.is_success() {
        Ok(())
    } else {
        Err(error(st, &r.text().await.unwrap_or_default()))
    }
}

/// The initial wizard, in the order Jellyfin expects it. Nothing here carries
/// a token: it is the window in which it accepts none.
///
/// It answers how many steps failed **and whether the administrator was really
/// created** — the second one because it is what decides whether the wizard may
/// be closed, and "there was a user and a password in the modal" is not the
/// same thing as "the account exists".
async fn wizard(http: &reqwest::Client, base: &str, jf: &Jellyfin, log: &Log) -> (usize, bool) {
    let mut failures = 0;
    let mut admin = false;
    {
        let mut step = |what: Msg, result: Result<(), Msg>| match result {
            Ok(()) => {
                log.line(msg!("job.apply.link", jf.name.clone(), what, Msg::k("job.apply.readyM")));
                true
            }
            Err(e) => {
                log.line(msg!("job.apply.link", jf.name.clone(), what, e));
                failures += 1;
                false
            }
        };

        if !jf.culture.is_empty() {
            let meta = meta_culture(jf);
            let body = json!({
                "ServerName": jf.name,
                "UICulture": jf.culture,
                "MetadataCountryCode": country_of(meta),
                "PreferredMetadataLanguage": language_of(meta),
            });
            let r = post_json(http, &format!("{base}/Startup/Configuration"), "", body).await;
            step(Msg::k("job.apply.language"), r);
        }

        if !jf.user.is_empty() && !jf.pass.is_empty() {
            let target = format!("{base}/Startup/User");
            let body = json!({"Name": jf.user, "Password": jf.pass});
            let r = match first_user(http, &target).await {
                Ok(()) => post_json(http, &target, "", body).await,
                Err(e) => Err(e),
            };
            admin = step(msg!("job.apply.admin", jf.user.clone()), r);
        }

        let body = json!({"EnableRemoteAccess": true, "EnableAutomaticPortMapping": false});
        let r = post_json(http, &format!("{base}/Startup/RemoteAccess"), "", body).await;
        step(Msg::k("job.apply.remoteAccess"), r);
    }
    (failures, admin)
}

/// The token of an already configured Jellyfin. The `Authorization` header with
/// the MediaBrowser fields is mandatory: without it the call is refused before
/// the password is even looked at.
async fn authenticate(http: &reqwest::Client, base: &str, jf: &Jellyfin) -> Result<String, Msg> {
    let target = format!("{base}/Users/AuthenticateByName");
    let body = json!({"Username": jf.user, "Pw": jf.pass}).to_string();
    let r = retry("POST", &target, || {
        http.post(&target)
            .header(
                "Authorization",
                format!(
                    "MediaBrowser Client=\"{JF_CLIENT}\", Device=\"{JF_CLIENT}\", \
                     DeviceId=\"hubstarr\", Version=\"{}\"",
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
    })
    .await?;
    let st = r.status();
    api("POST", &target, st);
    let txt = r.text().await.unwrap_or_default();
    if !st.is_success() {
        return Err(error(st, &txt));
    }
    serde_json::from_str::<Value>(&txt)
        .ok()
        .and_then(|v| v["AccessToken"].as_str().map(String::from))
        .ok_or_else(|| Msg::k("job.apply.noTokenAfterLogin"))
}

/* The metadata language of a Jellyfin that is already set up. The whole
   configuration is read and written back, for the same reason `naming` and
   `mediamanagement` are: it is a single resource full of fields the page never
   shows, and building it from scratch would wipe them. */
async fn server_language(
    http: &reqwest::Client,
    base: &str,
    jf: &Jellyfin,
    token: &str,
    log: &Log,
) -> usize {
    let meta = meta_culture(jf).to_string();
    let url = format!("{base}/System/Configuration");
    let current = match retry("GET", &url, || {
        http.get(&url).header("X-Emby-Token", token).send()
    })
    .await
    {
        Ok(r) if r.status().is_success() => {
            api("GET", &url, r.status());
            serde_json::from_str::<Value>(&r.text().await.unwrap_or_default()).ok()
        }
        Ok(r) => {
            log.line(msg!(
                "job.apply.link",
                jf.name.clone(),
                Msg::k("job.apply.language"),
                msg!("job.apply.httpStatus", r.status().as_u16())
            ));
            return 1;
        }
        Err(e) => {
            log.line(msg!("job.apply.link", jf.name.clone(), Msg::k("job.apply.language"), e));
            return 1;
        }
    };
    let Some(mut body) = current.filter(Value::is_object) else {
        log.line(msg!(
            "job.apply.link",
            jf.name.clone(),
            Msg::k("job.apply.language"),
            Msg::k("job.apply.badConfigResponseNotObject")
        ));
        return 1;
    };
    body["PreferredMetadataLanguage"] = json!(language_of(&meta));
    body["MetadataCountryCode"] = json!(country_of(&meta));
    match post_json(http, &url, token, body).await {
        Ok(()) => {
            log.line(msg!(
                "job.apply.link",
                jf.name.clone(),
                Msg::k("job.apply.language"),
                Msg::raw(meta)
            ));
            0
        }
        Err(e) => {
            log.line(msg!("job.apply.link", jf.name.clone(), Msg::k("job.apply.language"), e));
            1
        }
    }
}

/// The libraries that are missing. What is already there stays as it is — the
/// owner may have changed the options, and overwriting would undo their choice.
async fn libraries(
    http: &reqwest::Client,
    base: &str,
    jf: &Jellyfin,
    token: &str,
    log: &Log,
) -> usize {
    let url = format!("{base}/Library/VirtualFolders");
    let current: Vec<Value> = match retry("GET", &url, || {
        http.get(&url).header("X-Emby-Token", token).send()
    })
    .await
    {
        Ok(r) if r.status().is_success() => {
            api("GET", &url, r.status());
            serde_json::from_str(&r.text().await.unwrap_or_default()).unwrap_or_default()
        }
        Ok(r) => {
            log.line(msg!(
                "job.apply.link",
                jf.name.clone(),
                Msg::k("job.apply.librariesLabel"),
                msg!("job.apply.httpStatus", r.status().as_u16())
            ));
            return 1;
        }
        Err(e) => {
            log.line(msg!("job.apply.link", jf.name.clone(), Msg::k("job.apply.librariesLabel"), e));
            return 1;
        }
    };
    let already_has: Vec<String> = current
        .iter()
        .flat_map(|v| v["Locations"].as_array().cloned().unwrap_or_default())
        .filter_map(|p| p.as_str().map(|s| s.trim_end_matches('/').to_string()))
        .collect();

    let mut failures = 0;
    for lib in &jf.libs {
        let target = lib.path.trim_end_matches('/');
        if already_has.iter().any(|p| p == target) {
            log.line(msg!(
                "job.apply.link",
                jf.name.clone(),
                msg!("job.apply.libraryLabel", lib.path.clone()),
                Msg::k("job.apply.alreadyThere")
            ));
            continue;
        }
        let mut request = format!(
            "{url}?name={}&paths={}&refreshLibrary=true",
            enc(&lib.name),
            enc(&lib.path)
        );
        if !lib.kind.is_empty() {
            request.push_str(&format!("&collectionType={}", lib.kind));
        }
        /* The body is mandatory even when empty: without it Jellyfin returns
           400. Since v0.7 it also carries the search language, so a library
           born now fetches its metadata in it — only a new one, because the
           existing ones are not touched, for the same reason as above. */
        let meta = meta_culture(jf);
        let opts = if meta.is_empty() {
            json!({})
        } else {
            json!({"PreferredMetadataLanguage": language_of(meta),
                   "MetadataCountryCode": country_of(meta)})
        };
        let r = post_json(http, &request, token, json!({"LibraryOptions": opts})).await;
        match r {
            Ok(()) => log.line(msg!(
                "job.apply.link",
                jf.name.clone(),
                msg!("job.apply.libraryWithPath", lib.name.clone(), lib.path.clone()),
                Msg::k("job.apply.readyF")
            )),
            Err(e) => {
                log.line(msg!("job.apply.link", jf.name.clone(), msg!("job.apply.libraryLabel", lib.name.clone()), e));
                failures += 1;
            }
        }
    }
    failures
}

/// POST with a JSON body, with the token when there is one.
async fn post_json(
    http: &reqwest::Client,
    url: &str,
    token: &str,
    body: Value,
) -> Result<(), Msg> {
    let text = body.to_string();
    let r = retry("POST", url, || {
        let mut req = http
            .post(url)
            .header("Content-Type", "application/json")
            .body(text.clone());
        if !token.is_empty() {
            req = req.header("X-Emby-Token", token);
        }
        req.send()
    })
    .await?;
    let st = r.status();
    api("POST", url, st);
    if st.is_success() {
        Ok(())
    } else {
        let txt = r.text().await.unwrap_or_default();
        Err(error(st, &txt))
    }
}

/// Escapes what goes in the query — a library name has spaces, and a path has
/// slashes.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The culture the **metadata** follows. Since v0.7 it is the Environment's
/// search language, which is a different question from the interface: whoever
/// builds the stack in Portuguese may well want its titles in Japanese. With no
/// language picked it falls back to the interface one, which is exactly what
/// every stack did before the field existed.
fn meta_culture(jf: &Jellyfin) -> &str {
    match jf.meta_lang.as_deref() {
        Some(l) if !l.is_empty() => l,
        _ => &jf.culture,
    }
}

/// `pt-BR` → `BR`, `en-US` → `US`. With no region, the country is left empty
/// and Jellyfin uses its own default.
fn country_of(culture: &str) -> String {
    culture.split('-').nth(1).unwrap_or("").to_string()
}

/// `pt-BR` → `pt`: the metadata language is only the first part.
fn language_of(culture: &str) -> String {
    culture.split('-').next().unwrap_or("en").to_string()
}

/* The root folders of an *arr — the *Root Folder*, which is where it keeps
   what it downloads. The paths come from the page, which is the one building
   the compose binds and therefore knows what the container sees.

   It adds what is missing and does not touch what is already there: a root
   folder is the kind of thing that is removed with the library along with it,
   so taking away one someone added by hand would be damage, not configuration.
   The app refuses one that already exists with a 400, and that is why the list
   is read first.

   A folder the app cannot reach also becomes a 400 — that is the case of
   sending the host path instead of the one inside the container. Its message
   goes to the log whole: it says which path it is. */
async fn ensure_root_folders(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    arr: &Arr,
    log: &Log,
) -> usize {
    if arr.root_folders.is_empty() {
        return 0;
    }
    let url = format!("{base}{}/api/{}/rootfolder", arr.route, arr.api);
    let current = match list(http, &url, &req.api_key).await {
        Ok(v) => v,
        Err(e) => {
            log.line(msg!("job.apply.link", arr.name.clone(), Msg::k("job.apply.rootFoldersLabel"), e));
            return 1;
        }
    };
    let already_has: Vec<&str> = current
        .iter()
        .filter_map(|v| v.get("path").and_then(|p| p.as_str()))
        // the app sometimes stores it with a trailing slash, and comparing them as
        // text would let the same folder in twice
        .map(|p| p.trim_end_matches('/'))
        .collect();

    /* Lidarr asks for more than the path, and that is why its root folder used
       to fail silently until now. Measured on the app: `Name` cannot be empty,
       and both default profiles have to be greater than zero — without them the
       answer is a validation list, not the folder created. The other *arr apps
       accept just `path`, so this stays where it belongs: in Lidarr's branch. */
    let extras = if arr.family == "lidarr" {
        Some(json!({
            "name": LIDARR_ROOT_NAME,
            "defaultQualityProfileId": first_id(http, base, req, arr, "qualityprofile").await,
            "defaultMetadataProfileId": first_id(http, base, req, arr, "metadataprofile").await,
        }))
    } else {
        None
    };

    let mut failures = 0;
    for dir in &arr.root_folders {
        let target = dir.trim_end_matches('/');
        if already_has.contains(&target) {
            log.line(msg!(
                "job.apply.link",
                arr.name.clone(),
                msg!("job.apply.rootFolder", dir.clone()),
                Msg::k("job.apply.alreadyThere")
            ));
            continue;
        }
        let mut body = json!({"path": dir});
        if let (Some(extras), Some(o)) = (extras.as_ref(), body.as_object_mut()) {
            for (k, v) in extras.as_object().into_iter().flatten() {
                o.insert(k.clone(), v.clone());
            }
        }
        match send(http, &url, &req.api_key, None, body).await {
            Ok(()) => log.line(msg!(
                "job.apply.link",
                arr.name.clone(),
                msg!("job.apply.rootFolder", dir.clone()),
                Msg::k("job.apply.readyF")
            )),
            Err(e) => {
                log.line(msg!("job.apply.link", arr.name.clone(), msg!("job.apply.rootFolder", dir.clone()), e));
                failures += 1;
            }
        }
    }
    failures
}

/* The name of Lidarr's root folder. It demands one and does not derive it from
   the path; the other *arr apps do not even have the field. It accepts a
   repeated name (measured), so a stack with two music folders needs no
   tiebreaker. */
const LIDARR_ROOT_NAME: &str = "Music";

/// The first id of a list resource of the app — the profiles Lidarr's root
/// folder demands. It falls back to `1` when it cannot be read: it is the id of
/// the factory one, and a wrong guess becomes a validation error, not damage.
async fn first_id(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    arr: &Arr,
    resource: &str,
) -> i64 {
    let url = format!("{base}{}/api/{}/{resource}", arr.route, arr.api);
    list(http, &url, &req.api_key)
        .await
        .ok()
        .and_then(|items| {
            items
                .iter()
                .filter_map(|v| v.get("id").and_then(|i| i.as_i64()))
                .min()
        })
        .unwrap_or(1)
}

/// One instance's Media Management and naming. They are two single resources
/// (no id in the route, but an id in the body), so each one is read, changed
/// and sent back whole.
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
    /* The advanced block travels inside `naming` — that is the table storing free
       JSON — but it is a `mediamanagement` field: it comes out of there and in here. */
    let mut mm_fields = mm.clone();
    if let Some(adv) = naming
        .as_ref()
        .and_then(|n| n.get("adv"))
        .and_then(|v| v.as_object())
    {
        for (k, v) in adv {
            mm_fields.insert(k.clone(), v.clone());
        }
    }
    // the "rename" lives in the page's `mm`, but in the API it is a naming
    // field: it goes in along with it
    let mut naming_fields = naming.unwrap_or_default();
    if let Some(r) = mm.get("rename") {
        naming_fields.insert("rename".into(), r.clone());
    }
    // what the Configuration took away from this instance goes as no key at all
    for k in &arr.skip_naming {
        naming_fields.remove(k);
    }

    let mut failures = 0;
    for (resource, fields, map) in [
        (
            "naming",
            naming_fields,
            naming_map(&arr.family).to_vec(),
        ),
        (
            "mediamanagement",
            mm_fields,
            MEDIA_MANAGEMENT.to_vec(),
        ),
    ] {
        let url = format!("{base}{}/api/{}/config/{resource}", arr.route, arr.api);
        match put_config(http, &url, &req.api_key, &fields, &map).await {
            Ok(()) => log.line(msg!("job.apply.link", arr.name.clone(), resource, Msg::k("job.apply.appliedM"))),
            Err(e) => {
                log.line(msg!("job.apply.link", arr.name.clone(), resource, e));
                failures += 1;
            }
        }
    }
    failures
}

/* The subtitle half of the search language (v0.7): Bazarr.

   It speaks neither the *arr apps' API nor Jellyfin's — the key goes in
   `X-API-KEY`, and `system/settings` takes a **form**, not a JSON body, with
   the profiles as JSON inside one field. That key is the app's own, generated
   on its first boot, so it is typed into its modal like SABnzbd's; without one
   this is a line in the log, not a failure of the stack.

   A profile of the same name is **updated in place** and none is ever removed:
   a series may already point at one, exactly as with a root folder. And the
   profile alone configures nothing — what makes a new series or movie inherit
   it is the two `*_default_profile` settings, which is why they go along. */
async fn bazarr(http: &reqwest::Client, bz: &Bazarr, lang: &SearchLang, log: &Log) -> usize {
    if bz.api_key.is_empty() || lang.bz2.is_empty() {
        log.line(msg!("job.apply.item", bz.name.clone(), Msg::k("job.apply.noApiKeyForBazarr")));
        return 0;
    }
    let base = bz.url.trim_end_matches('/').to_string();
    let url = format!("{base}/api/system/settings");

    // what is there now, so a profile of ours is updated instead of duplicated
    let current = match retry("GET", &url, || {
        http.get(&url).header("X-API-KEY", &bz.api_key).send()
    })
    .await
    {
        Ok(r) if r.status().is_success() => {
            api("GET", &url, r.status());
            serde_json::from_str::<Value>(&r.text().await.unwrap_or_default()).unwrap_or(Value::Null)
        }
        Ok(r) => {
            log.line(msg!(
                "job.apply.link",
                bz.name.clone(),
                Msg::k("job.apply.language"),
                msg!("job.apply.httpStatus", r.status().as_u16())
            ));
            return 1;
        }
        Err(e) => {
            log.line(msg!("job.apply.link", bz.name.clone(), Msg::k("job.apply.language"), e));
            return 1;
        }
    };

    let mut profiles: Vec<Value> = current["languagesProfiles"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let name = format!("Hubstarr {}", lang.code);
    let item = json!({"id": 1, "language": lang.bz2, "audio_exclude": "False",
                      "forced": "False", "hi": "False"});
    // an id nobody else has: profiles are numbered from 1 upwards
    let id = profiles
        .iter()
        .find(|p| p["name"].as_str() == Some(name.as_str()))
        .and_then(|p| p["profileId"].as_i64())
        .unwrap_or_else(|| {
            profiles
                .iter()
                .filter_map(|p| p["profileId"].as_i64())
                .max()
                .unwrap_or(0)
                + 1
        });
    let ours = json!({"profileId": id, "name": name, "cutoff": null,
                      "items": [item], "mustContain": [], "mustNotContain": [],
                      "originalFormat": false, "tag": null});
    match profiles
        .iter()
        .position(|p| p["profileId"].as_i64() == Some(id))
    {
        Some(i) => profiles[i] = ours,
        None => profiles.push(ours),
    }

    let body = [
        format!("languages-enabled={}", enc(&lang.bz2)),
        format!("languages-profiles={}", enc(&Value::Array(profiles).to_string())),
        format!("settings-general-serie_default_enabled={}", enc("True")),
        format!("settings-general-serie_default_profile={id}"),
        format!("settings-general-movie_default_enabled={}", enc("True")),
        format!("settings-general-movie_default_profile={id}"),
    ]
    .join("&");
    let sent = retry("POST", &url, || {
        http.post(&url)
            .header("X-API-KEY", &bz.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send()
    })
    .await;
    match sent {
        Ok(r) if r.status().is_success() => {
            api("POST", &url, r.status());
            log.line(msg!(
                "job.apply.link",
                bz.name.clone(),
                Msg::k("job.apply.language"),
                Msg::raw(lang.code.clone())
            ));
            0
        }
        Ok(r) => {
            log.line(msg!(
                "job.apply.link",
                bz.name.clone(),
                Msg::k("job.apply.language"),
                msg!("job.apply.httpStatus", r.status().as_u16())
            ));
            1
        }
        Err(e) => {
            log.line(msg!("job.apply.link", bz.name.clone(), Msg::k("job.apply.language"), e));
            1
        }
    }
}

/* The metadata half of the search language (v0.7): the language the app looks
   titles up in, so that whoever asked for Portuguese gets "Rogue One: Uma
   História Star Wars" and not the English title.

   Only **Radarr** has it. Sonarr's `config/ui` carries no metadata language,
   only `uiLanguage`, which is the *interface* — turning someone's Sonarr
   Japanese because they wanted Japanese titles is not what the field promises,
   so Sonarr's language reaches it through the release half instead, in the
   Configarr templates. Lidarr has neither.

   The number is looked up in the app itself, the same idea as `first_id()`: the
   page sends the **name** Radarr publishes, and a name the app does not have
   becomes a line in the log, never a zero — the same rule as `enum_value()`. */
async fn metadata_language(
    http: &reqwest::Client,
    base: &str,
    req: &Req,
    arr: &Arr,
    log: &Log,
) -> usize {
    let Some(lang) = &req.search_lang else { return 0 };
    if arr.family != "radarr" || lang.arr.is_empty() {
        return 0;
    }
    let list_url = format!("{base}{}/api/{}/language", arr.route, arr.api);
    let id = match list(http, &list_url, &req.api_key).await {
        Ok(all) => all
            .iter()
            .find(|l| l.get("name").and_then(Value::as_str) == Some(lang.arr.as_str()))
            .and_then(|l| l.get("id").and_then(Value::as_i64)),
        Err(e) => {
            log.line(msg!("job.apply.link", arr.name.clone(), Msg::k("job.apply.language"), e));
            return 1;
        }
    };
    let Some(id) = id else {
        log.line(msg!(
            "job.apply.link",
            arr.name.clone(),
            Msg::k("job.apply.language"),
            msg!("job.apply.unknownLanguage", lang.arr.clone())
        ));
        return 1;
    };
    let mut fields = Map::new();
    fields.insert("movieInfoLanguage".into(), json!(id));
    let url = format!("{base}{}/api/{}/config/ui", arr.route, arr.api);
    let map: &[(&str, &str)] = &[("movieInfoLanguage", "movieInfoLanguage")];
    match put_config(http, &url, &req.api_key, &fields, map).await {
        Ok(()) => {
            log.line(msg!(
                "job.apply.link",
                arr.name.clone(),
                Msg::k("job.apply.language"),
                Msg::raw(lang.code.clone())
            ));
            0
        }
        Err(e) => {
            log.line(msg!("job.apply.link", arr.name.clone(), Msg::k("job.apply.language"), e));
            1
        }
    }
}

async fn put_config(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    fields: &Map<String, Value>,
    map: &[(&str, &str)],
) -> Result<(), Msg> {
    let r = retry("GET", url, || http.get(url).header("X-Api-Key", key).send()).await?;
    let st = r.status();
    api("GET", url, st);
    let txt = r.text().await.unwrap_or_default();
    if !st.is_success() {
        return Err(error(st, &txt));
    }
    let mut current: Value =
        serde_json::from_str(&txt).map_err(|_| Msg::k("job.apply.badConfigResponse"))?;
    /* A single resource is an object. If something else arrives — a list, an
       error in JSON, an app of another version — `merge` would index by key and
       **panic**, bringing the whole round down instead of complaining about one
       link. Better the line in the log. */
    if !current.is_object() {
        return Err(Msg::k("job.apply.badConfigResponseNotObject"));
    }
    merge(&mut current, fields, map)?;

    let body = current.to_string();
    let r = retry("PUT", url, || {
        http.put(url)
            .header("X-Api-Key", key)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
    })
    .await?;
    let st = r.status();
    api("PUT", url, st);
    if st.is_success() {
        return Ok(());
    }
    Err(error(st, &r.text().await.unwrap_or_default()))
}

/* Applied right after the `up`, no app has answered yet: they take from a few
   seconds to a minute to open the API. So we wait — but *answering* is not the
   same as *ready*, and the difference is what used to make the first Deploy fail
   in ways the second one did not.

   Two steps, then. `/ping` asks for no key and is the first path they serve: it
   says the process is listening. `system/status`, with the key, is the one that
   says the **initialization finished** — the database migrated, the config
   loaded, the API key in place. Between the two an *arr answers 503, or answers
   401 because it has not read its own key yet, and a client registered in that
   window comes back as a validation error about an app that was merely still
   starting.

   Whoever does not get ready in time interrupts nothing — it goes on as it was,
   and its error shows up line by line in the record of each link. */
async fn wait_apps(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    targets: &[Arr],
    running: &[String],
    log: &Log,
) {
    for target in targets {
        /* A container that is not up answers nothing, and the ninety seconds
           spent asking are ninety seconds of a log that says "still starting"
           about something that never started. The `compose ps` knows better —
           when it is the one that answered, its word is final. */
        if !running.is_empty() && !running.iter().any(|k| k == &target.key) {
            log.line(msg!("job.apply.item", target.name.clone(), Msg::k("job.apply.containerNotRunning")));
            continue;
        }
        wait_url(http, &format!("{base}{}/ping", target.route), &target.name, log).await;
        let url = format!("{base}{}/api/{}/system/status", target.route, target.api);
        wait_ready(http, &url, key, &target.name, log).await;
    }
}

/// The second half of the wait: the app is initialized when the resource that
/// needs its configuration answers **200**. Anything else is "not yet" — the
/// 503 of an app still coming up, the 401 of one that has not loaded its key.
/// Running out of tries is not a failure: the round goes on and whatever is
/// still starting shows up as an error on its own line.
async fn wait_ready(http: &reqwest::Client, url: &str, key: &str, name: &str, log: &Log) {
    let mut warned = false;
    for _ in 0..45 {
        match http.get(url).header("X-Api-Key", key).send().await {
            Ok(r) if r.status().is_success() => return,
            Ok(r) if r.status().is_server_error() => crate::journal::detail(|| {
                format!(
                    "{name}: nginx still does not reach the container — {} at {}",
                    r.status(),
                    without_query(url)
                )
            }),
            Ok(r) => crate::journal::detail(|| {
                format!("{name}: still starting up — {} at {}", r.status(), without_query(url))
            }),
            Err(e) => crate::journal::detail(|| format!("{name}: {e}")),
        }
        if !warned {
            log.line(msg!("job.apply.waitingToStart", name));
            warned = true;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    log.line(msg!("job.apply.stillStarting", name));
}

/* The same for the download clients. It matters less for the first `up` and
   more for what comes right before this round: writing qBittorrent's conf
   **restarts** its container, so when the *arr apps go to register it, it has
   been starting for seconds — and the connection test they run on save fails.

   Any answer will do, 401 and 403 included: what we want to know is whether
   somebody is listening, not whether the credential is right. What will not do
   are nginx's own answers — it returns 502 while the container behind it has
   not come up, and taking that for "ready" is the same as not waiting. */
async fn wait_clients(http: &reqwest::Client, clients: &[Client], log: &Log) {
    for c in clients {
        if c.web_url.is_empty() {
            continue;
        }
        wait_url(http, &c.web_url, &c.name, log).await;
        /* And the app's own API, not just the address: the root of qBittorrent
           answers the login page while it is still reading the conf we have
           just written, and `app/version` only answers once the WebUI is up.
           Any answer of its own counts, 401 and 403 included — what is being
           asked is whether it is initialized, not whether we may come in. */
        let api_url = match c.kind.as_str() {
            "qbittorrent" => format!("{}/api/v2/app/version", c.web_url.trim_end_matches('/')),
            "sabnzbd" => format!("{}/api?mode=version", c.web_url.trim_end_matches('/')),
            _ => continue,
        };
        wait_url(http, &api_url, &c.name, log).await;
    }
}

async fn wait_url(http: &reqwest::Client, url: &str, name: &str, log: &Log) {
    let mut warned = false;
    for _ in 0..45 {
        match http.get(url).send().await {
            // a 5xx here is nginx saying the one behind it has not come up yet
            Ok(r) if !r.status().is_server_error() => break,
            _ => {
                if !warned {
                    log.line(msg!("job.apply.waitingToRespond", name));
                    warned = true;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// The fields the app says a resource has, with its factory values.
/// A failure here is no error: whoever does not serve the schema gets only our fields.
async fn schema_of(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    implementation: &str,
) -> Option<Vec<Value>> {
    let items = list(http, &format!("{url}/schema"), key).await.ok()?;
    items
        .iter()
        .find(|i| i.get("implementation").and_then(|v| v.as_str()) == Some(implementation))
        .and_then(|i| i.get("fields"))
        .and_then(|f| f.as_array())
        .cloned()
}

/// Each client's implementation, which is how the schema identifies it.
fn implementation_of(kind: &str) -> &'static str {
    match kind {
        "sabnzbd" => "Sabnzbd",
        _ => "QBittorrent",
    }
}

/// The clients the *arr already has, to know what is a new registration and
/// what is an update.
async fn list(http: &reqwest::Client, url: &str, key: &str) -> Result<Vec<Value>, Msg> {
    let r = retry("GET", url, || http.get(url).header("X-Api-Key", key).send()).await?;
    let st = r.status();
    api("GET", url, st);
    let txt = r.text().await.unwrap_or_default();
    if !st.is_success() {
        return Err(error(st, &txt));
    }
    match serde_json::from_str::<Value>(&txt) {
        Ok(Value::Array(a)) => Ok(a),
        _ => Err(Msg::k("job.apply.badClientListResponse")),
    }
}

async fn send(
    http: &reqwest::Client,
    url: &str,
    key: &str,
    id: Option<i64>,
    mut body: Value,
) -> Result<(), Msg> {
    // updating requires the id in the body *and* in the path: the *arr refuses if they differ
    let target = match id {
        Some(id) => {
            body["id"] = json!(id);
            format!("{url}/{id}")
        }
        None => url.to_string(),
    };
    let method = if id.is_some() { "PUT" } else { "POST" };
    let body = body.to_string();
    let r = retry(method, &target, || {
        let req = if id.is_some() {
            http.put(&target)
        } else {
            http.post(&target)
        };
        req.header("X-Api-Key", key)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
    })
    .await?;
    api(method, &target, r.status());
    let st = r.status();
    if st.is_success() {
        return Ok(());
    }
    Err(error(st, &r.text().await.unwrap_or_default()))
}

/* A call to a stack app, in detailed mode. This is where `-v` pays off: a
   round of Apply is dozens of them, and knowing **which one** answered what is
   the difference between "it did not work" and the reason. The path and the
   status come out; the body does not, which is where the keys and the passwords
   live. */
/* Insisting when the app does not answer.

   Ten attempts, five seconds apart. It holds for **could not reach**: a
   transport error (nobody listening, connection cut) and a **5xx** answer,
   which behind nginx is it saying the container has not come up yet. An app
   error — 400, 401, 404, the validation refusing the body — is **not** retried:
   the answer would be the same ten times, and fifty seconds per call in a round
   of dozens of them would turn a configuration error into an endless wait.

   This does not replace `wait_apps()`: that one is the single wait, before
   starting; this is the net for what falls **mid-round** — the qBittorrent that
   restarts on receiving the conf, the *arr busy importing.

   The request is built by the function on every attempt, not cloned: a consumed
   body cannot be reused, and `try_clone()` returns `None` exactly when there is
   a streaming body. */
const ATTEMPTS: usize = 10;
const WAIT: Duration = Duration::from_secs(5);

async fn retry<F, Fut>(what: &str, url: &str, mut build: F) -> Result<reqwest::Response, Msg>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    let mut last = String::new();
    for n in 1..=ATTEMPTS {
        match build().await {
            Ok(r) if !r.status().is_server_error() => return Ok(r),
            Ok(r) => last = format!("HTTP {}", r.status().as_u16()),
            Err(e) => last = format!("{e}"),
        }
        if n < ATTEMPTS {
            crate::journal::detail(|| {
                format!("api {what} {url} → {last}, attempt {n}/{ATTEMPTS}")
            });
            tokio::time::sleep(WAIT).await;
        }
    }
    Err(msg!("job.apply.retryTimeout", ATTEMPTS, last))
}

fn api(method: &str, url: &str, st: reqwest::StatusCode) {
    crate::journal::detail(|| format!("api {method} {url} → {}", st.as_u16()));
}

/// The address without the query, for the log: it is in the query that
/// SABnzbd's API key travels and, in Jellyfin, the library name and path.
fn without_query(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

/// The *arr's message, when it sends one: its validation list says far more
/// than the status number — "category does not exist", "could not talk to the
/// client", and so on.
fn error(st: reqwest::StatusCode, body: &str) -> Msg {
    let detail = match serde_json::from_str::<Value>(body) {
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
    if detail.is_empty() {
        msg!("job.apply.httpStatus", st.as_u16())
    } else {
        msg!("job.apply.httpErrorDetail", st.as_u16(), detail)
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
            root_folders: vec![format!("/data/{key}")],
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
            prefs: None,
            categories: Vec::new(),
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
    fn qbittorrent_comes_out_with_its_contract_and_credentials() {
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
    fn qbittorrent_is_registered_by_api_key_when_the_app_knows_one() {
        let c = qbit();
        let a = arr("sonarr", "tvCategory");
        // the app that has the field gets the key, not the password
        let with_key = vec![json!({"name":"apiKey","value":null}),
                       json!({"name":"username","value":null}),
                       json!({"name":"password","value":null}),
                       json!({"name":"host","value":"localhost"})];
        let b = c.body(&a, Some(&with_key)).unwrap();
        assert_eq!(val(&b, "apiKey"), "qbt_chave");
        assert_eq!(val(&b, "username"), Value::Null);
        assert_eq!(val(&b, "password"), Value::Null);
        // the one that does not know the field goes on with user and password
        let without_key = vec![json!({"name":"username","value":null}),
                       json!({"name":"password","value":null})];
        let b = c.body(&a, Some(&without_key)).unwrap();
        assert_eq!(val(&b, "username"), "admin");
        assert_eq!(val(&b, "password"), "senha");
    }

    #[test]
    fn sabnzbd_swaps_user_and_password_for_the_api_key() {
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
    fn the_category_is_the_instance_one_and_vanishes_when_there_is_none() {
        let c = qbit();
        // `cats` only carries sonarr: radarr comes in with no category, which is
        // what the *arr reads as "the client's root"
        assert_eq!(val(&c.body(&arr("radarr", "movieCategory"), None).unwrap(), "movieCategory"), "");
    }

    #[test]
    fn an_unknown_client_kind_stops_at_that_pair_and_not_mid_body() {
        let mut c = qbit();
        c.kind = "transmission".into();
        assert!(c.body(&arr("sonarr", "tvCategory"), None).is_err());
    }

    #[test]
    fn the_prowlarr_application_carries_both_internal_addresses() {
        let a = arr("sonarr-anime", "tvCategory");
        let b = app_body(&a, "http://prowlarr:9696/prowlarr", "chave").unwrap();
        assert_eq!(b["implementation"], "Sonarr");
        assert_eq!(b["configContract"], "SonarrSettings");
        assert_eq!(b["syncLevel"], "fullSync");
        // the name is the instance's, which is what tells two of the same app apart
        assert_eq!(b["name"], "sonarr-anime");
        assert_eq!(val(&b, "prowlarrUrl"), "http://prowlarr:9696/prowlarr");
        assert_eq!(val(&b, "baseUrl"), "http://sonarr-anime:8989/sonarr-anime");
        assert_eq!(val(&b, "apiKey"), "chave");
        // syncing no category at all is a Prowlarr that syncs nothing
        assert!(!val(&b, "syncCategories").as_array().unwrap().is_empty());
    }

    #[test]
    fn with_no_internal_address_the_application_stops_before_hitting_the_network() {
        let mut a = arr("radarr", "movieCategory");
        a.internal_url = String::new();
        assert!(app_body(&a, "http://prowlarr:9696", "chave").is_err());
    }

    #[test]
    fn each_family_has_its_own_categories_and_not_the_others() {
        assert!(sync_categories("sonarr").iter().all(|c| (5000..6000).contains(c)));
        assert!(sync_categories("radarr").iter().all(|c| (2000..3000).contains(c)));
        assert!(sync_categories("lidarr").iter().all(|c| (3000..4000).contains(c)));
        assert!(sync_categories("bazarr").is_empty());
    }

    #[test]
    fn the_merge_replaces_only_what_the_page_governs() {
        let mut current = json!({"id": 1, "renameEpisodes": false,
                               "standardEpisodeFormat": "velho",
                               "campoQueANaoConheco": "fica"});
        let de: Map<String, Value> = serde_json::from_str(
            r#"{"rename": true, "standardEp": "novo", "colon": "smart"}"#,
        )
        .unwrap();
        merge(&mut current, &de, naming_map("sonarr")).unwrap();
        assert_eq!(current["renameEpisodes"], true);
        assert_eq!(current["standardEpisodeFormat"], "novo");
        // the colon goes as a number, in the order of the app's enum
        assert_eq!(current["colonReplacementFormat"], 4);
        // the id and what the page does not show come back as they were
        assert_eq!(current["id"], 1);
        assert_eq!(current["campoQueANaoConheco"], "fica");
        // a field the page did not send is not invented
        assert!(current.get("animeEpisodeFormat").is_none());
    }

    #[test]
    fn the_advanced_block_becomes_a_mediamanagement_field() {
        let mut current = json!({"id": 1, "recycleBin": "", "autoRenameFolders": true});
        let de: Map<String, Value> = serde_json::from_str(
            r#"{"rescan":"never","fileDate":"localAirDate",
                "recycleBin":"/mnt/media/.lixeira","recycleDays":"14",
                "extraFiles":true,"extraExts":"srt,sub","skipFree":false,"minFree":"250"}"#,
        )
        .unwrap();
        merge(&mut current, &de, MEDIA_MANAGEMENT).unwrap();
        assert_eq!(current["rescanAfterRefresh"], "never");
        assert_eq!(current["fileDate"], "localAirDate");
        assert_eq!(current["recycleBin"], "/mnt/media/.lixeira");
        assert_eq!(current["importExtraFiles"], true);
        assert_eq!(current["extraFileExtensions"], "srt,sub");
        assert_eq!(current["skipFreeSpaceCheckWhenImporting"], false);
        // text on the page, a number in the API
        assert_eq!(current["recycleBinCleanupDays"], 14);
        assert_eq!(current["minimumFreeSpaceWhenImporting"], 250);
        // and what belongs to the app is still there
        assert_eq!(current["autoRenameFolders"], true);
    }

    #[test]
    fn an_empty_number_becomes_zero_and_bad_text_becomes_an_error() {
        assert_eq!(enum_value("recycleBinCleanupDays", &json!("")).unwrap(), json!(0));
        assert_eq!(enum_value("minimumFreeSpaceWhenImporting", &json!(" 100 ")).unwrap(), json!(100));
        assert!(enum_value("recycleBinCleanupDays", &json!("uma semana")).is_err());
        // a number that already came as a number passes straight through
        assert_eq!(enum_value("minimumFreeSpaceWhenImporting", &json!(7)).unwrap(), json!(7));
    }

    #[test]
    fn a_list_option_outside_the_enum_fails_instead_of_becoming_the_first() {
        assert_eq!(
            enum_value("multiEpisodeStyle", &json!("prefixedRange")).unwrap(),
            json!(5)
        );
        assert!(enum_value("colonReplacementFormat", &json!("outra")).is_err());
        // a field that is not a list passes as it came
        assert_eq!(
            enum_value("standardEpisodeFormat", &json!("{Series Title}")).unwrap(),
            json!("{Series Title}")
        );
    }

    #[test]
    fn an_instance_outside_the_format_does_not_get_that_key() {
        // the merge only replaces what comes; without the key, the app keeps its own
        // format, which is what Sonarr's mandatory field demands
        let mut current = json!({"id": 1, "animeEpisodeFormat": "o do app",
                               "standardEpisodeFormat": "velho"});
        let mut de: Map<String, Value> = serde_json::from_str(
            r#"{"standardEp": "novo", "animeEp": "nosso"}"#,
        )
        .unwrap();
        let a = Arr { skip_naming: vec!["animeEp".into()], ..arr("sonarr", "tvCategory") };
        for k in &a.skip_naming {
            de.remove(k);
        }
        merge(&mut current, &de, naming_map("sonarr")).unwrap();
        assert_eq!(current["standardEpisodeFormat"], "novo");
        assert_eq!(current["animeEpisodeFormat"], "o do app");
    }

    #[test]
    fn each_family_renames_with_the_name_its_api_uses() {
        let rename = |f| {
            naming_map(f)
                .iter()
                .find(|(p, _)| *p == "rename")
                .map(|(_, a)| *a)
        };
        assert_eq!(rename("sonarr"), Some("renameEpisodes"));
        assert_eq!(rename("radarr"), Some("renameMovies"));
        assert_eq!(rename("lidarr"), Some("renameTracks"));
        // Lidarr has no colon field, and the page does not offer one there either
        assert!(naming_map("lidarr").iter().all(|(p, _)| *p != "colon"));
    }

    #[test]
    fn in_prowlarr_the_client_is_registered_once_in_its_own_category() {
        let c = qbit();
        // one registration per client, with its name and Prowlarr's category —
        // what it grabs is loose, it belongs to no instance
        let b = c.body_as(&c.name, "category", CAT_PROWLARR, None).unwrap();
        assert_eq!(b["name"], "qBittorrent");
        assert_eq!(val(&b, "category"), "prowlarr");
        // and the rest of the client is the same that goes to the *arr apps
        assert_eq!(b["implementation"], "QBittorrent");
        assert_eq!(val(&b, "host"), "gluetun");
        assert_eq!(val(&b, "username"), "admin");
    }

    /// The format qBittorrent demands, checked in the 5.2.3 source
    /// (`Utils::APIKey::isValid`): the prefix and 32 characters. What does not pass
    /// it discards silently, and key authentication never comes in.
    #[test]
    fn the_qbittorrent_api_key_has_the_right_shape() {
        assert!(super::api_key_valid("qbt_ABCDEFGHJKLMNPQRSTUVWXYZ2345"));
        // 27 after the prefix: one fewer, and the app ignores it
        assert!(!super::api_key_valid("qbt_ABCDEFGHJKLMNPQRSTUVWXYZ234"));
        assert!(!super::api_key_valid("ABCDEFGHJKLMNPQRSTUVWXYZ2345"));
        assert!(!super::api_key_valid(""));
    }

    /// The key travels in `Authorization: Bearer`, which is where qBittorrent
    /// 5.2.3 reads it from, and the session in `Cookie`. An empty session — the
    /// app that accepts with no login — sends neither, and the call goes through
    /// all the same.
    #[test]
    fn the_key_goes_in_the_bearer_header_and_the_session_in_the_cookie() {
        let http = reqwest::Client::new();
        let head = |a: super::QbitAuth| {
            a.apply(http.get("http://x/api/v2/app/version"))
                .build()
                .unwrap()
                .headers()
                .clone()
        };

        let h = head(super::QbitAuth::Key("qbt_ABCDEFGHJKLMNPQRSTUVWXYZ2345".into()));
        assert_eq!(
            h.get("authorization").unwrap(),
            "Bearer qbt_ABCDEFGHJKLMNPQRSTUVWXYZ2345"
        );
        assert!(h.get("cookie").is_none());

        let h = head(super::QbitAuth::Cookie("QBT_SID_8181=abc".into()));
        assert_eq!(h.get("cookie").unwrap(), "QBT_SID_8181=abc");
        assert!(h.get("authorization").is_none());

        let h = head(super::QbitAuth::Cookie(String::new()));
        assert!(h.get("cookie").is_none() && h.get("authorization").is_none());
    }

    #[test]
    fn the_client_categories_come_out_deduped_and_without_blanks() {
        let mut c = qbit();
        c.cats = serde_json::from_str(
            r#"{"sonarr":"tv-sonarr","sonarr-anime":"tv-sonarr","radarr":"radarr","lidarr":"  "}"#,
        )
        .unwrap();
        assert_eq!(c.categories(), vec!["radarr", "tv-sonarr"]);
    }

    #[test]
    fn prowlarr_alone_with_the_solver_is_already_work() {
        // a Prowlarr + FlareSolverr stack, with no *arr at all: there is something to
        // apply, because the indexer proxy belongs to Prowlarr, not to the *arr apps
        let mut req = Req {
            base: "http://127.0.0.1".into(),
            api_key: "k".into(),
            arrs: vec![],
            clients: vec![],
            prowlarr: Some(Prowlarr { route: "/prowlarr".into(), url: "http://p:9696".into() }),
            solver: Some(Solver { name: "FlareSolverr".into(), url: "http://f:8191".into() }),
            jellyfin: None,
            mm: Map::new(),
            configarr: None,
            search_lang: None,
            bazarr: None,
        };
        assert!(req.has_work());
        // with no solver and no client, Prowlarr alone has nothing to do
        req.solver = None;
        assert!(!req.has_work());
    }

    #[test]
    fn a_stack_with_no_arr_has_nothing_to_apply() {
        let req = |arrs: Vec<Arr>, clients: Vec<Client>| Req {
            base: "http://127.0.0.1".into(),
            api_key: "k".into(),
            arrs,
            clients,
            prowlarr: None,
            solver: None,
            jellyfin: None,
            mm: Map::new(),
            configarr: None,
            search_lang: None,
            bazarr: None,
        };
        assert!(!req(vec![], vec![qbit()]).has_work());
        assert!(!req(vec![arr("sonarr", "tvCategory")], vec![]).has_work());
        assert!(req(vec![arr("sonarr", "tvCategory")], vec![qbit()]).has_work());

        /* Except when the client brings a setting of its own: qBittorrent's
           preferences do not depend on any *arr, and a stack with only it was
           left without them — silently, which is the worst way of doing nothing. */
        let mut with_prefs = qbit();
        with_prefs.prefs = Some(
            serde_json::from_str(r#"{"auto_tmm_enabled":true}"#).unwrap(),
        );
        assert!(req(vec![], vec![with_prefs]).has_work());
        // empty preferences are no work
        let mut empty = qbit();
        empty.prefs = Some(Map::new());
        assert!(!req(vec![], vec![empty]).has_work());
    }

    /// Configarr alone in the stack is already reason for the round to happen: the
    /// profiles depend neither on a download client nor on Prowlarr.
    #[test]
    fn configarr_alone_is_already_reason_to_run() {
        let mut r = Req {
            base: "http://127.0.0.1".into(),
            api_key: "k".into(),
            arrs: vec![arr("sonarr", "tvCategory")],
            clients: vec![],
            prowlarr: None,
            solver: None,
            jellyfin: None,
            mm: Map::new(),
            configarr: Some(crate::deploy::Configarr {
                dir: "/cfg/configarr".into(),
                network: "starrnet".into(),
                user: "1000:1000".into(),
                tz: "America/Sao_Paulo".into(),
            }),
            search_lang: None,
            bazarr: None,
        };
        assert!(!r.has_work());
        assert!(r.configarr().is_some());
        // with no *arr there is nothing to apply a profile to, even with profiles chosen
        r.arrs = vec![];
        assert!(r.configarr().is_none());
    }

    /// `error()` builds a `Msg`, not text: what reaches the log is the key plus the
    /// app's own validation message, still untranslated (it is not ours to translate).
    #[test]
    fn the_arr_validation_message_reaches_the_log() {
        use crate::msg::Arg;

        let e = error(
            reqwest::StatusCode::BAD_REQUEST,
            r#"[{"errorMessage":"Unable to connect to qBittorrent"}]"#,
        );
        assert_eq!(e.key, "job.apply.httpErrorDetail");
        assert_eq!(e.args[0], Arg::Text("400".into()));
        assert!(matches!(&e.args[1], Arg::Text(t) if t.contains("Unable to connect")));

        let e = error(reqwest::StatusCode::NOT_FOUND, "não é json");
        assert_eq!(e.key, "job.apply.httpStatus");
        assert_eq!(e.args, vec![Arg::Text("404".into())]);
    }
}

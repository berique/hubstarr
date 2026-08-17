/* Hubstarr — the page's optional server.
   Copyright (C) 2025 Henrique Moreno
   Distributed under the GPL-3.0-or-later; see LICENSE at the project root.

   The page is still the product: it is the one that generates
   docker-compose.yml, .env and nginx.conf, and it goes on working opened from
   disk, with no server at all. What this binary adds is what the browser cannot
   reach on its own — keeping the stack between sessions, writing the files to
   disk and bringing the stack up.

   There is a single stack: the one in the --dir folder. No API path carries an
   id, and the database has no stacks table — switching stacks means pointing
   --dir and --db somewhere else.

   That is why it never generates content: it receives ready-made whatever the
   page built. That way the generators go on existing in a single place. */

mod apply;
mod deploy;
mod files;
mod jobs;
mod patch;
mod journal;
mod shots;
mod store;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use clap::Parser;
use serde::Deserialize;
use serde_json::{json, Value};

use jobs::Jobs;

/// The page, embedded in the binary: a single copy of the file at the root.
const PAGE: &str = include_str!("../../hubstarr.html");
const FAVICON: &[u8] = include_bytes!("../../favicon.ico");

#[derive(Parser, Debug)]
#[command(
    name = "hubstarr",
    version,
    about = "Servidor do Hubstarr: serve a página, guarda a stack e a sobe no Docker",
    long_about = "\
Servidor opcional do Hubstarr.

A página continua sendo o produto: é ela que gera o docker-compose.yml, o .env e
o nginx.conf, e aberta do disco funciona inteira, com o .zip e mais nada. Este
binário acrescenta o que o navegador não alcança sozinho — guardar a stack
entre sessões (em SQLite), gravar os arquivos em disco e rodar o docker compose.
Ele nunca gera conteúdo: recebe pronto o que os geradores da página montaram.

A página vem embutida no binário; abra o endereço de --addr no navegador.",
    after_help = "\
Exemplos:
  hubstarr                            atende só nesta máquina, em 127.0.0.1:7878
  hubstarr --addr 0.0.0.0:7878        atende também na rede local
  hubstarr --dir /srv/stack           põe os arquivos da stack em outro lugar
  hubstarr --docker podman            força o podman (sem isso, ele já é usado
                                      quando o docker não responde)
  hubstarr -v                         diz o passo a passo: arquivos, banco e as
                                      chamadas às APIs dos apps

Cuidado com o endereço: 127.0.0.1 é sempre a máquina em que o NAVEGADOR está
rodando. Se você navega de outro computador, use --addr 0.0.0.0:7878 e abra o
endereço de rede desta máquina, ou faça um túnel:

  ssh -N -L 7878:127.0.0.1:7878 usuario@esta-maquina

Abrir na rede dá a quem alcançar a porta o direito de rodar docker compose e
escrever arquivos aqui: não há autenticação nenhuma. O túnel não tem esse custo.

Documentação: README.pt-BR.md, seção \"Servidor (opcional)\".",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct Args {
    /// Address the server listens on
    #[arg(long, value_name = "IP:PORTA", default_value = "127.0.0.1:7878")]
    addr: SocketAddr,

    /// Folder the generated files are written to
    #[arg(long, value_name = "PASTA", default_value = "./stack")]
    dir: PathBuf,

    /// Database the stack is kept in
    #[arg(long, value_name = "ARQUIVO", default_value_os_t = default_db())]
    db: PathBuf,

    /// Docker command (default: docker, and podman when only it answers)
    #[arg(long, value_name = "COMANDO")]
    docker: Option<String>,

    /// Tells the step by step: every file written, every row touched in the
    /// database and every call to the stack apps' APIs
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Shows this help
    #[arg(short = 'h', long, action = clap::ArgAction::Help)]
    help: Option<bool>,

    /// Shows the version
    #[arg(short = 'V', long, action = clap::ArgAction::Version)]
    version: Option<bool>,
}

pub struct App {
    base: PathBuf,
    db_path: PathBuf,
    docker: String,
    jobs: Jobs,
    db: store::Db,
}

pub type Ctx = Arc<App>;

fn default_db() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".hubstarr/hubstarr.db")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let base = std::fs::canonicalize(&args.dir).unwrap_or_else(|_| {
        // it does not exist yet: resolve against the current directory without requiring it to exist
        std::env::current_dir().unwrap_or_default().join(&args.dir)
    });
    tokio::fs::create_dir_all(&base).await?;

    let db_path = args.db;
    let (db, migrated) = store::Db::open(&db_path)?;
    // the log lives next to the database, not in the stack folder: it belongs to
    // the server, not to the stack — `--dir` is wiped and remade, `--db` is what lasts
    journal::open(&db_path);
    journal::set_detail(args.verbose);
    if let Some(m) = migrated {
        journal::record("Banco migrado para o modelo de uma stack só (era o de várias).");
        journal::record(format!("  A stack que ficou gravava em {}", m.kept));
        for d in &m.dropped {
            journal::record(format!(
                "  Descartada a stack que gravava em {d} — os arquivos dela ficam onde estão"
            ));
        }
        if !m.dropped.is_empty() {
            journal::record("  Para editar uma delas, rode outro servidor com --dir e --db próprios.");
        }
    }
    let docker = deploy::pick_engine(args.docker).await;
    if docker != deploy::ENGINES[0] {
        journal::record(format!("Usando o {docker} para rodar o compose."));
    }
    let ctx: Ctx = Arc::new(App {
        base,
        db_path: db_path.clone(),
        docker,
        jobs: Jobs::new(),
        db,
    });

    let app = Router::new()
        .route("/", get(page))
        .route("/favicon.ico", get(favicon))
        .route("/api/health", get(health))
        .route("/api/state", get(load_state))
        .route("/api/settings", put(save_settings))
        .route("/api/instance", put(put_instance))
        .route("/api/instance/:key", delete(del_instance))
        .route("/api/files", post(write_files))
        .route("/api/deploy", post(start_deploy))
        .route("/api/down", post(start_down))
        .route("/api/service/:key/:action", post(start_service))
        .route("/api/config/apply", post(apply_config))
        .route("/api/shot/:app/:theme", get(shot))
        .route("/api/status", get(stack_status))
        .route("/api/job/:id", get(job_status))
        .route("/api/job/:id/stop", post(stop_job))
        .with_state(ctx.clone());

    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    if args.verbose {
        journal::record("Modo detalhado (-v): o passo a passo vai para a saída e para o log.");
    }
    journal::record(format!(
        "Hubstarr em http://{}  (stack em {}, banco em {}, log em {})",
        args.addr,
        ctx.base.display(),
        db_path.display(),
        journal::path(&db_path).display()
    ));
    axum::serve(listener, app).await?;
    Ok(())
}

/* ---------- page ---------- */

/// The page comes embedded in the binary, so it changes on every rebuild:
/// without `no-store` the browser serves the old copy from cache and the change vanishes.
async fn page() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        PAGE,
    )
}

async fn favicon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/x-icon")], FAVICON)
}

/// The palette screenshot, from the server cache. The page opened from disk
/// fetches straight from the theme.park documentation; with a server, it comes
/// through here — the first visit goes to the network and the following ones come from disk.
async fn shot(State(ctx): State<Ctx>, Path((app, theme)): Path<(String, String)>) -> Response {
    match shots::fetch(&shots::cache_dir(&ctx.db_path), &app, &theme).await {
        Ok(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "max-age=86400"),
            ],
            png,
        )
            .into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e).into_response(),
    }
}

/// Applies the Configuration to the apps already up. Like bringing the stack
/// up, it is a numbered job: it is several API calls, and each app that answers
/// becomes a line of the log.
async fn apply_config(State(ctx): State<Ctx>, Json(req): Json<apply::Req>) -> Response {
    let job = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move {
            let cfgarr = req.configarr();
            let mut error = None;
            if req.has_work() {
                error = apply::download_clients(req, log.clone()).await.err();
            } else if cfgarr.is_some() {
                apply::wait(&req, &log).await?;
            }
            /* And the quality profiles, which are files — Configarr reads the
               `config.yml` the page generated and writes into the apps. A link
               that did not go through does not cancel this: they are
               independent things, and losing the profiles because one app was
               down is out of proportion. The error comes back at the end,
               without getting lost. */
            if let Some(c) = cfgarr {
                deploy::configarr(&ctx.docker, &c, &log).await?;
            }
            match error {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    });
    Json(json!({"ok": true, "job": job})).into_response()
}

async fn health(State(ctx): State<Ctx>) -> Json<Value> {
    let (puid, pgid) = server_user();
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "dir": ctx.base.display().to_string(),
        "db": ctx.db_path.display().to_string(),
        "docker": deploy::docker_ok(&ctx.docker).await,
        // the owner of the folders it creates: the PUID/PGID the apps need to have
        "puid": puid,
        "pgid": pgid,
    }))
}

/// The user and group the server runs as — its `id -u` and `id -g`.
///
/// It comes from the owner of `/proc/self`, which is the process itself: it
/// avoids depending on a crate just to call `getuid()`. Where that path does
/// not exist, the answer is `None` and the page keeps its own default.
fn server_user() -> (Option<u32>, Option<u32>) {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata("/proc/self") {
        Ok(m) => (Some(m.uid()), Some(m.gid())),
        Err(_) => (None, None),
    }
}


/* ---------- the stack ---------- */

/// Returns the stored state, or 204 when there is nothing in the database yet.
async fn load_state(State(ctx): State<Ctx>) -> Response {
    match ctx.db.load() {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => fail(&e),
    }
}

/// What holds for the whole stack: the Environment, the Configuration and the
/// list of keys, which fixes the order and deletes what left without going through the modal.
#[derive(Deserialize)]
struct Settings {
    #[serde(default)]
    defaults: Option<Value>,
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    keys: Option<Vec<String>>,
}

/* This is the only path that deletes an instance without anyone having clicked
   "Delete": the list of keys rules, and whatever does not come in it goes away.
   A page with the wrong list — one that could not read the state, or an old tab
   coming back to life — wipes the stack through here, and with no log there is
   no way of knowing afterwards who sent what. So every PUT leaves a line on the
   server output, and the line says how many keys came and **which ones left**. */
async fn save_settings(State(ctx): State<Ctx>, Json(s): Json<Settings>) -> Response {
    let done = (|| -> Result<Vec<String>, String> {
        if let Some(v) = &s.defaults {
            ctx.db.put_env(v)?;
        }
        if let Some(v) = &s.config {
            ctx.db.put_config(v)?;
        }
        match &s.keys {
            Some(k) => ctx.db.reconcile(k),
            None => Ok(vec![]),
        }
    })();
    let how_many = s.keys.as_ref().map(|k| k.len());
    match done {
        Ok(left) => {
            let list = match how_many {
                Some(n) => format!("{n} chave(s)"),
                None => "sem lista de chaves".to_string(),
            };
            let removed = if left.is_empty() {
                String::new()
            } else {
                format!(" — apagou {}", left.join(", "))
            };
            journal::record(format!("{} PUT /api/settings: {list}{removed}", journal::stamp()));
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => {
            journal::record(format!("{} PUT /api/settings: falhou ({e})", journal::stamp()));
            fail(&e)
        }
    }
}

/// A service added or edited: a single row, created or updated.
async fn put_instance(State(ctx): State<Ctx>, Json(inc): Json<store::InstanceIn>) -> Response {
    match ctx.db.put_instance(&inc) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(&e),
    }
}

async fn del_instance(State(ctx): State<Ctx>, Path(key): Path<String>) -> Response {
    match ctx.db.delete_instance(&key) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(&e),
    }
}

/* ---------- files and deploy ---------- */

/// What the page sends: the files, already finished. The state does not come
/// here — it is written on every add, edit or delete, not only at deploy time.
#[derive(Deserialize)]
pub struct Payload {
    #[serde(default)]
    pub files: Vec<files::OutFile>,
    /// folders for the compose `source` entries, to be created before coming up
    #[serde(default)]
    pub dirs: Vec<String>,
    /// keys to write into the configuration the app itself creates, after coming up
    #[serde(default)]
    pub patches: Vec<patch::Patch>,
    /// the Configuration, applied as soon as the apps answer
    #[serde(default)]
    pub config: Option<apply::Req>,
}

async fn write_files(State(ctx): State<Ctx>, Json(p): Json<Payload>) -> Response {
    let cfg = match config_base(&ctx) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    match files::write_all(&ctx.base, cfg.as_deref(), &p.files).await {
        Ok(names) => {
            Json(json!({"ok": true, "dir": ctx.base.display().to_string(), "files": names}))
                .into_response()
        }
        Err(e) => fail(&e),
    }
}

/// Writes the files and brings the stack up. Returns the job number right
/// away: `docker compose up` pulls images, and that does not fit in an HTTP response.
async fn start_deploy(State(ctx): State<Ctx>, Json(p): Json<Payload>) -> Response {
    let cfg = match config_base(&ctx) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    if let Err(e) = files::write_all(&ctx.base, cfg.as_deref(), &p.files).await {
        return fail(&e);
    }
    let job = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move {
            /* Before coming up: the folders the compose binds expect. The one that
               lists them is the page, which is the one building the paths;
               Docker would create the missing ones, but as root — and then the
               app, running with the Environment's PUID/PGID, would not write
               into its own configuration. */
            files::ensure_dirs(&p.dirs, &log).await?;
            deploy::up(&ctx.docker, &ctx.base, log.clone()).await?;
            /* Only after the stack comes up: the configuration we are going to
               touch is the one the app itself creates, and before the first
               `up` it does not exist. */
            let cfg = config_base(&ctx)?;
            patch::apply_all(&ctx.docker, &ctx.base, cfg.as_deref(), &p.patches, &log).await?;
            /* And, with the stack up, the Configuration: Prowlarr learns the *arr
               apps and the download clients, and each *arr learns the clients.
               The qBittorrent credentials were written above — that is what
               lets the *arr talk to it when validating the registration. */
            match p.config {
                Some(cfg) => {
                    let cfgarr = cfg.configarr();
                    let mut error = None;
                    if cfg.has_work() {
                        error = apply::download_clients(cfg, log.clone()).await.err();
                    } else if cfgarr.is_some() {
                        // nothing to configure through the API, but the apps still
                        // need to be up for Configarr to write into them
                        apply::wait(&cfg, &log).await?;
                    }
                    /* And, last, the quality profiles: Configarr runs once and exits,
                       and depends on the apps being up — which is why it comes
                       here, and not in the `up` above. A link that did not go
                       through does not cancel it: one does not depend on the other. */
                    if let Some(c) = cfgarr {
                        deploy::configarr(&ctx.docker, &c, &log).await?;
                    }
                    match error {
                        Some(e) => Err(e),
                        None => Ok(()),
                    }
                }
                None => Ok(()),
            }
        }
    });
    Json(json!({"ok": true, "job": job})).into_response()
}

async fn start_down(State(ctx): State<Ctx>) -> Response {
    let job = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move { deploy::down(&ctx.docker, &ctx.base, log).await }
    });
    Json(json!({"ok": true, "job": job})).into_response()
}

/// A single container, brought up or stopped — it is the click on the list's status dot.
/// Nothing is written here: the `up` of one service uses the compose already
/// in the folder, so whoever has never brought the whole stack up once has
/// nothing to bring up. The key is the page's `cname()`, which is the service
/// name in the compose; it is checked before becoming a command argument.
async fn start_service(State(ctx): State<Ctx>, Path((key, action)): Path<(String, String)>) -> Response {
    if !deploy::ok_service(&key) {
        return fail("nome de serviço inválido");
    }
    let up = match action.as_str() {
        "up" => true,
        "down" => false,
        _ => return fail("ação desconhecida"),
    };
    if !ctx.base.join("docker-compose.yml").exists() {
        return fail("a stack ainda não foi gravada nesta pasta");
    }
    let job = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move {
            if up {
                deploy::up_one(&ctx.docker, &ctx.base, &key, log).await
            } else {
                deploy::stop_one(&ctx.docker, &ctx.base, &key, log).await
            }
        }
    });
    Json(json!({"ok": true, "job": job})).into_response()
}

/// The state of each container of the stack. The page asks every so often to
/// paint the status dot of each service in the list.
async fn stack_status(State(ctx): State<Ctx>) -> Response {
    match deploy::status(&ctx.docker, &ctx.base).await {
        Ok(v) => Json(json!({"ok": true, "services": v})).into_response(),
        Err(e) => fail(&e),
    }
}

async fn job_status(State(ctx): State<Ctx>, Path(id): Path<u64>) -> Response {
    match ctx.jobs.get(id) {
        Some(j) => Json(j).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "trabalho desconhecido"})),
        )
            .into_response(),
    }
}

/// The log modal's Stop. Killing the job halfway leaves the stack however it
/// happens to be — half-started containers, half-applied configuration — so
/// that is recorded like any other change.
async fn stop_job(State(ctx): State<Ctx>, Path(id): Path<u64>) -> Response {
    if ctx.jobs.stop(id) {
        journal::record(format!("trabalho {id} parado a pedido"));
        Json(json!({"ok": true})).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "trabalho desconhecido"})),
        )
            .into_response()
    }
}

/* ---------- helpers ---------- */

/// The root of the configurations, which comes from the stored Environment and
/// never from what the browser sends: that way nothing writes outside what the stack itself declared.
fn config_base(ctx: &Ctx) -> Result<Option<PathBuf>, String> {
    Ok(ctx.db.config_base()?.map(PathBuf::from))
}

type Response = axum::response::Response;

fn fail(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"ok": false, "error": msg})),
    )
        .into_response()
}

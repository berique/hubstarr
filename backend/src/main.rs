/* Hubstarr — servidor opcional da página.
   Copyright (C) 2025 Henrique Moreno
   Distribuído sob a GPL-3.0-or-later; veja o LICENSE na raiz do projeto.

   A página continua sendo o produto: ela é que gera docker-compose.yml, .env e
   nginx.conf, e continua funcionando aberta do disco, sem servidor nenhum. O
   que este binário acrescenta é o que o navegador não alcança sozinho — gravar
   os arquivos em disco, subir a stack, guardar o estado entre sessões e falar
   com a API dos *arr depois que eles estão de pé.

   Por isso ele nunca gera conteúdo: recebe pronto o que a página montou. Assim
   os geradores continuam existindo num lugar só. */

mod arr;
mod deploy;
mod files;
mod jobs;
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

/// A página, embutida no binário: uma cópia só do arquivo que está na raiz.
const PAGE: &str = include_str!("../../hubstarr.html");
const FAVICON: &[u8] = include_bytes!("../../favicon.ico");

#[derive(Parser, Debug)]
#[command(name = "hubstarr", version, about = "Servidor do Hubstarr")]
struct Args {
    /// Endereço em que o servidor atende
    #[arg(long, default_value = "127.0.0.1:7878")]
    addr: SocketAddr,

    /// Pasta em que os arquivos gerados são gravados
    #[arg(long, default_value = "./stack")]
    dir: PathBuf,

    /// Comando do docker (para quem usa podman ou um wrapper)
    #[arg(long, default_value = "docker")]
    docker: String,
}

pub struct App {
    dir: PathBuf,
    docker: String,
    jobs: Jobs,
    db: store::Db,
}

pub type Ctx = Arc<App>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let dir = std::fs::canonicalize(&args.dir).unwrap_or_else(|_| {
        // ainda não existe: resolve contra o diretório atual sem exigir que exista
        std::env::current_dir().unwrap_or_default().join(&args.dir)
    });
    tokio::fs::create_dir_all(&dir).await?;

    let db = store::Db::open(&dir)?;
    let ctx: Ctx = Arc::new(App {
        dir,
        docker: args.docker,
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
        .route("/api/configure", post(start_configure))
        .route("/api/job/:id", get(job_status))
        .with_state(ctx.clone());

    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    println!("Hubstarr em http://{}  (arquivos em {})", args.addr, ctx.dir.display());
    axum::serve(listener, app).await?;
    Ok(())
}

/* ---------- página ---------- */

async fn page() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], PAGE)
}

async fn favicon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/x-icon")], FAVICON)
}

/* ---------- estado ---------- */

async fn health(State(ctx): State<Ctx>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "dir": ctx.dir.display().to_string(),
        "docker": deploy::docker_ok(&ctx.docker).await,
        "saved": ctx.db.has_stack(),
    }))
}

/// Devolve o estado guardado, ou 204 quando o banco ainda está vazio.
async fn load_state(State(ctx): State<Ctx>) -> Response {
    match ctx.db.load() {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => fail(&e),
    }
}

/// O que vale para a stack inteira: o Ambiente, a Configuração e a lista de
/// chaves, que acerta a ordem e apaga o que saiu sem passar pelo modal.
#[derive(Deserialize)]
struct Settings {
    #[serde(default)]
    defaults: Option<Value>,
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    keys: Option<Vec<String>>,
}

async fn save_settings(State(ctx): State<Ctx>, Json(s): Json<Settings>) -> Response {
    let done = (|| {
        if let Some(v) = &s.defaults {
            ctx.db.put_setting("defaults", v)?;
        }
        if let Some(v) = &s.config {
            ctx.db.put_setting("config", v)?;
        }
        if let Some(k) = &s.keys {
            ctx.db.reconcile(k)?;
        }
        Ok::<(), String>(())
    })();
    match done {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(&e),
    }
}

/// Um serviço adicionado ou editado: uma linha só, criada ou atualizada.
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

/* ---------- arquivos e deploy ---------- */

/// O que a página manda: os arquivos já prontos. O estado não vem aqui — ele
/// é gravado a cada adicionar, editar ou excluir, não só na hora do deploy.
#[derive(Deserialize)]
pub struct Payload {
    #[serde(default)]
    pub files: Vec<files::OutFile>,
    /// plano de configuração dos *arr, quando a página pede para aplicar
    #[serde(default)]
    pub plan: Option<arr::Plan>,
}

async fn write_files(State(ctx): State<Ctx>, Json(p): Json<Payload>) -> Response {
    match files::write_all(&ctx.dir, &p.files).await {
        Ok(names) => Json(json!({"ok": true, "dir": ctx.dir.display().to_string(), "files": names}))
            .into_response(),
        Err(e) => fail(&e),
    }
}

/// Grava os arquivos e sobe a stack. Devolve na hora o número do trabalho: o
/// `docker compose up` baixa imagem, e isso não cabe numa resposta HTTP.
async fn start_deploy(State(ctx): State<Ctx>, Json(p): Json<Payload>) -> Response {
    if let Err(e) = files::write_all(&ctx.dir, &p.files).await {
        return fail(&e);
    }
    let id = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move { deploy::up(&ctx.docker, &ctx.dir, log).await }
    });
    Json(json!({"ok": true, "job": id})).into_response()
}

async fn start_down(State(ctx): State<Ctx>) -> Response {
    let id = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move { deploy::down(&ctx.docker, &ctx.dir, log).await }
    });
    Json(json!({"ok": true, "job": id})).into_response()
}

/// Aplica no Prowlarr e nos *arr o que a Configuração descreve. Também é um
/// trabalho: os apps demoram a responder depois de subir.
async fn start_configure(State(ctx): State<Ctx>, Json(p): Json<Payload>) -> Response {
    let Some(plan) = p.plan else {
        return fail("faltou o plano de configuração");
    };
    let id = ctx.jobs.spawn(move |log| async move { arr::apply(plan, log).await });
    Json(json!({"ok": true, "job": id})).into_response()
}

async fn job_status(State(ctx): State<Ctx>, Path(id): Path<u64>) -> Response {
    match ctx.jobs.get(id) {
        Some(j) => Json(j).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "trabalho desconhecido"})))
            .into_response(),
    }
}

/* ---------- utilidades ---------- */

type Response = axum::response::Response;

fn fail(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"ok": false, "error": msg})),
    )
        .into_response()
}

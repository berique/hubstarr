/* Hubstarr — servidor opcional da página.
   Copyright (C) 2025 Henrique Moreno
   Distribuído sob a GPL-3.0-or-later; veja o LICENSE na raiz do projeto.

   A página continua sendo o produto: ela é que gera docker-compose.yml, .env e
   nginx.conf, e continua funcionando aberta do disco, sem servidor nenhum. O
   que este binário acrescenta é o que o navegador não alcança sozinho — guardar
   a stack entre sessões, gravar os arquivos em disco e subir a stack.

   A stack é uma só: a da pasta do --dir. Nenhum caminho da API leva id, e o
   banco não tem tabela de stacks — trocar de stack é apontar o --dir e o --db
   para outro lugar.

   Por isso ele nunca gera conteúdo: recebe pronto o que a página montou. Assim
   os geradores continuam existindo num lugar só. */

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
  hubstarr --docker podman            usa o podman no lugar do docker

Cuidado com o endereço: 127.0.0.1 é sempre a máquina em que o NAVEGADOR está
rodando. Se você navega de outro computador, use --addr 0.0.0.0:7878 e abra o
endereço de rede desta máquina, ou faça um túnel:

  ssh -N -L 7878:127.0.0.1:7878 usuario@esta-maquina

Abrir na rede dá a quem alcançar a porta o direito de rodar docker compose e
escrever arquivos aqui: não há autenticação nenhuma. O túnel não tem esse custo.

Documentação: README.md, seção \"Servidor (opcional)\".",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct Args {
    /// Endereço em que o servidor atende
    #[arg(long, value_name = "IP:PORTA", default_value = "127.0.0.1:7878")]
    addr: SocketAddr,

    /// Pasta em que os arquivos gerados são gravados
    #[arg(long, value_name = "PASTA", default_value = "./stack")]
    dir: PathBuf,

    /// Banco em que a stack é guardada
    #[arg(long, value_name = "ARQUIVO", default_value_os_t = default_db())]
    db: PathBuf,

    /// Comando do docker (para quem usa podman ou um wrapper)
    #[arg(long, value_name = "COMANDO", default_value = "docker")]
    docker: String,

    /// Mostra esta ajuda
    #[arg(short = 'h', long, action = clap::ArgAction::Help)]
    help: Option<bool>,

    /// Mostra a versão
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
        // ainda não existe: resolve contra o diretório atual sem exigir que exista
        std::env::current_dir().unwrap_or_default().join(&args.dir)
    });
    tokio::fs::create_dir_all(&base).await?;

    let db_path = args.db;
    let (db, migrated) = store::Db::open(&db_path)?;
    if let Some(m) = migrated {
        println!("Banco migrado para o modelo de uma stack só (era o de várias).");
        println!("  A stack que ficou gravava em {}", m.kept);
        for d in &m.dropped {
            println!("  Descartada a stack que gravava em {d} — os arquivos dela ficam onde estão");
        }
        if !m.dropped.is_empty() {
            println!("  Para editar uma delas, rode outro servidor com --dir e --db próprios.");
        }
    }
    let ctx: Ctx = Arc::new(App {
        base,
        db_path: db_path.clone(),
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
        .route("/api/status", get(stack_status))
        .route("/api/job/:id", get(job_status))
        .with_state(ctx.clone());

    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    println!(
        "Hubstarr em http://{}  (stack em {}, banco em {})",
        args.addr,
        ctx.base.display(),
        db_path.display()
    );
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

async fn health(State(ctx): State<Ctx>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "dir": ctx.base.display().to_string(),
        "db": ctx.db_path.display().to_string(),
        "docker": deploy::docker_ok(&ctx.docker).await,
    }))
}

/* ---------- a stack ---------- */

/// Devolve o estado guardado, ou 204 quando ainda não há nada no banco.
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
    let done = (|| -> Result<(), String> {
        if let Some(v) = &s.defaults {
            ctx.db.put_env(v)?;
        }
        if let Some(v) = &s.config {
            ctx.db.put_config(v)?;
        }
        if let Some(k) = &s.keys {
            ctx.db.reconcile(k)?;
        }
        Ok(())
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

/// O que a página manda: os arquivos já prontos. O estado não vem aqui — ele é
/// gravado a cada adicionar, editar ou excluir, não só na hora do deploy.
#[derive(Deserialize)]
pub struct Payload {
    #[serde(default)]
    pub files: Vec<files::OutFile>,
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

/// Grava os arquivos e sobe a stack. Devolve na hora o número do trabalho: o
/// `docker compose up` baixa imagem, e isso não cabe numa resposta HTTP.
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
        move |log| async move { deploy::up(&ctx.docker, &ctx.base, log).await }
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

/// O estado de cada container da stack. A página pergunta de tempos em tempos
/// para pintar o ponto de status de cada serviço da lista.
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

/* ---------- utilidades ---------- */

/// A raiz das configurações, que vem do Ambiente guardado e nunca do que o
/// navegador manda: assim nada escreve fora do que a própria stack declarou.
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

/* Hubstarr — servidor opcional da página.
   Copyright (C) 2025 Henrique Moreno
   Distribuído sob a GPL-3.0-or-later; veja o LICENSE na raiz do projeto.

   A página continua sendo o produto: ela é que gera docker-compose.yml, .env e
   nginx.conf, e continua funcionando aberta do disco, sem servidor nenhum. O
   que este binário acrescenta é o que o navegador não alcança sozinho — guardar
   as stacks entre sessões, gravar os arquivos em disco e subir a stack.

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
    about = "Servidor do Hubstarr: serve a página, guarda as stacks e sobe a stack no Docker",
    long_about = "\
Servidor opcional do Hubstarr.

A página continua sendo o produto: é ela que gera o docker-compose.yml, o .env e
o nginx.conf, e aberta do disco funciona inteira, com o .zip e mais nada. Este
binário acrescenta o que o navegador não alcança sozinho — guardar as stacks
entre sessões (em SQLite), gravar os arquivos em disco e rodar o docker compose.
Ele nunca gera conteúdo: recebe pronto o que os geradores da página montaram.

A página vem embutida no binário; abra o endereço de --addr no navegador.",
    after_help = "\
Exemplos:
  hubstarr                            atende só nesta máquina, em 127.0.0.1:7878
  hubstarr --addr 0.0.0.0:7878        atende também na rede local
  hubstarr --dir /srv/stacks          põe as pastas das stacks em outro lugar
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

    /// Pasta em que cada stack ganha a sua, com os arquivos gerados
    #[arg(long, value_name = "PASTA", default_value = "./stacks")]
    dir: PathBuf,

    /// Banco em que as stacks são guardadas
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
    let db = store::Db::open(&db_path)?;
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
        .route("/api/stacks", get(list_stacks).post(create_stack))
        .route("/api/stacks/:id", delete(del_stack))
        .route("/api/stacks/:id/state", get(load_state))
        .route("/api/stacks/:id/settings", put(save_settings))
        .route("/api/stacks/:id/instance", put(put_instance))
        .route("/api/stacks/:id/instance/:key", delete(del_instance))
        .route("/api/stacks/:id/files", post(write_files))
        .route("/api/stacks/:id/deploy", post(start_deploy))
        .route("/api/stacks/:id/down", post(start_down))
        .route("/api/stacks/:id/status", get(stack_status))
        .route("/api/job/:id", get(job_status))
        .with_state(ctx.clone());

    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    println!(
        "Hubstarr em http://{}  (stacks em {}, banco em {})",
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

/* ---------- stacks ---------- */

async fn list_stacks(State(ctx): State<Ctx>) -> Response {
    match ctx.db.stacks() {
        Ok(v) => Json(v).into_response(),
        Err(e) => fail(&e),
    }
}

/// Cria a stack e a pasta dela. A pasta sai do slug do nome, para quem for
/// olhar no disco reconhecer qual é qual.
async fn create_stack(State(ctx): State<Ctx>, Json(n): Json<store::stack::NewStack>) -> Response {
    let row = match ctx.db.create_stack(&n.name, &ctx.base) {
        Ok(r) => r,
        Err(e) => return fail(&e),
    };
    if let Err(e) = tokio::fs::create_dir_all(&row.dir).await {
        return fail(&format!("{}: {e}", row.dir));
    }
    Json(row).into_response()
}

async fn del_stack(State(ctx): State<Ctx>, Path(id): Path<i64>) -> Response {
    match ctx.db.delete_stack(id) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(&e),
    }
}

/// Devolve o estado guardado, ou 204 quando a stack não existe mais.
async fn load_state(State(ctx): State<Ctx>, Path(id): Path<i64>) -> Response {
    match ctx.db.load(id) {
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

async fn save_settings(
    State(ctx): State<Ctx>,
    Path(id): Path<i64>,
    Json(s): Json<Settings>,
) -> Response {
    let done = (|| {
        if let Some(v) = &s.defaults {
            ctx.db.put_env(id, v)?;
        }
        if let Some(v) = &s.config {
            ctx.db.put_config(id, v)?;
        }
        if let Some(k) = &s.keys {
            ctx.db.reconcile(id, k)?;
        }
        ctx.db.touch(id)
    })();
    match done {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(&e),
    }
}

/// Um serviço adicionado ou editado: uma linha só, criada ou atualizada.
async fn put_instance(
    State(ctx): State<Ctx>,
    Path(id): Path<i64>,
    Json(inc): Json<store::InstanceIn>,
) -> Response {
    match ctx.db.put_instance(id, &inc).and_then(|()| ctx.db.touch(id)) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(&e),
    }
}

async fn del_instance(State(ctx): State<Ctx>, Path((id, key)): Path<(i64, String)>) -> Response {
    match ctx.db.delete_instance(id, &key).and_then(|()| ctx.db.touch(id)) {
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

async fn write_files(
    State(ctx): State<Ctx>,
    Path(id): Path<i64>,
    Json(p): Json<Payload>,
) -> Response {
    let (dir, cfg) = match stack_paths(&ctx, id) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    match files::write_all(&dir, cfg.as_deref(), &p.files).await {
        Ok(names) => {
            Json(json!({"ok": true, "dir": dir.display().to_string(), "files": names}))
                .into_response()
        }
        Err(e) => fail(&e),
    }
}

/// Grava os arquivos e sobe a stack. Devolve na hora o número do trabalho: o
/// `docker compose up` baixa imagem, e isso não cabe numa resposta HTTP.
async fn start_deploy(
    State(ctx): State<Ctx>,
    Path(id): Path<i64>,
    Json(p): Json<Payload>,
) -> Response {
    let (dir, cfg) = match stack_paths(&ctx, id) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    if let Err(e) = files::write_all(&dir, cfg.as_deref(), &p.files).await {
        return fail(&e);
    }
    let job = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move { deploy::up(&ctx.docker, &dir, log).await }
    });
    Json(json!({"ok": true, "job": job})).into_response()
}

async fn start_down(State(ctx): State<Ctx>, Path(id): Path<i64>) -> Response {
    let dir = match ctx.db.stack_dir(id) {
        Ok(d) => PathBuf::from(d),
        Err(e) => return fail(&e),
    };
    let job = ctx.jobs.spawn({
        let ctx = ctx.clone();
        move |log| async move { deploy::down(&ctx.docker, &dir, log).await }
    });
    Json(json!({"ok": true, "job": job})).into_response()
}

/// O estado de cada container da stack. A página pergunta de tempos em tempos
/// para pintar o ponto de status de cada serviço da lista.
async fn stack_status(State(ctx): State<Ctx>, Path(id): Path<i64>) -> Response {
    let dir = match ctx.db.stack_dir(id) {
        Ok(d) => PathBuf::from(d),
        Err(e) => return fail(&e),
    };
    match deploy::status(&ctx.docker, &dir).await {
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

/// A pasta da stack e a raiz das configurações. A segunda vem do Ambiente
/// guardado, nunca do que o navegador manda: assim nada escreve fora do que a
/// própria stack declarou.
fn stack_paths(ctx: &Ctx, id: i64) -> Result<(PathBuf, Option<PathBuf>), String> {
    let dir = PathBuf::from(ctx.db.stack_dir(id)?);
    let cfg = ctx.db.config_base(id)?.map(PathBuf::from);
    Ok((dir, cfg))
}

type Response = axum::response::Response;

fn fail(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"ok": false, "error": msg})),
    )
        .into_response()
}

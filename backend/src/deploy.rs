/* Chamada ao docker compose.

   O compose é rodado com a pasta dos arquivos como diretório do projeto, para
   que os caminhos relativos do compose e o `.env` que está ali sejam achados
   como seriam se alguém tivesse rodado o comando à mão naquela pasta. */

use std::path::Path;
use std::process::Stdio;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::jobs::Log;

/// O `docker compose` responde? É o que a página usa para avisar antes de
/// tentar subir. Pergunta pelo plugin, não só pelo docker: é o `compose` que
/// sobe a stack, e ele é um pacote à parte que pode faltar num docker que
/// está lá e funcionando.
pub async fn docker_ok(docker: &str) -> bool {
    Command::new(docker)
        .args(["compose", "version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/* O estado de cada container da stack, para o ponto de status da lista.

   O `compose ps` é lido com `--format json`, que sai como uma linha por
   container (nas versões mais novas) ou um array só (nas antigas) — os dois
   casos entram aqui. `--all` é o que faz o container parado aparecer: sem ele,
   parado e inexistente ficariam iguais, e é justamente a diferença que o ponto
   mostra.

   A chave é o `Service` do compose, que é o `cname()` da página; quem não
   aparecer na resposta é porque nunca foi criado. */
pub async fn status(docker: &str, dir: &Path) -> Result<Value, String> {
    let out = Command::new(docker)
        .args(["compose", "ps", "--format", "json", "--all"])
        .current_dir(dir)
        .output()
        .await
        .map_err(|e| format!("não consegui rodar o {docker}: {e}"))?;
    // pasta sem compose ainda, ou docker fora do ar: ninguém está no ar, e
    // isso não é erro — a página só não pinta ponto nenhum
    if !out.status.success() {
        return Ok(json!({}));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows: Vec<Value> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(a)) => rows.extend(a),
            Ok(v) => rows.push(v),
            Err(_) => {}
        }
    }

    let mut map = serde_json::Map::new();
    for r in rows {
        let key = r
            .get("Service")
            .or_else(|| r.get("Name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if key.is_empty() {
            continue;
        }
        map.insert(
            key.to_string(),
            json!({
                "state":  r.get("State").and_then(|v| v.as_str()).unwrap_or(""),
                "status": r.get("Status").and_then(|v| v.as_str()).unwrap_or(""),
                "health": r.get("Health").and_then(|v| v.as_str()).unwrap_or(""),
            }),
        );
    }
    Ok(Value::Object(map))
}

pub async fn up(docker: &str, dir: &Path, log: Log) -> Result<(), String> {
    run(docker, &["compose", "up", "-d", "--remove-orphans"], dir, &log).await
}

pub async fn down(docker: &str, dir: &Path, log: Log) -> Result<(), String> {
    run(docker, &["compose", "down"], dir, &log).await
}

/// Nome de serviço que pode virar argumento do compose. O `cname()` da página
/// só produz minúsculas, dígitos e hífen; o que fugir disso não veio de lá e
/// não entra na linha de comando. Hífen no começo é recusado à parte: seria
/// nome válido pela regra das letras, mas o compose o leria como opção.
pub fn ok_service(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('-')
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Sobe um container só, sem tocar nos outros. `--no-deps` é o que mantém a
/// promessa do clique: quem está parado ao lado continua parado.
pub async fn up_one(docker: &str, dir: &Path, key: &str, log: Log) -> Result<(), String> {
    run(docker, &["compose", "up", "-d", "--no-deps", key], dir, &log).await
}

/// Para um container só. É `stop`, não `down`: o `down` derruba a stack
/// inteira, e aqui o que se quer é o inverso — só aquele serviço sai do ar, e
/// o container continua existindo para o ponto voltar a "parado" em vez de
/// "não criado".
pub async fn stop_one(docker: &str, dir: &Path, key: &str, log: Log) -> Result<(), String> {
    run(docker, &["compose", "stop", key], dir, &log).await
}

/// O Configarr, que aplica os perfis de qualidade e os custom formats do TRaSH
/// Guides no que a página escreveu no `config.yml` dele.
///
/// É `run --rm`, não `up`: ele roda uma vez e sai, e por isso a página o deixa
/// atrás de um profile do compose — no `up -d` ele subiria antes de os apps
/// responderem. Quem chama espera os apps primeiro, como o resto do `apply`.
pub async fn configarr(docker: &str, dir: &Path, log: &Log) -> Result<(), String> {
    log.line("aplicando os perfis do TRaSH Guides (configarr)…");
    // `-T` porque o servidor não tem terminal: sem ele o `run` tenta alocar um
    // TTY e morre antes de o Configarr começar
    compose(docker, &["run", "--rm", "-T", "configarr"], dir, log).await
}

/// Um `docker compose <args>` qualquer na pasta da stack — é como o `patch.rs`
/// para e sobe um container só, em volta da edição da configuração dele.
pub async fn compose(docker: &str, args: &[&str], dir: &Path, log: &Log) -> Result<(), String> {
    let mut todos = vec!["compose"];
    todos.extend_from_slice(args);
    run(docker, &todos, dir, log).await
}

/// Roda o comando na pasta da stack copiando as duas saídas para o log,
/// linha a linha — o compose escreve o progresso na stderr.
async fn run(docker: &str, args: &[&str], dir: &Path, log: &Log) -> Result<(), String> {
    log.line(format!("$ {docker} {}", args.join(" ")));

    let mut child = Command::new(docker)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("não consegui rodar o {docker}: {e}"))?;

    let out = child.stdout.take().map(|h| pipe(h, log.clone()));
    let err = child.stderr.take().map(|h| pipe(h, log.clone()));
    if let (Some(a), Some(b)) = (out, err) {
        let _ = tokio::join!(a, b);
    }

    let st = child.wait().await.map_err(|e| e.to_string())?;
    if st.success() {
        log.line("pronto.");
        Ok(())
    } else {
        Err(format!("{docker} terminou com {st}"))
    }
}

async fn pipe<R>(handle: R, log: Log)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(handle).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        log.line(line);
    }
}

#[cfg(test)]
mod tests {
    use super::ok_service;

    #[test]
    fn nome_de_servico_so_aceita_o_que_o_cname_produz() {
        assert!(ok_service("sonarr-4k"));
        assert!(!ok_service(""));
        assert!(!ok_service("--rmi")); // o compose leria isso como opção
        assert!(!ok_service("sonarr; rm -rf /"));
        assert!(!ok_service("../fora"));
        assert!(!ok_service("Sonarr"));
    }
}

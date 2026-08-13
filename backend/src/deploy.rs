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
/// está lá e funcionando. Vale igual para o `podman compose`.
/// Os motores que sabemos rodar, na ordem em que são tentados quando ninguém
/// passou `--docker`. O `podman` entra aqui porque o `podman compose` roda o
/// mesmo arquivo — quem tem só ele instalado não tem `docker` nenhum a
/// encontrar, e a página abriria o aviso de "instale o Docker" com a máquina
/// pronta para subir a stack.
pub const ENGINES: [&str; 2] = ["docker", "podman"];

/// Qual motor usar: o que veio na linha de comando, se veio, senão o primeiro
/// dos `ENGINES` que responder. Sem nenhum, fica o `docker` — é o que a
/// mensagem do `docker_ok()` na página fala de instalar.
pub async fn pick_engine(escolhido: Option<String>) -> String {
    if let Some(c) = escolhido {
        return c;
    }
    for e in ENGINES {
        if docker_ok(e).await {
            return e.to_string();
        }
    }
    ENGINES[0].to_string()
}

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

/// O que a página manda para rodar o Configarr: caminhos, rede e usuário — as
/// decisões continuam lá, aqui só se monta a linha de comando.
#[derive(serde::Deserialize, Clone)]
pub struct Configarr {
    /// pasta dele no host, com o `config.yml`, o `secrets.yml`, os custom
    /// formats e o cache dos repositórios
    pub dir: String,
    /// a rede da stack, por onde ele alcança os *arr pelo nome do container
    pub network: String,
    /// `PUID:PGID` do Ambiente — é quem tem de ser dono do cache
    pub user: String,
    #[serde(default)]
    pub tz: String,
}

/// O Configarr, que aplica os perfis de qualidade e os custom formats do TRaSH
/// Guides a partir do `config.yml` que a página escreveu.
///
/// É um `docker run --rm` avulso, não um serviço da stack: ele roda uma vez e
/// sai, e num `up -d` subiria antes de os apps responderem. Entra na rede da
/// stack para alcançar cada *arr pelo nome do container, com a base URL que o
/// `config.yml` já traz. Quem chama espera os apps primeiro, como o resto do
/// `apply`.
///
/// O `--dns` é o que faz ele resolver o github de dentro da rede da stack, que
/// é uma bridge nossa: sem isso o clone do TRaSH e do Recyclarr falha em
/// máquina cujo resolvedor não é alcançável de lá.
pub async fn configarr(docker: &str, cfg: &Configarr, log: &Log) -> Result<(), String> {
    log.line("aplicando os perfis do TRaSH Guides (configarr)…");
    let dir = cfg.dir.trim_end_matches('/');
    let tz = if cfg.tz.is_empty() { "Etc/UTC" } else { &cfg.tz };
    let montar = [
        format!("{dir}/config.yml:/app/config/config.yml:ro"),
        format!("{dir}/secrets.yml:/app/config/secrets.yml:ro"),
        format!("{dir}/custom_formats:/app/cfs:ro"),
        format!("{dir}/repos:/app/repos"),
    ];
    let mut args: Vec<String> = vec![
        "run".into(), "--rm".into(),
        "--name".into(), "configarr".into(),
        "--user".into(), cfg.user.clone(),
        "--network".into(), cfg.network.clone(),
        "--dns".into(), "1.1.1.1".into(),
    ];
    for m in &montar {
        args.push("-v".into());
        args.push(m.clone());
    }
    /* O cache pode ter sido clonado por outro dono — pelo root, em quem vem da
       versão em que o Configarr era serviço do compose e rodava sem `--user`.
       O git recusa mexer em repositório de outro dono ("dubious ownership") e o
       Configarr morre antes de ler o `config.yml`; estas três variáveis são a
       forma sancionada de dizer que ali é de casa, sem precisar de um
       `git config` dentro do container. */
    for e in [
        "LOG_STACKTRACE=true",
        "LOG_LEVEL=debug",
        "GIT_CONFIG_COUNT=1",
        "GIT_CONFIG_KEY_0=safe.directory",
        "GIT_CONFIG_VALUE_0=*",
    ] {
        args.push("-e".into());
        args.push(e.into());
    }
    args.push("-e".into());
    args.push(format!("TZ={tz}"));
    args.push(CONFIGARR_IMG.into());

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(docker, &refs, Path::new(dir), log).await.inspect_err(|_| {
        /* O erro do docker não diz qual é o problema quando ele é de posse: o
           Configarr morre num "Permission denied" do git, no meio de um rastro
           de pilha do node. O caso conhecido é o cache clonado por outro dono —
           quem vem da versão em que ele era serviço do compose, e rodava como
           root. A saída é apagar o cache: ele se refaz sozinho. */
        if !escrevivel(&format!("{dir}/repos")) {
            log.line(format!(
                "o cache em {dir}/repos é de outro dono e o Configarr roda como {};                  apague a pasta (ela se refaz sozinha) e mande subir de novo",
                cfg.user
            ));
        }
    })
}

/// A pasta existe e aceita escrita de quem roda o servidor? É ele quem cria as
/// pastas da stack, e o Configarr roda com o mesmo PUID/PGID.
fn escrevivel(dir: &str) -> bool {
    let sonda = Path::new(dir).join(".hubstarr-escrita");
    match std::fs::File::create(&sonda) {
        Ok(_) => {
            let _ = std::fs::remove_file(&sonda);
            true
        }
        Err(_) => false,
    }
}

pub const CONFIGARR_IMG: &str = "ghcr.io/raydak-labs/configarr:latest";

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

/* Chamada ao docker compose.

   O compose é rodado com a pasta dos arquivos como diretório do projeto, para
   que os caminhos relativos do compose e o `.env` que está ali sejam achados
   como seriam se alguém tivesse rodado o comando à mão naquela pasta. */

use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::jobs::Log;

/// O docker responde? É o que a página usa para avisar antes de tentar subir.
pub async fn docker_ok(docker: &str) -> bool {
    Command::new(docker)
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

pub async fn up(docker: &str, dir: &Path, log: Log) -> Result<(), String> {
    run(docker, &["compose", "up", "-d", "--remove-orphans"], dir, &log).await
}

pub async fn down(docker: &str, dir: &Path, log: Log) -> Result<(), String> {
    run(docker, &["compose", "down"], dir, &log).await
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

/* Gravação dos arquivos que a página mandou.

   O conteúdo vem pronto — este módulo não gera nada, só decide onde cada um
   cai. Os nomes vêm do mesmo lugar que alimenta o .zip (`docker-compose.yml`,
   `.env`, `nginx.conf`, `<container>/<conf do serviço>`), então trazem subpasta
   e precisam ser conferidos antes de virar caminho.

   Duas raízes, porque a árvore gerada tem duas metades. O `docker-compose.yml`,
   o `.env` e o `nginx.conf` ficam na pasta da stack, que é de onde o compose é
   rodado — e o bind do nginx é relativo a ela. As configurações de serviço vão
   para o `BASE_CONFIG` do Ambiente, porque é de lá que o próprio compose as
   monta nos containers: gravá-las na pasta da stack faz o container subir sem
   configuração nenhuma.

   Quem diz qual é qual é a página, no campo `base` de cada arquivo — ela é que
   conhece o layout. O servidor só resolve o `BASE_CONFIG`, que ele lê do banco
   em vez de aceitar caminho absoluto vindo do navegador. */

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct OutFile {
    pub name: String,
    pub text: String,
    /// "config" para o que é montado dos containers; o resto fica na stack
    #[serde(default)]
    pub base: Option<String>,
}

/// Junta o nome ao destino recusando tudo que escapa dele: caminho absoluto,
/// `..`, raiz do Windows. Sem isso, um `name` malformado escreveria em
/// qualquer lugar em que o processo tenha permissão.
fn safe_join(dir: &Path, name: &str) -> Result<PathBuf, String> {
    if name.trim().is_empty() {
        return Err("nome de arquivo vazio".into());
    }
    if name.contains('\\') {
        return Err(format!("nome de arquivo inválido: {name}"));
    }
    let rel = Path::new(name);
    let mut out = dir.to_path_buf();
    for c in rel.components() {
        match c {
            Component::Normal(part) => out.push(part),
            _ => return Err(format!("nome de arquivo inválido: {name}")),
        }
    }
    Ok(out)
}

/// Grava todos e devolve os caminhos gravados. `cfg` é o `BASE_CONFIG` do
/// Ambiente; sem ele — stack que nunca teve o Ambiente salvo — tudo cai na
/// pasta da stack, como antes.
pub async fn write_all(
    dir: &Path,
    cfg: Option<&Path>,
    files: &[OutFile],
) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Err("nenhum arquivo recebido".into());
    }
    let mut done = Vec::with_capacity(files.len());
    for f in files {
        let root = match f.base.as_deref() {
            Some("config") => cfg.unwrap_or(dir),
            _ => dir,
        };
        let path = safe_join(root, &f.name)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        tokio::fs::write(&path, &f.text)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        crate::registro::detalhe(|| {
            format!("arquivo {} ({} bytes)", path.display(), f.text.len())
        });
        done.push(path.display().to_string());
    }
    Ok(done)
}

/// Cria as pastas que a página listou, quando faltam. Caminho relativo é
/// recusado: estes vêm dos `source` do compose, que são do host e absolutos —
/// um relativo aqui seria erro de quem chamou, e sairia em algum lugar
/// imprevisível a partir do diretório do servidor.
pub async fn ensure_dirs(dirs: &[String], log: &crate::jobs::Log) -> Result<(), String> {
    for d in dirs {
        let p = Path::new(d);
        if !p.is_absolute() {
            return Err(format!("caminho relativo não vira pasta: {d}"));
        }
        if p.is_dir() {
            continue;
        }
        if p.exists() {
            // já existe e não é pasta: o bind vai falhar, e dizer isso agora é
            // melhor do que o erro do docker três passos adiante
            return Err(format!("{d} existe e não é uma pasta"));
        }
        tokio::fs::create_dir_all(p)
            .await
            .map_err(|e| format!("{d}: {e}"))?;
        crate::registro::detalhe(|| format!("pasta {d}"));
        log.line(format!("pasta criada: {d}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, base: Option<&str>) -> OutFile {
        OutFile { name: name.into(), text: "x\n".into(), base: base.map(String::from) }
    }

    #[tokio::test]
    async fn cria_o_que_falta_e_recusa_o_que_nao_e_pasta() {
        let tmp = std::env::temp_dir().join(format!("hubdirs-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&tmp).await;
        let log = crate::jobs::Jobs::new().log_de_teste();
        let a = tmp.join("media/tv");
        ensure_dirs(&[a.display().to_string()], &log).await.unwrap();
        assert!(a.is_dir());
        // de novo não é erro: o que já existe fica como está
        ensure_dirs(&[a.display().to_string()], &log).await.unwrap();

        // caminho ocupado por um arquivo é recusado antes de o docker tentar
        let f = tmp.join("um-arquivo");
        tokio::fs::write(&f, b"x").await.unwrap();
        assert!(ensure_dirs(&[f.display().to_string()], &log).await.is_err());
        // e caminho relativo também
        assert!(ensure_dirs(&["media/tv".into()], &log).await.is_err());
        tokio::fs::remove_dir_all(&tmp).await.unwrap();
    }

    #[tokio::test]
    async fn a_conf_vai_para_o_base_config_e_o_compose_fica_na_stack() {
        let tmp = std::env::temp_dir().join(format!("hubstarr-teste-{}", std::process::id()));
        let (dir, cfg) = (tmp.join("stack"), tmp.join("appdata"));
        let files = vec![
            f("docker-compose.yml", None),
            f("nginx/conf.d/starrnet.conf", Some("config")),
            f("qbittorrent/qBittorrent.conf", Some("config")),
        ];
        write_all(&dir, Some(&cfg), &files).await.unwrap();
        assert!(dir.join("docker-compose.yml").exists());
        assert!(cfg.join("nginx/conf.d/starrnet.conf").exists());
        assert!(cfg.join("qbittorrent/qBittorrent.conf").exists());
        // a conf não pode ficar também na pasta da stack: é lá que ela some
        assert!(!dir.join("nginx/conf.d/starrnet.conf").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn sem_base_config_tudo_cai_na_pasta_da_stack() {
        let tmp = std::env::temp_dir().join(format!("hubstarr-teste2-{}", std::process::id()));
        write_all(&tmp, None, &[f("nginx/conf.d/x.conf", Some("config"))])
            .await
            .unwrap();
        assert!(tmp.join("nginx/conf.d/x.conf").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn recusa_o_que_sai_do_destino() {
        let d = Path::new("/tmp/stack");
        assert!(safe_join(d, "docker-compose.yml").is_ok());
        assert!(safe_join(d, "nginx/conf.d/starr.conf").is_ok());
        assert!(safe_join(d, "../fora.yml").is_err());
        assert!(safe_join(d, "/etc/passwd").is_err());
        assert!(safe_join(d, "qbit\\conf").is_err());
        assert!(safe_join(d, "  ").is_err());
    }
}

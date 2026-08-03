/* Gravação dos arquivos que a página mandou.

   O conteúdo vem pronto — este módulo não gera nada, só decide onde cada um
   cai. Os nomes vêm do mesmo lugar que alimenta o .zip (`docker-compose.yml`,
   `.env`, `nginx/conf.d/<stack>.conf`, `<container>/<conf do serviço>`), então
   trazem subpasta e precisam ser conferidos antes de virar caminho. */

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct OutFile {
    pub name: String,
    pub text: String,
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

/// Grava todos e devolve os caminhos, relativos ao destino.
pub async fn write_all(dir: &Path, files: &[OutFile]) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Err("nenhum arquivo recebido".into());
    }
    let mut done = Vec::with_capacity(files.len());
    for f in files {
        let path = safe_join(dir, &f.name)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        tokio::fs::write(&path, &f.text)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        done.push(f.name.clone());
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

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

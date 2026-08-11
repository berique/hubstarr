/* Cache das capturas das paletas do theme.park.

   O repositório não redistribui captura de tela de ninguém: as imagens são da
   documentação do theme.park, e é de lá que elas vêm na primeira vez. O que
   este módulo acrescenta é o cache — a página aberta do disco continua
   buscando direto na documentação deles, e com servidor atrás dela a segunda
   visita à mesma paleta não sai da máquina.

   A capa é `GET /api/shot/:app/:theme`, e o par app/tema vem do combobox da
   página. Vem do navegador, então é conferido aqui: só letras minúsculas,
   dígitos e hífen entram na URL de origem e no nome do arquivo em cache —
   assim nem o caminho escapa da pasta nem o endereço deixa de ser o do
   theme.park. */

use std::path::{Path, PathBuf};
use std::time::Duration;

const DOCS: &str = "https://docs.theme-park.dev";
/// Teto da pasta de cache. Cada captura tem uns 2,5 MB, e as paletas de todos
/// os apps somadas passariam de 200 MB — isto guarda as últimas duas dúzias,
/// que é o que alguém chega a olhar de uma vez.
const CACHE_MAX: u64 = 64 * 1024 * 1024;

/// Cabe num nome de arquivo e num pedaço de URL? É o mesmo alfabeto dos ids de
/// serviço e dos nomes de paleta da página — o resto é recusado inteiro, em
/// vez de saneado, porque não existe pedido legítimo fora dele.
fn ok_seg(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A pasta do cache fica ao lado do banco: é estado do servidor, não da stack,
/// e apagá-la só custa uma busca a mais.
pub fn cache_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("shots")
}

/// O PNG da paleta, do cache ou da documentação do theme.park. Quando a busca
/// falha o erro sobe: a página já tem o "não deu para carregar" do `#shotErr`.
pub async fn fetch(dir: &Path, app: &str, theme: &str) -> Result<Vec<u8>, String> {
    if !ok_seg(app) || !ok_seg(theme) {
        return Err("app ou tema inválido".into());
    }
    let file = dir.join(format!("{app}-{theme}.png"));
    if let Ok(b) = tokio::fs::read(&file).await {
        if !b.is_empty() {
            return Ok(b);
        }
    }

    let url = format!("{DOCS}/site_assets/{app}/{theme}.png");
    let r = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("{url}: {}", r.status()));
    }
    let body = r.bytes().await.map_err(|e| e.to_string())?.to_vec();
    if body.is_empty() {
        return Err(format!("{url}: resposta vazia"));
    }

    /* O cache é conveniência: se a gravação falhar — pasta sem permissão, disco
       cheio —, a captura ainda é servida, e a próxima visita tenta de novo. */
    if tokio::fs::create_dir_all(dir).await.is_ok() {
        let _ = tokio::fs::write(&file, &body).await;
        prune(dir, CACHE_MAX).await;
    }
    Ok(body)
}

/// Apaga as capturas mais antigas até a pasta caber em `max`. A ordem é a da
/// gravação, não a do último uso: manter um LRU custaria escrever no disco a
/// cada acerto do cache, e o que se perde no pior caso é uma busca à rede.
///
/// Como o teto, aqui, é conveniência e não garantia, todo erro é engolido — a
/// captura já foi servida, e a próxima gravação tenta podar de novo.
async fn prune(dir: &Path, max: u64) {
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else {
        return;
    };
    let mut files = Vec::new();
    let mut total = 0u64;
    while let Ok(Some(e)) = rd.next_entry().await {
        let Ok(m) = e.metadata().await else { continue };
        if !m.is_file() {
            continue;
        }
        total += m.len();
        files.push((m.modified().ok(), m.len(), e.path()));
    }
    if total <= max {
        return;
    }
    /* Sem mtime vai para a frente da fila: é o que menos se sabe defender. */
    files.sort_by_key(|(t, _, _)| *t);
    for (_, len, path) in files {
        if total <= max {
            break;
        }
        if tokio::fs::remove_file(&path).await.is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recusa_segmento_que_escaparia_da_pasta_ou_do_dominio() {
        assert!(ok_seg("sonarr"));
        assert!(ok_seg("space-gray"));
        assert!(!ok_seg(""));
        assert!(!ok_seg(".."));
        assert!(!ok_seg("a/b"));
        assert!(!ok_seg("a.png"));
        assert!(!ok_seg("Sonarr"));
        assert!(!ok_seg("x?y=1"));
    }

    #[tokio::test]
    async fn serve_do_cache_sem_ir_a_rede() {
        let dir = std::env::temp_dir().join(format!("hubshots{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("sonarr-nord.png"), b"png")
            .await
            .unwrap();
        assert_eq!(fetch(&dir, "sonarr", "nord").await.unwrap(), b"png");
        assert!(fetch(&dir, "..", "nord").await.is_err());
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn a_poda_leva_as_capturas_mais_antigas_e_deixa_as_novas() {
        let dir = std::env::temp_dir().join(format!("hubpoda{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        for (i, nome) in ["velha", "media", "nova"].iter().enumerate() {
            tokio::fs::write(dir.join(format!("sonarr-{nome}.png")), vec![b'x'; 100])
                .await
                .unwrap();
            /* mtime tem granularidade de sistema de arquivos: sem a pausa as
               três ficariam com o mesmo instante e a ordem seria arbitrária. */
            if i < 2 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        prune(&dir, 250).await;
        assert!(!dir.join("sonarr-velha.png").exists());
        assert!(dir.join("sonarr-media.png").exists());
        assert!(dir.join("sonarr-nova.png").exists());

        prune(&dir, 10_000).await; // já cabe: não apaga nada
        assert!(dir.join("sonarr-media.png").exists());
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}

/* Chaves escritas na configuração que o próprio app criou.

   O qBittorrent guarda usuário, senha e API key num INI dentro da pasta de
   config dele, e reescreve esse arquivo inteiro ao sair — montar o nosso por
   cima congelaria tudo o que ele grava ali (torrents em andamento, janelas,
   preferências mexidas na interface). Então, em vez de montar, o servidor
   espera a stack subir e escreve só as chaves que o Hubstarr governa.

   Duas coisas que a ordem exige:

   - o container **para** antes da edição e volta depois. Com ele no ar, o que
     escrevêssemos seria sobrescrito no próximo encerramento dele, que é quando
     ele despeja a configuração em memória no disco;
   - o arquivo pode ainda não existir no primeiro `up` — o app leva alguns
     segundos para criá-lo —, então há uma espera curta. Se mesmo assim não
     aparecer, o arquivo é criado com as nossas chaves e o app o completa
     depois, que é o que ele faz com um `/config` vazio.

   O que escrever vem pronto da página, seção por seção: aqui só entra o
   formato do INI. */

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::jobs::Log;

#[derive(Deserialize)]
pub struct Patch {
    /// serviço do compose a parar e subir de novo em volta da edição
    pub service: String,
    /// caminho relativo ao `BASE_CONFIG`, como a página o montou
    pub path: String,
    /// `[Seção]` → lista de `chave=valor`, na ordem em que a página as quer
    pub sections: Vec<(String, Vec<(String, String)>)>,
}

/// Mesma regra do `files::safe_join`: o caminho vem do navegador, então nada de
/// absoluto nem de `..`.
fn safe_join(dir: &Path, name: &str) -> Result<PathBuf, String> {
    if name.trim().is_empty() || name.contains('\\') {
        return Err(format!("caminho inválido: {name}"));
    }
    let mut out = dir.to_path_buf();
    for c in Path::new(name).components() {
        match c {
            Component::Normal(part) => out.push(part),
            _ => return Err(format!("caminho inválido: {name}")),
        }
    }
    Ok(out)
}

/// Troca no INI as chaves que vieram, deixando todo o resto onde estava:
/// as outras chaves, os comentários, as seções que não conhecemos e até a
/// ordem das linhas. Chave que não existe entra no fim da seção dela; seção
/// que não existe entra no fim do arquivo.
pub fn merge_ini(atual: &str, sections: &[(String, Vec<(String, String)>)]) -> String {
    let mut linhas: Vec<String> = atual.lines().map(String::from).collect();

    for (secao, pares) in sections {
        let cabecalho = format!("[{secao}]");
        let inicio = linhas.iter().position(|l| l.trim() == cabecalho);
        let (inicio, mut fim) = match inicio {
            Some(i) => {
                // a seção vai até o próximo cabeçalho, ou até o fim
                let fim = linhas
                    .iter()
                    .enumerate()
                    .skip(i + 1)
                    .find(|(_, l)| l.trim().starts_with('[') && l.trim().ends_with(']'))
                    .map(|(j, _)| j)
                    .unwrap_or(linhas.len());
                (i, fim)
            }
            None => {
                // seção nova: entra no fim, separada por uma linha em branco
                if linhas.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                    linhas.push(String::new());
                }
                linhas.push(cabecalho);
                (linhas.len() - 1, linhas.len())
            }
        };

        for (chave, valor) in pares {
            let linha = format!("{chave}={valor}");
            let achou = linhas[inicio + 1..fim]
                .iter()
                .position(|l| chave_de(l).as_deref() == Some(chave.as_str()))
                .map(|k| inicio + 1 + k);
            match achou {
                Some(k) => linhas[k] = linha,
                None => {
                    // no fim da seção, mas antes das linhas em branco que a
                    // separam da próxima
                    let mut pos = fim;
                    while pos > inicio + 1 && linhas[pos - 1].trim().is_empty() {
                        pos -= 1;
                    }
                    linhas.insert(pos, linha);
                    fim += 1;
                }
            }
        }
    }

    let mut out = linhas.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A chave de uma linha `chave=valor`, ignorando comentário e linha em branco.
fn chave_de(linha: &str) -> Option<String> {
    let l = linha.trim();
    if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with('[') {
        return None;
    }
    l.split_once('=').map(|(k, _)| k.trim().to_string())
}

/// Espera o app criar a configuração dele. Quando ele não a cria a tempo, o
/// arquivo nasce aqui mesmo, só com as nossas chaves — que é o que ele
/// completa na próxima subida.
async fn esperar(path: &Path, log: &Log) {
    for tentativa in 0..30 {
        if let Ok(txt) = tokio::fs::read_to_string(path).await {
            if !txt.trim().is_empty() {
                return;
            }
        }
        if tentativa == 0 {
            log.line(format!("esperando {} aparecer…", path.display()));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    log.line("o app não criou a configuração a tempo; ela nasce com estas chaves");
}

/// Para o container, escreve as chaves e sobe de novo.
pub async fn apply(
    docker: &str,
    dir: &Path,
    cfg: Option<&Path>,
    p: &Patch,
    log: &Log,
) -> Result<(), String> {
    let raiz = cfg.ok_or_else(|| {
        "sem o BASE_CONFIG do Ambiente não dá para achar a configuração do app".to_string()
    })?;
    let path = safe_join(raiz, &p.path)?;

    // esperar primeiro, parar depois, ler por último: o app despeja a
    // configuração em memória no disco justamente ao sair, então o arquivo
    // mais novo é o de depois do `stop` — ler antes descartaria isso
    esperar(&path, log).await;
    crate::deploy::compose(docker, &["stop", &p.service], dir, log).await?;
    let atual = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let novo = merge_ini(&atual, &p.sections);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    tokio::fs::write(&path, &novo).await.map_err(|e| {
        // o arquivo é do container: se ele o criou com outro dono, o servidor
        // não o reescreve — é o PUID/PGID do Ambiente que faz os dois baterem
        let dica = if e.kind() == std::io::ErrorKind::PermissionDenied {
            " — confira o PUID/PGID do Ambiente: o arquivo é de outro dono"
        } else {
            ""
        };
        format!("{}: {e}{dica}", path.display())
    })?;
    log.line(format!("{}: {} chaves escritas", path.display(),
                     p.sections.iter().map(|(_, v)| v.len()).sum::<usize>()));
    crate::deploy::compose(docker, &["start", &p.service], dir, log).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secoes() -> Vec<(String, Vec<(String, String)>)> {
        vec![(
            "Preferences".into(),
            vec![
                ("WebUI\\Username".into(), "admin".into()),
                ("WebUI\\APIKey".into(), "qbt_novo".into()),
            ],
        )]
    }

    #[test]
    fn troca_a_chave_existente_e_nao_mexe_no_resto() {
        let atual = "[BitTorrent]\nSession\\Port=6881\n\n[Preferences]\nWebUI\\Username=velho\nWebUI\\Port=8181\n";
        let novo = merge_ini(atual, &secoes());
        assert!(novo.contains("WebUI\\Username=admin"));
        assert!(!novo.contains("velho"));
        // o que é do app fica onde estava
        assert!(novo.contains("Session\\Port=6881"));
        assert!(novo.contains("WebUI\\Port=8181"));
    }

    #[test]
    fn chave_que_falta_entra_na_secao_dela() {
        let atual = "[Preferences]\nWebUI\\Username=velho\n\n[Network]\nProxy\\Type=0\n";
        let novo = merge_ini(atual, &secoes());
        let linhas: Vec<&str> = novo.lines().collect();
        let i = linhas.iter().position(|l| l.starts_with("WebUI\\APIKey")).unwrap();
        let sec = linhas.iter().position(|l| *l == "[Preferences]").unwrap();
        let prox = linhas.iter().position(|l| *l == "[Network]").unwrap();
        assert!(i > sec && i < prox, "a chave nova caiu fora da seção: {novo}");
        assert!(novo.contains("Proxy\\Type=0"));
    }

    #[test]
    fn secao_que_falta_entra_no_fim() {
        let novo = merge_ini("[BitTorrent]\nSession\\Port=6881\n", &secoes());
        assert!(novo.contains("[Preferences]"));
        assert!(novo.trim_end().ends_with("WebUI\\APIKey=qbt_novo"));
    }

    #[test]
    fn arquivo_vazio_nasce_so_com_as_nossas_chaves() {
        let novo = merge_ini("", &secoes());
        assert_eq!(
            novo,
            "[Preferences]\nWebUI\\Username=admin\nWebUI\\APIKey=qbt_novo\n"
        );
    }

    #[test]
    fn aplicar_de_novo_nao_duplica_nem_reordena() {
        let atual = "[Preferences]\nWebUI\\Username=admin\nWebUI\\APIKey=qbt_novo\n";
        assert_eq!(merge_ini(atual, &secoes()), atual);
    }

    #[test]
    fn comentario_e_linha_em_branco_atravessam() {
        let atual = "# escrito pelo app\n[Preferences]\nWebUI\\Username=velho\n";
        let novo = merge_ini(atual, &secoes());
        assert!(novo.starts_with("# escrito pelo app\n"));
    }

    #[test]
    fn caminho_que_escapa_da_pasta_e_recusado() {
        let dir = Path::new("/tmp/x");
        assert!(safe_join(dir, "qbittorrent/qBittorrent/qBittorrent.conf").is_ok());
        assert!(safe_join(dir, "../fora.conf").is_err());
        assert!(safe_join(dir, "/etc/passwd").is_err());
        assert!(safe_join(dir, "").is_err());
    }
}

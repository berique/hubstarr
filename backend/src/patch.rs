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

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::jobs::Log;

#[derive(Deserialize)]
pub struct Patch {
    /// serviço do compose a parar e subir de novo em volta da edição
    pub service: String,
    /// caminho relativo ao `BASE_CONFIG`, como a página o montou
    pub path: String,
    /// `ini` (o padrão) ou `json`
    #[serde(default)]
    pub format: Option<String>,
    /// INI: `[Seção]` → lista de `chave=valor`, na ordem em que a página as quer
    #[serde(default)]
    pub sections: Vec<(String, Vec<(String, String)>)>,
    /// O que separa chave de valor no arquivo daquele app. O padrão é `=`, do
    /// INI do Qt que o qBittorrent usa; o SABnzbd escreve `chave = valor`, com
    /// os espaços, e é assim que ele relê o que está lá — quem sabe disso é a
    /// página, que é a dona do formato de cada arquivo.
    #[serde(default)]
    pub sep: Option<String>,
    /* As chaves que **não** se sobrescreve quando o arquivo já traz um valor
       para elas. É a API key do qBittorrent: uma vez que o app tenha uma, ela
       é a que os clientes dele conhecem, e trocá-la a cada Subir cortaria
       quem já estava falando com ele. Chave ausente ou vazia continua sendo
       escrita — o caso da primeira subida. */
    #[serde(default)]
    pub keep: Vec<String>,
    /// JSON: as chaves de primeiro nível a pôr por cima das que já estão lá
    #[serde(default)]
    pub json: Option<Value>,
    /// XML: elementos de primeiro nível. Valor de texto vira `<K>v</K>`; lista
    /// vira `<K><string>a</string>…</K>`, que é como o Jellyfin guarda as dele.
    #[serde(default)]
    pub xml: Option<Map<String, Value>>,
}

impl Patch {
    /// O conteúdo novo do arquivo, a partir do que já estava nele.
    fn merge(&self, atual: &str) -> Result<String, String> {
        match self.format.as_deref() {
            Some("json") => merge_json(atual, self.json.as_ref().unwrap_or(&Value::Null)),
            Some("xml") => merge_xml(atual, self.xml.as_ref().unwrap_or(&Map::new())),
            _ => Ok(merge_ini(
                atual,
                &self.sections,
                self.sep.as_deref().unwrap_or("="),
                &self.keep,
            )),
        }
    }

    /// Quantas chaves este arquivo recebe — é o que vai para o log.
    fn chaves(&self) -> usize {
        match self.format.as_deref() {
            Some("json") => self
                .json
                .as_ref()
                .and_then(|v| v.as_object())
                .map(|m| m.len())
                .unwrap_or(0),
            Some("xml") => self.xml.as_ref().map(|m| m.len()).unwrap_or(0),
            _ => self.sections.iter().map(|(_, v)| v.len()).sum(),
        }
    }
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
pub fn merge_ini(
    atual: &str,
    sections: &[(String, Vec<(String, String)>)],
    sep: &str,
    manter: &[String],
) -> String {
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
            let linha = format!("{chave}{sep}{valor}");
            let achou = linhas[inicio + 1..fim]
                .iter()
                .position(|l| chave_de(l).as_deref() == Some(chave.as_str()))
                .map(|k| inicio + 1 + k);
            match achou {
                /* Chave que o app já respondeu por si: se ela está no `manter`
                   e tem valor, fica como está. Vazia não conta — é a linha que
                   ele deixa pronta esperando alguém preencher. */
                Some(k)
                    if manter.iter().any(|m| m == chave)
                        && valor_de(&linhas[k], sep).is_some_and(|v| !v.trim().is_empty()) => {}
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

/// Põe as nossas chaves de primeiro nível por cima das que já estão no
/// arquivo, deixando as outras onde estavam — no `categories.json`, é o que
/// preserva a categoria que alguém criou na interface do app.
///
/// Arquivo ilegível não é motivo para parar: o app o reescreve inteiro na
/// próxima vez, e o que interessa é o nosso chegar lá.
/* Os elementos que o Hubstarr governa dentro de um XML de configuração — hoje
   o `network.xml` do Jellyfin.

   Não é um parser de XML: é a mesma ideia do merge do INI, linha a linha. O
   elemento que já existe é trocado no lugar, com a indentação dele; o que falta
   entra antes da tag de fechamento da raiz. Todo o resto do arquivo — ordem,
   comentários, o que o app guardou — fica como estava, porque quem manda nele é
   o app.

   Vale só para elemento de primeiro nível cujo valor cabe numa linha, que é o
   caso do `BaseUrl`. Lista vira `<K><string>a</string>…</K>`, na forma em que o
   Jellyfin as escreve. */
pub fn merge_xml(atual: &str, nosso: &Map<String, Value>) -> Result<String, String> {
    if nosso.is_empty() {
        return Ok(atual.to_string());
    }
    let mut linhas: Vec<String> = atual.lines().map(String::from).collect();

    for (chave, valor) in nosso {
        let corpo = match valor {
            Value::Array(itens) => itens
                .iter()
                .map(|i| format!("<string>{}</string>", esc_xml(&texto_de(i))))
                .collect::<String>(),
            v => esc_xml(&texto_de(v)),
        };

        let abre = format!("<{chave}>");
        let vazio = format!("<{chave} />");
        let achou = linhas.iter().position(|l| {
            let t = l.trim_start();
            t.starts_with(&abre) || t.starts_with(&vazio)
        });
        match achou {
            Some(i) => {
                // o elemento pode estar aberto numa linha e fechado noutra:
                // o que houver entre as duas sai junto
                let ind: String = linhas[i].chars().take_while(|c| c.is_whitespace()).collect();
                let fecha = format!("</{chave}>");
                let fim = linhas[i..]
                    .iter()
                    .position(|l| l.contains(&fecha))
                    .map(|k| i + k)
                    .unwrap_or(i);
                linhas.splice(i..=fim, [format!("{ind}<{chave}>{corpo}</{chave}>")]);
            }
            None => {
                // entra antes de fechar a raiz, com a indentação de quem já está lá
                let fim = linhas
                    .iter()
                    .rposition(|l| l.trim_start().starts_with("</"))
                    .unwrap_or(linhas.len());
                linhas.insert(fim, format!("  <{chave}>{corpo}</{chave}>"));
            }
        }
    }
    let mut out = linhas.join("\n");
    if atual.ends_with('\n') || !atual.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

/// O texto de um valor JSON: string sem as aspas, o resto como veio.
fn texto_de(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        outro => outro.to_string(),
    }
}

fn esc_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn merge_json(atual: &str, nosso: &Value) -> Result<String, String> {
    let mut obj: Map<String, Value> = serde_json::from_str(atual).unwrap_or_default();
    let nosso = nosso
        .as_object()
        .ok_or_else(|| "o patch em JSON não veio como objeto".to_string())?;
    for (k, v) in nosso {
        obj.insert(k.clone(), v.clone());
    }
    /* Quatro espaços, que é como o qBittorrent escreve este arquivo e como a
       aba da página o mostra: seguir o estilo dele evita um diff do arquivo
       inteiro toda vez que um dos dois grava. */
    let mut buf = Vec::new();
    let recuo = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, recuo);
    Value::Object(obj)
        .serialize(&mut ser)
        .map_err(|e| e.to_string())?;
    let mut txt = String::from_utf8(buf).map_err(|e| e.to_string())?;
    txt.push('\n');
    Ok(txt)
}

/// A chave de uma linha `chave=valor`, ignorando comentário e linha em branco.
/// O valor de uma linha `chave=valor`, para saber se o app já respondeu por ela.
/// O separador é o daquele arquivo, mas o `=` do INI serve de fallback: é o que
/// divide a linha em qualquer um dos dois formatos que geramos.
fn valor_de(linha: &str, sep: &str) -> Option<String> {
    let l = linha.trim();
    if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with('[') {
        return None;
    }
    l.split_once(sep.trim())
        .or_else(|| l.split_once('='))
        .map(|(_, v)| v.trim().to_string())
}

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

/// Escreve o que a página mandou, um serviço por vez: o container para uma
/// vez só, todos os arquivos dele são mesclados, e ele sobe de novo. Parar por
/// arquivo daria dois ciclos no qBittorrent, que tem dois.
pub async fn apply_all(
    docker: &str,
    dir: &Path,
    cfg: Option<&Path>,
    patches: &[Patch],
    log: &Log,
) -> Result<(), String> {
    if patches.is_empty() {
        return Ok(());
    }
    let raiz = cfg.ok_or_else(|| {
        "sem o BASE_CONFIG do Ambiente não dá para achar a configuração do app".to_string()
    })?;

    // na ordem em que a página os mandou, agrupados por serviço
    let mut servicos: Vec<&str> = Vec::new();
    for p in patches {
        if !servicos.contains(&p.service.as_str()) {
            servicos.push(&p.service);
        }
    }

    for servico in servicos {
        let meus: Vec<&Patch> = patches.iter().filter(|p| p.service == servico).collect();
        let caminhos: Vec<PathBuf> = meus
            .iter()
            .map(|p| safe_join(raiz, &p.path))
            .collect::<Result<_, _>>()?;

        /* Esperar uma vez por serviço, pelo primeiro arquivo: é o sinal de que
           o app subiu e criou a pasta de config dele. O segundo arquivo pode
           legitimamente não existir — o qBittorrent só escreve o
           `categories.json` quando tem categoria. */
        esperar(&caminhos[0], log).await;
        crate::deploy::compose(docker, &["stop", servico], dir, log).await?;
        for (p, path) in meus.iter().zip(&caminhos) {
            let atual = tokio::fs::read_to_string(path).await.unwrap_or_default();
            let novo = p.merge(&atual)?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            tokio::fs::write(path, &novo).await.map_err(|e| {
                // o arquivo é do container: se ele o criou com outro dono, o
                // servidor não o reescreve — é o PUID/PGID do Ambiente que faz
                // os dois baterem
                let dica = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    " — confira o PUID/PGID do Ambiente: o arquivo é de outro dono"
                } else {
                    ""
                };
                format!("{}: {e}{dica}", path.display())
            })?;
            crate::registro::detalhe(|| {
                format!(
                    "arquivo {} ({} bytes, chaves escritas na conf do app)",
                    path.display(),
                    novo.len()
                )
            });
            log.line(format!("{}: {} chaves escritas", path.display(), p.chaves()));
        }
        crate::deploy::compose(docker, &["start", servico], dir, log).await?;
    }
    Ok(())
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
        let novo = merge_ini(atual, &secoes(), "=", &[]);
        assert!(novo.contains("WebUI\\Username=admin"));
        assert!(!novo.contains("velho"));
        // o que é do app fica onde estava
        assert!(novo.contains("Session\\Port=6881"));
        assert!(novo.contains("WebUI\\Port=8181"));
    }

    #[test]
    fn chave_que_falta_entra_na_secao_dela() {
        let atual = "[Preferences]\nWebUI\\Username=velho\n\n[Network]\nProxy\\Type=0\n";
        let novo = merge_ini(atual, &secoes(), "=", &[]);
        let linhas: Vec<&str> = novo.lines().collect();
        let i = linhas.iter().position(|l| l.starts_with("WebUI\\APIKey")).unwrap();
        let sec = linhas.iter().position(|l| *l == "[Preferences]").unwrap();
        let prox = linhas.iter().position(|l| *l == "[Network]").unwrap();
        assert!(i > sec && i < prox, "a chave nova caiu fora da seção: {novo}");
        assert!(novo.contains("Proxy\\Type=0"));
    }

    /* A API key que o app já tem fica como está: uma vez que ele responda por
       ela, é a chave que os clientes dele conhecem, e trocá-la a cada Subir
       cortaria quem já estava falando com ele. */
    #[test]
    fn a_chave_do_manter_nao_e_sobrescrita() {
        let atual = "[Preferences]\nWebUI\\APIKey=qbt_do_proprio_app\nWebUI\\Port=8080\n";
        let manter = vec!["WebUI\\APIKey".to_string()];
        let novo = merge_ini(atual, &secoes(), "=", &manter);
        assert!(novo.contains("WebUI\\APIKey=qbt_do_proprio_app"), "{novo}");
        assert!(!novo.contains("qbt_novo"), "{novo}");
        // o resto continua sendo escrito normalmente
        assert!(novo.contains("WebUI\\Username=admin"), "{novo}");
    }

    /// Chave que existe vazia é a linha que o app deixa esperando alguém
    /// preencher — essa nós preenchemos.
    #[test]
    fn a_chave_vazia_do_manter_e_preenchida() {
        let atual = "[Preferences]\nWebUI\\APIKey=\n";
        let manter = vec!["WebUI\\APIKey".to_string()];
        let novo = merge_ini(atual, &secoes(), "=", &manter);
        assert!(novo.contains("WebUI\\APIKey=qbt_novo"), "{novo}");
    }

    #[test]
    fn secao_que_falta_entra_no_fim() {
        let novo = merge_ini("[BitTorrent]\nSession\\Port=6881\n", &secoes(), "=", &[]);
        assert!(novo.contains("[Preferences]"));
        assert!(novo.trim_end().ends_with("WebUI\\APIKey=qbt_novo"));
    }

    #[test]
    fn arquivo_vazio_nasce_so_com_as_nossas_chaves() {
        let novo = merge_ini("", &secoes(), "=", &[]);
        assert_eq!(
            novo,
            "[Preferences]\nWebUI\\Username=admin\nWebUI\\APIKey=qbt_novo\n"
        );
    }

    #[test]
    fn aplicar_de_novo_nao_duplica_nem_reordena() {
        let atual = "[Preferences]\nWebUI\\Username=admin\nWebUI\\APIKey=qbt_novo\n";
        assert_eq!(merge_ini(atual, &secoes(), "=", &[]), atual);
    }

    #[test]
    fn comentario_e_linha_em_branco_atravessam() {
        let atual = "# escrito pelo app\n[Preferences]\nWebUI\\Username=velho\n";
        let novo = merge_ini(atual, &secoes(), "=", &[]);
        assert!(novo.starts_with("# escrito pelo app\n"));
    }

    /// O separador não é enfeite: o SABnzbd escreve `chave = valor`, com os
    /// espaços, e é nessa forma que ele relê o arquivo. Escrever `chave=valor`
    /// ali deixava a chave onde o app não a encontra.
    #[test]
    fn o_sabnzbd_leva_espaco_dos_dois_lados_do_igual() {
        let atual = "[misc]\nhost_whitelist = velho,\ninet_exposure = 0\n";
        let secoes = vec![(
            "misc".to_string(),
            vec![
                ("host_whitelist".to_string(), "sabnzbd,localhost".to_string()),
                ("api_key".to_string(), "abc123".to_string()),
            ],
        )];
        let novo = merge_ini(atual, &secoes, " = ", &[]);
        assert!(novo.contains("host_whitelist = sabnzbd,localhost"));
        assert!(novo.contains("api_key = abc123"));
        // o que já estava lá, e não veio no patch, fica como estava
        assert!(novo.contains("inet_exposure = 0"));
        // e o padrão continua sendo o do Qt, sem espaço
        let qt = merge_ini("[BitTorrent]\n", &secoes, "=", &[]);
        assert!(qt.contains("api_key=abc123"));
    }

    #[test]
    fn o_json_poe_as_nossas_categorias_e_deixa_as_de_fora() {
        let atual = r#"{"minha":{"save_path":"/downloads/minha"},"tv-sonarr":{"save_path":"/velho"}}"#;
        let nosso = serde_json::json!({"tv-sonarr": {"save_path": "/downloads/torrents/tv-sonarr"}});
        let novo: Value = serde_json::from_str(&merge_json(atual, &nosso).unwrap()).unwrap();
        // a categoria criada na interface do app fica
        assert_eq!(novo["minha"]["save_path"], "/downloads/minha");
        // a nossa manda na que tem o mesmo nome
        assert_eq!(novo["tv-sonarr"]["save_path"], "/downloads/torrents/tv-sonarr");
        // e o recuo é o do app, não o do serde
        assert!(merge_json(atual, &nosso).unwrap().contains("\n    \"minha\""));
    }

    #[test]
    fn json_ilegivel_ou_vazio_nao_derruba_a_gravacao() {
        let nosso = serde_json::json!({"tv": {"save_path": "/downloads/tv"}});
        for atual in ["", "nem json é", "[]"] {
            let novo: Value = serde_json::from_str(&merge_json(atual, &nosso).unwrap()).unwrap();
            assert_eq!(novo["tv"]["save_path"], "/downloads/tv", "veio de {atual:?}");
        }
    }

    #[test]
    fn o_formato_escolhe_o_merge_e_o_ini_e_o_padrao() {
        let ini = Patch { service: "qbittorrent".into(), path: "x".into(), format: None,
                          sections: secoes(), sep: None, json: None, xml: None, keep: vec![] };
        assert!(ini.merge("").unwrap().contains("[Preferences]"));
        assert_eq!(ini.chaves(), 2);

        let js = Patch { service: "qbittorrent".into(), path: "x".into(),
                         format: Some("json".into()), sections: vec![], sep: None, xml: None, keep: vec![],
                         json: Some(serde_json::json!({"tv": {"save_path": "/d"}})) };
        assert!(js.merge("{}").unwrap().contains("\"tv\""));
        assert_eq!(js.chaves(), 1);
    }

    #[test]
    fn caminho_que_escapa_da_pasta_e_recusado() {
        let dir = Path::new("/tmp/x");
        assert!(safe_join(dir, "qbittorrent/qBittorrent/qBittorrent.conf").is_ok());
        assert!(safe_join(dir, "../fora.conf").is_err());
        assert!(safe_join(dir, "/etc/passwd").is_err());
        assert!(safe_join(dir, "").is_err());
    }

    /// O `network.xml` do Jellyfin: o que já está lá é trocado no lugar, o que
    /// falta entra antes de fechar a raiz, e o resto do arquivo não se mexe.
    #[test]
    fn o_xml_troca_o_que_existe_e_acrescenta_o_que_falta() {
        let atual = "<?xml version=\"1.0\"?>\n<NetworkConfiguration>\n  \
                     <EnableIPv6>true</EnableIPv6>\n  <BaseUrl></BaseUrl>\n\
                     </NetworkConfiguration>\n";
        let mut nosso = Map::new();
        nosso.insert("BaseUrl".into(), Value::String("/jellyfin".into()));
        nosso.insert(
            "KnownProxies".into(),
            Value::Array(vec![Value::String("nginx".into())]),
        );
        let novo = merge_xml(atual, &nosso).unwrap();
        assert!(novo.contains("<BaseUrl>/jellyfin</BaseUrl>"));
        assert!(novo.contains("<KnownProxies><string>nginx</string></KnownProxies>"));
        // o que o app guardou fica
        assert!(novo.contains("<EnableIPv6>true</EnableIPv6>"));
        // e não duplica: aplicar de novo dá o mesmo arquivo
        assert_eq!(merge_xml(&novo, &nosso).unwrap(), novo);
        assert_eq!(novo.matches("<BaseUrl>").count(), 1);
    }

    /// Elemento que ocupa várias linhas — como o Jellyfin escreve as listas —
    /// é trocado inteiro, sem deixar metade para trás.
    #[test]
    fn o_xml_troca_o_elemento_de_varias_linhas_inteiro() {
        let atual = "<Config>\n  <KnownProxies>\n    <string>velho</string>\n  \
                     </KnownProxies>\n</Config>\n";
        let mut nosso = Map::new();
        nosso.insert(
            "KnownProxies".into(),
            Value::Array(vec![Value::String("nginx".into())]),
        );
        let novo = merge_xml(atual, &nosso).unwrap();
        assert!(novo.contains("<KnownProxies><string>nginx</string></KnownProxies>"));
        assert!(!novo.contains("velho"));
        assert_eq!(novo.matches("KnownProxies").count(), 2);   // abre e fecha, uma vez
    }
}

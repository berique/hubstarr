# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Projeto

Hubstarr é um protótipo que monta e sobe uma stack de mídia (*arr + clientes de
download + servidor de mídia). São duas metades, e as duas são necessárias para
o resultado que o projeto promete:

- **A página**, `hubstarr.html` — um arquivo só (CSS, HTML e um `<script>`
  inline). É ela que **decide e gera**: o catálogo de serviços, o `CONFIG`, e os
  geradores do `docker-compose.yml`, do `.env` e do `nginx.conf`. Não tem build
  nem package manager; para vê-la, abra o endereço do servidor — ou o arquivo no
  navegador, que é o modo de quem quer só os arquivos.
- **O servidor**, `backend/` (Rust, desde o v0.2) — guarda a stack em SQLite,
  grava os arquivos, sobe no Docker e **configura os apps uns nos outros** pela
  API de cada um. Tem build e testes (`cd backend && cargo test`).

A página abre sozinha do disco e ali gera os arquivos num `.zip`, e isso
continua valendo — mas o que depende de app no ar (senha do qBittorrent, base
URL do Jellyfin, clientes de download nos *arr, perfis do TRaSH Guides) é do
servidor. Ao mexer na página, não quebre o modo `file://`; ao mexer no
servidor, lembre que ele **nunca gera conteúdo** — ver "Servidor" mais abaixo.

O `.mvn/` é resto de outro projeto e está no `.gitignore`.

Licença GPL-3.0 (`LICENSE`, texto oficial da FSF). O aviso de copyright fica no
comentário logo depois do `<!DOCTYPE html>` — não o remova ao mexer no arquivo,
é ele que liga o código à licença.

## Arquitetura do script

O script é uma sequência de seções marcadas por comentários `/* ---------- x ---------- */`:

1. **`I18N` / `LANGS`** — dicionário com uma chave por string visível, em
   en, pt-BR e es — o **inglês é o padrão**: é o fallback do `t()`, o `lang` do
   `<html>`, o texto que está escrito no HTML estático, o primeiro do seletor e
   o idioma em que a página abre. O do navegador **não** entra na conta: quem
   quer outro escolhe no seletor do cabeçalho, e é essa escolha que fica no
   `localStorage`. Valor é string ou função quando depende de dados. Acesso por
   `t(chave, ...args)`. O HTML estático usa `data-i18n` (e `-html`, `-ph`,
   `-title`), aplicados por `applyI18n()`. As traduções cobrem também os
   comentários dos arquivos gerados. Adicionar idioma = copiar um bloco e traduzir.
2. **`SERVICES`** — catálogo dos serviços disponíveis. Cada entrada traz
   `id, name, port (interna), img, color` e flags que dirigem a geração:
   `media`/`mdir` (subpasta da biblioteca), `needsDl` (monta a árvore de
   downloads inteira), `dlClient`, `vpn`, `hw` (Jellyfin), `library` (Jellyfin:
   monta a base e mais as pastas que ficaram fora dela), `solver` (Prowlarr;
   ver abaixo), `internal` (gluetun e FlareSolverr: sem rota no nginx e sem
   botão de link), `stripBase` (qBittorrent: rota com o prefixo retirado, por
   não ter base URL configurável — ver os invariantes), `publish` (Seerr: fora do nginx, publica a
   porta dele no host; a porta escolhida fica na instância, em `hostPort`, e o
   campo dela é do modal do serviço), `vpnCfg` (gluetun: as
   credenciais da VPN no modal dele),
   `webAuth` (usuário e senha de quem tem interface própria — o valor da flag
   diz *de quem* são, e é a chave do `WEB_AUTH`, que aponta para o par no
   `DEFAULTS`: `'qbit'` vai para a conf do qBittorrent e `'jf'` é o
   administrador que o Subir cria no assistente do Jellyfin, e não vai para
   arquivo nenhum. O bloco do modal é um só; a linha da API key é do
   qBittorrent), `conf` (qBittorrent: os
   dois arquivos dele — nenhum dos dois é montado, o servidor os escreve depois
   de subir; ver `patch` abaixo), `cdh` (SABnzbd: gerenciamento de
   downloads concluídos na Configuração), `dlKey` (SABnzbd: a API key no modal
   dele — vazia significa "acompanhe a `${STARR_APIKEY}`", e o botão Gerar cria
   outra pelo mesmo método, 16 bytes em hexadecimal; com uma própria, ela vai ao
   `.env` como `<CNAME>_API_KEY` em vez de virar texto solto no compose),
   `dlDirs` (SABnzbd: as duas pastas dele — o que está baixando e o que
   terminou — em vez da subpasta única; as duas são montadas e viram o
   `download_dir` e o `complete_dir` do `sabnzbd.ini`), `noVol`, `derived` (Bazarr herda as
   subpastas das instâncias de Radarr/Sonarr presentes), `site` (endereço do
   projeto, que é o que põe o serviço na grade dos Créditos) e `credit` (nome
   a creditar quando ele difere do `name` — o FlareSolverr roda o Byparr).
   Adicionar um serviço normalmente é acrescentar uma linha aqui + o ícone em
   `ICONS` + as strings `d.<id>` no `I18N`.
3. **Constantes de convenção** — `STACK`/`NETWORK` (`starrnet`), `NGINX`
   (reverse proxy fixo, fora do combobox),
   `MULTI` (serviços com múltiplas instâncias), e os mapas de
   variáveis de ambiente `INSTANCE_ENV`, `URLBASE_ENV`, `APIKEY_ENV`.
4. **Estado** — quatro globais mutáveis: `added` (instâncias,
   `{id,title,data,abs,libs,vpn,hw,solver}` — `abs` só quando o caminho da mídia
   sai das bases, e aí é ele que vai literal para o compose; `libs` são as
   pastas avulsas do Jellyfin, do "+ pasta"), `picked` (id no
   combobox), `editing` (key em edição). `DEFAULTS` guarda o ambiente global
   (caminhos base, PUID/PGID, TZ, portas do host, TLS, VPN, API key). Nem tudo
   que está no `DEFAULTS` se edita no Ambiente: as portas do host saem no modal
   do nginx e as credenciais da VPN no do gluetun (flag `vpnCfg`) — os dois são
   de um serviço só, não da stack. `CONFIG` guarda as ligações entre instâncias:
   `profiles[chave da instância]` (os perfis de qualidade — ver "Perfis de
   qualidade" abaixo), `apps` (o que o Prowlarr configura), `clients[cliente] = {arrs, cats, cdh}`
   (com que categoria cada *arr usa o cliente e o gerenciamento de downloads
   concluídos; o `arrs` fica todo `true` — a interface não tem mais caixa de
   quem recebe, porque o cliente entra em todas as instâncias. O padrão da
   categoria sai do `catDefault()`: o `CATEGORY` por app e, para quem tem
   categorias próprias, o `CLIENT_CATEGORY` — o SABnzbd usa as dele, `tv`,
   `movies` e `music`) e `mm[família]` (Media Management mais a nomenclatura,
   cujos campos estão em `NAMING_FIELDS` — os formatos de episódio e de filme
  saem de fábrica com os do TRaSH Guides, variante do Jellyfin com id do TMDb).
   `syncConfig()` o alinha com o `added` — e completa o que falta: chave nova do
   `NAMING_FIELDS` ou do `mmDefault()` entra num `mm` já guardado, senão o banco
   ficaria para sempre com a forma de uma versão anterior da página. Ele é
   chamado no `render()`, não só ao abrir o modal: quem nunca abriu a Configuração tinha o `CONFIG` vazio, e o
   Subir não aplicava nomenclatura nem Media Management nenhum, porque não havia
   o que mandar.

   O bloco **Media Management (avançado)** — o `MM_ADV`, o que os apps escondem
   atrás do "Advanced Settings" — vale para as três famílias com os mesmos oito
   campos, porque os nomes de API deles coincidem no Sonarr, no Radarr e no
   Lidarr. O que muda de família para família é só o *valor* do `fileDate`, e
   por isso ele traz `optsBy` no lugar de `opts`: cada app oferece as datas que
   conhece. Ele mora em `mm[família].naming.adv` pela mesma razão do `scope`
   abaixo: é o `cfg_naming` que guarda JSON livre, e assim não há coluna nem
   migração — o servidor o tira de dentro do `naming` e o trata como campo do
   `mediamanagement`. Os dois numéricos (`recycleDays`, `minFree`) são texto na
   interface e número na API; a conversão é do servidor, e texto que não for
   número vira erro no log em vez de virar zero calado.

   O Media Management é **por família**, com uma exceção: campo do
   `NAMING_FIELDS` marcado com `perInst` vale por instância, e quais delas o
   recebem fica em `mm[família].naming.scope[campo][chave da instância]` — hoje
   são os seis formatos do Sonarr: os três de episódio e as três pastas. O escopo mora *dentro* do
   `naming` de propósito: é o que o `cfg_naming` já guarda inteiro, em JSON, sem
   coluna nem migração. De fábrica toda instância recebe todos os formatos.
   **Pelo menos uma instância marcada no formato padrão é obrigatório** — o
   campo é obrigatório no app —, então a interface recusa desmarcar a última, e
   quem fica de fora simplesmente não recebe aquela chave: o `merge` do servidor
   só troca o que chega, e o app mantém o formato que já tinha. A lista só
   aparece com duas instâncias ou mais da família.
5. **Derivações** — `slug()` → `cname()` (container_name = chave do serviço =
   pasta de config), `route()`, `url()` (o endereço sai do domínio do Ambiente
   e, sem ele, do `location.hostname` — quem abriu a página pelo IP da LAN quer
   os links no mesmo IP; aberta do disco, sobra o `localhost`), `cfgPath`/`dataPath` (com variáveis
   `${...}` do `.env`) e `cfgReal`/`dataReal` (caminhos resolvidos, para o hint
   do modal). Alterar `cname` afeta compose, nginx e `.env` ao mesmo tempo.
   `dupPaths()` compara os caminhos já resolvidos e avisa, no rodapé da lista,
   quando duas instâncias caem na mesma pasta — Jellyfin e Bazarr ficam fora,
   um monta a biblioteca inteira e o outro segue as instâncias.
   Quem monta pasta de outro serviço passa por `derivedMounts()` (Bazarr) e
   `extraLibs()` (Jellyfin, que junta as pastas de fora das outras instâncias
   com as do "+ pasta") — os dois já devolvem o caminho certo de cada instância,
   literal ou com variável; não remonte `${BASE_MEDIA}/…` na mão.
6. **UI** — as etiquetas da linha do serviço saem todas do `tagHtml(kind,
   texto)`, e o `kind` é o que dá a cor no CSS (`.tag[data-kind=…]`) e o rótulo
   no `I18N` (`tag.<kind>`). O `TAG_KINDS` é a lista deles, na ordem de leitura,
   e é dele que o `renderLegend()` monta a legenda ao lado do "Limpar tudo" —
   cor sem legenda é adivinhação. Etiqueta nova é uma linha no `TAG_KINDS`, a
   cor nas duas tabelas do CSS (a da etiqueta e a do ponto da legenda) e as
   strings nos três idiomas; não monte `<span class="tag">` à mão.

   `renderCombo()`, `renderItems()` (a ordem da lista se muda
   arrastando a **linha inteira** — o `.item` é que leva o `draggable`, e o
   `dragstart` desiste quando o gesto começa num `button`, `a` ou campo, senão
   arrastar comeria o clique no Link e no Editar. `moveInstance()` mexe no
   `added` e o `render()` grava — a ordem chega ao banco pelo `keys` do
   `saveSettings()`, e é o `reconcile()` que a escreve no `ord`. A alça `⁙` é o
   sinal de que a linha se move e o que o teclado usa: as setas ↑ ↓ com ela em
   foco, porque arrastar não pode ser o único caminho. O nginx é linha fixa,
   sem `data-key`, e soltar sobre ele não faz nada; a ordem também não manda em
   quem sobe primeiro — isso é do `depends_on`), modal de configuração
   (`openModal` + o handler de `#mSave`), modal de ambiente (`openEnv`), modal
   do nginx (`openNgx`), modal de configuração (`renderConfig`/`openCfg`, com
   backup para o Cancelar), modal da captura da paleta (`openShot`) e o dos
   créditos (`renderCredits`/`openCred`, grade montada do `SERVICES` pelo campo
   `site`), lista de pastas do Jellyfin (`renderLibs`/`libsOf`),
   `buildHelp()` e tema claro/escuro. O hint do modal (`updateHint`) é montado
   à mão com `innerHTML` a cada digitada — quem mexe nos campos chama
   `hintNow()`, e o `applyI18n()` o refaz para acompanhar a troca de idioma.
7. **Geradores** — `build()` (compose), `buildEnv()`, `buildNginx()`,
   `buildJellyfin()` e `buildConfigarr()` (o `config.yml` do Configarr; ver
   "Perfis de qualidade" abaixo). Eles saem só quando o serviço está na stack, e
   a aba aparece e some com ele.

   **Aba, só quem é montado.** Os arquivos de `patch` — a conf e o
   `categories.json` do qBittorrent, o `sabnzbd.ini` — não têm painel nem
   gerador de HTML: existem só na forma de dados (`qbitPairs()`, `qbitCats()`,
   `sabPairs()`), que é o que o servidor recebe. Mostrá-los numa aba convidava a
   copiá-los para um lugar que o compose não monta. No `.zip` eles continuam
   saindo, com o texto montado pelo `patchText()` a partir do mesmo dado — um
   lugar só, para o pacote e o servidor nunca discordarem.

   Serviço com arquivo próprio
   traz `conf:[{host, target, pane, tab, patch}, …]` no catálogo — é **lista**,
   porque o qBittorrent gera dois (a conf e o `categories.json`) —, e o
   `confsOf()` é quem a percorre no bind do compose, no `outFiles()` e nas abas;
   entrada sem `pane` não vira aba.
   `host` é o caminho dentro da pasta de config dele, e vale para quem é
   montado. **`patch`** muda o destino da entrada: em vez de bind, ela vira
   chaves que o servidor escreve na configuração que o próprio app criou, depois
   do `up` — o valor da flag é a chave do `PATCH_DATA`, que diz o formato
   (`ini`, `json` ou `xml`) e quem monta os dados. O `xml` é o `network.xml` do
   Jellyfin: o `merge_xml` do servidor não é parser, é a mesma ideia do INI —
   elemento de primeiro nível que existe é trocado no lugar (inclusive quando
   ocupa várias linhas), o que falta entra antes de fechar a raiz, e o resto do
   arquivo não se toca. O `outPatches()` é o que a página
   manda junto do `up`, e no `.zip` a entrada sai no caminho em que o app lê o
   arquivo, não no `host`. Eles
   emitem **HTML com spans de realce** (`<span class="k">`/`v`/`c`); o texto
   puro para copiar/baixar vem de `textContent` dos panes (`plain()`,
   `plainEnv()`, `plainNginx()`). Ao editar um gerador, mantenha a marcação e
   passe strings pelo `t()`.
8. **ZIP** — `makeZip()` é uma implementação própria do formato (método
   "store", CRC32 manual), justamente para não depender de biblioteca externa.
   É a saída de quem não tem servidor: o `detectServer()` esconde o `#dl`
   quando alguém responde em `api/health`, porque ali o **Subir** grava os
   mesmos arquivos sem passar pelo pacote.

## Passo a passo da primeira visita

O `TOUR` é uma lista de `{sel, k}`: o seletor da área e a chave das strings
(`tour.<k>.h` e `tour.<k>` no `I18N`). O `startTour()` monta a volta só com os
passos cujo alvo está **visível** — o Subir e o Derrubar ficam de fora sem
servidor, e a volta encurta sozinha —, acende cada um com um recorte
(`#tourHole`, um `box-shadow` gigante) e põe o cartão acima ou abaixo, conforme
o espaço. Acrescentar um passo é acrescentar uma linha no `TOUR` e as duas
strings nos três idiomas.

O cartão traz o **seletor de idioma** (`#tourLang`), o mesmo do cabeçalho: quem
está na primeira visita é justamente quem ainda não achou aquele. As opções são
copiadas do `#langSel` e o `onchange` chama o `setLang()` — o `applyI18n()` já
redesenha o passo aberto, então o cartão muda de idioma sem sair da volta.

Ele não volta depois de concluído ou pulado, e a marca fica no `localStorage`
(`hubstarr.tour`), não no banco: quem viu a volta foi aquele navegador, não a
stack. O acesso é dentro de `try/catch` — navegador que recuse armazenamento em
`file://` só a repete na próxima abertura, em vez de quebrar.

## Quatro padrões que se repetem

**Ajuda por campo.** Marcar uma `.row` de qualquer modal com
`data-help="<chave do I18N>"` basta: `buildHelp()` põe um `?` na terceira coluna
do grid e insere abaixo um parágrafo escondido com `data-i18n-html`, que o botão
liga e desliga e o `applyI18n()` retraduz. Não escreva o parágrafo à mão nem
deixe hint fixo onde cabe um `data-help`. Em conteúdo remontado — a Configuração
—, chame `buildHelp(container)` no fim do render; ali o `?` pode vir pronto no
HTML com o `data-help` no próprio botão, e o parágrafo entra depois do grupo
(`.cfgList`) em que ele está.

**Variável do `.env` na interface.** O campo da subpasta mostra o caminho
resolvido e expande o que for digitado: `expandVars()` troca `${BASE_MEDIA}`,
`${DOWNLOAD_BASE}` e `${BASE_CONFIG}` pelo valor atual, preservando o cursor.
`dataOf()` faz o caminho de volta — devolve a subpasta e, quando o caminho sai
das bases, o literal em `abs`. Campo que aceita caminho deve usar os dois, não
`slug()` no valor cru.

**Campo de um serviço fica no modal dele.** O Ambiente é só o que vale para a
stack inteira. O que é de um container vai para o "Editar" dele, num bloco
escondido atrás de uma flag do `SERVICES` — `library` traz as pastas do
Jellyfin, `vpnCfg` traz as credenciais da VPN, e as portas do host vivem no
modal do nginx. Já foram duas mudanças nessa direção; não faça o caminho de
volta.

**Ícone da variante.** Serviço com a flag `tpAddon` tem variante de tema
(`std`/`uhd`/`anime`, o `TP_ADDONS`), e cada variante pode ter logotipo próprio:
a chave em `ICONS` é `<id>-<variante>`, e o `icoKey()` cai no logotipo do app
quando não há arte para ela. O `icoHtml()` aceita a chave como terceiro
argumento — é assim que a linha da lista e o `#mTpvIco` do modal mostram o
logotipo certo. As quatro que existem — `radarr-uhd`, `sonarr-uhd`,
`radarr-anime`, `sonarr-anime` — são o favicon de 144px do addon do theme.park
reduzido a 96 (o dobro do quadro em que a página desenha), e por isso o
theme.park está creditado também na linha dos ícones. Acrescentar uma variante
é pôr o data URI no `ICONS`, mais nada.

**Serviço que entra sozinho.** Um checkbox no modal pode arrastar outro serviço
para a stack no momento do save — `vpn` traz o `gluetun` (obrigatório, porque o
`network_mode` depende dele) e `solver` traz o `flaresolverr`. O padrão é o
mesmo: flag no `SERVICES`, campo no `cfg`, e um `if(cfg.X && !has('y'))` no
handler de `#mSave`.

## Invariantes a preservar

- **Zero dependências externas em runtime**: os logotipos são data URI, o ZIP é
  feito à mão, a lista de fusos vem do `Intl` do navegador. Não introduza CDN,
  npm nem `fetch` — aberta do disco, a página tem de funcionar inteira.
- **Publicar porta no host é exceção, e vem com a flag `publish`.** O nginx
  ouve em 80/443 dentro do container e publica no host as portas do modal
  próprio dele — o "Editar" da linha fixa (`DEFAULTS.http`/`https` →
  `HTTP_PORT`/`HTTPS_PORT` no `.env`). A **443 só é publicada com o TLS
  ligado** — sem ele o `buildNginx()` não põe `server` nenhum escutando ali, e
  publicar seria ocupar a porta da máquina sem nada do outro lado; nesse caso
  nem a porta nem o `HTTPS_PORT` saem. O Seerr é o outro caso — ele não tem
  base URL configurável: em vez de rota,
  ganha `ports` no compose e a variável `<CNAME>_PORT` no `.env`, e o link dele
  aponta para a porta do host — em `http://`, porque o TLS mora no nginx e ele
  não passa por lá. Quem roteia pela VPN publica **no gluetun**, não em si
  mesmo: a rede é dele, e o compose recusa `ports` junto de
  `network_mode: service:`. Todo o resto só existe na rede `starrnet` e é alcançado por
  `container:porta-interna`; quem publica sai da contagem de rotas e do
  `buildNginx()` pelo `publishes()`. Quem roteia pela VPN usa
  `network_mode: service:gluetun` e responde no endereço do gluetun.
- **Volumes em sintaxe longa**, com `type: bind` e `bind.propagation: rslave`.
- **O bloco do nginx é `default_server`**: a conf é montada como um arquivo
  dentro do `conf.d`, ao lado do `default.conf` que vem na imagem — antes a
  pasta inteira era montada e ele sumia. Sem `default_server`, é o dele que
  atende quem chega sem casar com o `server_name`, e a stack toda responde a
  página de boas-vindas do nginx.
- **Cada subpath do nginx casa com a base URL do app**: nos *arr é a variável
  `<APP>__SERVER__URLBASE`; no Jellyfin é o `BaseUrl` do `network.xml`, que o
  servidor escreve depois de subir (`patch`, formato `xml`) em
  **`/config/network.xml`** — a raiz da pasta de config, ao lado do
  `system.xml`. Montá-lo não serve: o arquivo é do app, que migra a
  configuração de rede ao subir, e num nível errado ele existe e é ignorado —
  o app sobe sem base URL e o subpath responde 404, sem nada no log dizer por
  quê; no SABnzbd é o `url_base` do `sabnzbd.ini`, que o servidor
  escreve depois de subir. Serviço em subpath sem esse ajuste monta os links na raiz e
  quebra atrás do proxy — e é essa a razão de o **Seerr** publicar porta em
  vez de virar rota.
- **App sem base URL configurável pode virar rota se o prefixo for retirado**,
  e é a flag `stripBase` que faz isso — hoje só o **qBittorrent**. Em vez de
  `proxy_pass` direto, o `location ^~ /<rota>/` traz um `rewrite` que corta o
  prefixo, e o app responde na raiz, sem saber que existe subpath; os estáticos
  dele são relativos (`css/login.css`), então acompanham. Três detalhes que o
  bloco tem e não são enfeite:

  - o `location = /<rota>` com `absolute_redirect off` — sem a barra no fim o
    `rewrite` não casa, e sem o `absolute_redirect off` o nginx monta o destino
    com a **porta em que ele escuta** (80), mandando quem abriu em `:8080` para
    um endereço que não existe;
  - o `resolver 127.0.0.11` (o DNS do Docker), porque `proxy_pass` com variável
    resolve o nome a cada pedido — sem ele a rota responde 502 dizendo "no
    resolver defined";
  - as chaves de proxy reverso na conf do app (`ReverseProxySupportEnabled`,
    `TrustedReverseProxiesList=nginx`, `HostHeaderValidation=false`), que o
    `qbitPairs()` já escrevia. Sem elas a interface abre e a **API** responde
    403, que é o que o *arr consulta.

  Isto substitui a porta publicada do qBittorrent (e o `subpathFix` que
  reescrevia redirect e HTML dele, removido faz tempo). Foi conferido de ponta a
  ponta com o app atrás do nginx: HTML, estático, login, `app/version`,
  `app/preferences` e `torrents/info`, todos 200.
- **Serviço `internal` não vira rota**: gluetun e FlareSolverr existem para os
  outros containers, então ficam sem `location`, sem link e fora da contagem de
  rotas — mas continuam no compose. Ao acrescentar um serviço assim, use a
  flag; não espalhe `if(id==='…')`.
- Toda string visível ao usuário passa pelo `I18N`, nos três idiomas.
- **`id` do serviço ≠ imagem do container**: o `flaresolverr` roda a imagem do
  Byparr (`ghcr.io/thephaseless/byparr`), substituto direto. O `id` é o que vira
  `container_name`, subpath e upstream — trocar de imagem não deve mexer nele.
  O logotipo também segue o nome, não a imagem: o do Byparr é um cookie que aos
  20px da lista vira um ponto laranja, e já foi tentado e revertido.
- **Logotipo sempre sobre fundo claro**: os SVGs do dashboardicons são
  desenhados para isso e alguns são pretos (SABnzbd, Bazarr), então
  `--ico-bg` é claro nos dois temas. Não o amarre ao `--panel`. O do nginx é a
  exceção de origem — vem do IconScout (Icon 54) e por isso está creditado na
  seção de licença dos READMEs; ícone de outra fonte entra com o crédito junto.
- **Credenciais no formato do app**: a senha do qBittorrent sai em
  PBKDF2-SHA512, 100 mil iterações e sal de 16 bytes, como
  `base64(sal):base64(hash)`; a API key é `qbt_` + 28 caracteres de um alfabeto
  sem os parecidos. Os dois foram conferidos no fonte da 5.2.3 —
  `base/utils/password.cpp` e `base/utils/apikey.cpp`. Ao mexer nisso, confira
  no fonte da versão em uso; formato errado vira app que não abre. O hash é
  assíncrono (WebCrypto): `refreshQbitHash()` devolve a promessa e redesenha
  quando ela chega. Enquanto ela não chega, o gerador põe um comentário no
  lugar da linha da senha — por isso o `#dl` e o `#up` dão
  `await refreshQbitHash()` antes de chamar o `outFiles()`: sem isso dá para
  gravar uma conf sem senha nenhuma. A API key sai da `${STARR_APIKEY}` mapeada no alfabeto dele
  (`qbitKeyFrom`) — a conf é INI lido pelo app, não pelo compose, então pôr a
  variável ali seria texto morto. `DEFAULTS.qbitKey` vazio significa "acompanhe
  a chave da stack".
- **Favicon em três lugares, uma arte só**: o data URI no `<link rel="icon">`
  (o que faz o arquivo aberto do disco ter ícone), o `favicon.ico` da raiz (para
  quem serve a página) e o `docs/logo.svg` do título do README. Mudou a marca,
  mude os três — o `.ico` sai do SVG, rasterizado em 16…256. O `<link>` do
  `.ico` vem antes do SVG de propósito: o SVG tem precedência.

## Servidor (`backend/`)

Crate em Rust (axum + rusqlite `bundled`), com a página embutida por
`include_str!` — o cargo rastreia o `hubstarr.html` (ele está no
`target/debug/hubstarr.d`), então mexer na página recompila o binário.

**Quem está servindo a página é o binário, não o arquivo.** Um servidor no ar
entrega a cópia congelada no momento em que foi compilado, e é fácil haver mais
de um por perto — outro clone do repositório, um binário solto no `$HOME`. Antes
de concluir que uma mudança "não pegou", descubra qual responde: o `api/health`
devolve o `dir` e o `db` dele, e um `grep` de alguma marca recente na página
servida a data. E cuidado com a armadilha da porta: subir um segundo servidor
no mesmo endereço falha com `AddrInUse` **em segundo plano** — o processo novo
morre, o velho continua respondendo, e tudo parece só não ter mudado. A saída
dele diz; é a primeira coisa a olhar. `cargo test` roda os testes do modelo e da gravação de arquivos;
`cargo run` serve tudo em `127.0.0.1:7878`. Opções: `--addr`, `--dir` (padrão
`./stack`, a pasta em que os arquivos são gravados), `--db` (padrão
`~/.hubstarr/hubstarr.db`), `--docker`, `-v`.

O que ele escreve vai para a saída **e** para o `servidor.log`, ao lado do
banco — não na pasta da stack: o log é do servidor, e o `--dir` se apaga e se
refaz enquanto o `--db` dura. É `append`, nunca reescrita, porque o valor dele
é justamente o histórico entre reinícios; e o arquivo que não abre vira um
aviso na saída, não um servidor que não sobe. Quem cuida disso é o
`journal.rs`, e `println!` fora dele é sinal de linha que não vai ao arquivo.

Ali moram **duas alturas de log**, e a distinção é o que mantém as duas úteis:

- `record()` é o que sempre sai — a subida, o motor escolhido, cada
  `PUT /api/settings`. Curto o bastante para se ler dias depois; responde "o que
  mudou na minha stack?".
- `detail()` só existe com o **`-v`**, e é o passo a passo: cada arquivo
  gravado (o `files.rs` e as chaves que o `patch.rs` escreve na conf do app),
  cada linha mexida no banco (instância, Ambiente, Configuração, a lista de
  chaves) e **cada chamada às APIs dos apps**, com método, caminho e status.
  Responde "por que isso não funcionou?". Ligá-lo por padrão afogaria o
  primeiro: uma volta do Aplicar são dezenas de chamadas.

Duas regras do `detail()`: o argumento é uma **função**, para que sem o `-v`
nem o texto seja montado — dá para chamá-lo dentro de laço sem pensar; e **nada
de valor sensível na linha**. O Ambiente sai como lista de *nomes* de campo (a
chave da stack e as senhas estão entre os valores), e a URL sai pelo
`without_query()` onde a query leva a API key. Chamada nova de API deve passar pelo
`api()` do `apply.rs`, que é o único lugar que formata essa linha.

**A stack é uma só**, a da pasta do `--dir`: nenhum caminho da API leva id e
nenhuma tabela tem `stack_id`. Manter duas é rodar dois servidores, cada um com
o seu `--dir` e o seu `--db`. Já houve seletor de stack no cabeçalho, com
`POST`/`DELETE /api/stacks`, e foi removido; não refaça o caminho de volta.

O banco daquela versão é migrado na abertura, pelo `store/migrate.rs`, que roda
**antes** do `schema.sql` — o `CREATE TABLE IF NOT EXISTS` não mexe em tabela
que já existe, então as antigas são renomeadas para `old_*`, o esquema novo
nasce ao lado e a stack de menor id é copiada para dentro dele. As outras se
perdem, de propósito: não há mais onde guardá-las, e o `dir` de cada uma é
anunciado na saída. Duas armadilhas do SQLite ali: o `legacy_alter_table` tem
de estar ligado para o `RENAME` não reescrever as chaves estrangeiras das
outras tabelas, e o `foreign_keys` tem de ser desligado *depois* do
`schema.sql`, que o religa — senão nem o `SELECT` das `old_*` nem o `DROP`
delas passam.

**O contrato, que é o que quase não muda: nenhum arquivo da stack nasce no
servidor.** Ele recebe pronto o que os geradores do `<script>` montaram
(`outFiles()`), grava e roda o `docker compose`. Os geradores existem num lugar
só — se você for tentado a montar YAML no Rust, é sinal de que a mudança
pertence à página.

A **única** exceção é o `apply.rs` do v0.3, e ela tem limite claro: um cliente
de download não é arquivo — o *arr guarda isso no banco dele e só aceita pela
API —, então ali o servidor monta o corpo JSON do `downloadclient`. O que ele
monta é só o **formato da API do app** (implementação, contrato, a lista de
`fields`). Decisão nenhuma é dele: endereço, porta, categoria, quem recebe o
quê e o nome do campo de categoria de cada família chegam prontos no corpo do
`POST`, do `applyPayload()` da página, que é onde o `SERVICES` e o `CONFIG`
vivem. Ao acrescentar o Prowlarr ou o Media Management, siga essa divisão.

Módulos: `store/` (o modelo, com `migrate.rs` à parte), `files.rs` (grava o que veio, com `safe_join()`
recusando o que escapa da pasta, e o `ensure_dirs()` que cria as pastas dos
`source` do compose antes do `up` — o Docker criaria as que faltam, mas como
**root**, e aí o app, rodando com o PUID/PGID do Ambiente, não escreve na
própria configuração; quem lista as pastas é a página, no `outDirs()`, porque é
ela que monta os caminhos), `deploy.rs` (`docker compose up -d`/`down` na
pasta da stack, mais o `docker_ok()` que o `api/health` devolve — ele pergunta
por `docker compose version`, não só pelo docker, porque o plugin é pacote à
parte e é ele que sobe a stack; sem ele a página abre o bloco "Precisa instalar
o Docker?" e mostra o aviso `#noDocker`. Em quem esse teste é feito sai do
`pick_engine()`, resolvido uma vez na subida: o comando do `--docker`, se veio,
senão o primeiro dos `ENGINES` (`docker`, `podman`) que passar — o
`podman compose` roda o mesmo arquivo, e máquina que só tem ele não tem docker
nenhum a encontrar; o `up_one`/`stop_one` é um container só, o que o clique no
ponto de status da lista chama por `POST /api/service/:key/:action` — `up` sobe
com `--no-deps`, para não arrastar vizinho parado, e `down` é um `stop`, que
deixa o container existindo em vez de sumir. A chave vira argumento de comando,
então passa pelo `ok_service()` antes), `apply.rs` (v0.3: a Configuração inteira
aplicada pela API — clientes de download em cada *arr **e no próprio Prowlarr**,
cada *arr no Prowlarr, o Media Management mais a nomenclatura de cada família, e
as **pastas raiz** de cada instância.

A pasta raiz é o caminho **de dentro do container** (`/data` mais a subpasta da
instância), e quem o monta é a página, no `rootFolders()` — é ela que escreve os
binds do compose, então é ela que sabe o que o app enxerga. Mandar o caminho do
host não dá erro na hora: o *arr aceita e depois não acha arquivo nenhum. O
`ensure_root_folders()` acrescenta o que falta e **não tira** o que já está lá — remover
pasta raiz leva a biblioteca junto.

O **Lidarr** pede mais do que o caminho, e isso foi medido no app: o `Name` não
pode ser vazio e os dois perfis padrão (`defaultQualityProfileId` e
`defaultMetadataProfileId`) têm de ser maiores que zero — com só o `path`, a
resposta é uma lista de validação e a pasta não nasce. O nome sai do
`LIDARR_ROOT_NAME` (**`Music`**; nome repetido ele aceita, então duas pastas de
música não precisam de desempate) e os ids saem do `first_id()`, que lê a
lista do próprio app e cai no `1` — o de fábrica — quando não dá para ler.
Sonarr e Radarr não têm esses campos, então o ramo é só do Lidarr.

O **Jellyfin** é a única volta que não fala a API dos *arr: sem `X-Api-Key`,
com requisições próprias, e é o `StartupWizardCompleted` do
`/System/Info/Public` que escolhe o caminho. Assistente aberto é a janela em
que o `FirstTimeSetupOrElevated` dele aceita criar usuário e biblioteca **sem
token** — daí a ordem, com as bibliotecas *antes* do `Startup/Complete`.
Assistente fechado exige token, e ele sai do usuário e senha do modal
(`webAuth: 'jf'`); sem eles, é uma linha no log, não uma falha da stack. O
`Complete` só é chamado quando houve administrador a criar: fechar o assistente
sem conta nenhuma entrega um Jellyfin em que ninguém entra. Biblioteca que já
existe não é tocada e nenhuma é removida, pela mesma razão da pasta raiz. Quem
monta a lista é a página, no `jellyfinLibs()`, com o caminho **de dentro do
container** — e ele **não** é a pasta raiz do *arr: o Jellyfin monta a base
inteira em `/data` mais uma pasta por caminho de fora dela, e é contra esses
binds que a biblioteca tem de bater. Caminho que ele não enxerga não dá erro: a
biblioteca nasce vazia e calada, e é por isso que o `check-compose.py` a
confere contra os binds do compose.

Com o FlareSolverr na stack, ele também entra no Prowlarr, em Settings →
Indexers → Indexer Proxies, com a **etiqueta `flaresolverr`** — criada ali
mesmo, se não existir. A etiqueta não é enfeite: o Prowlarr casa proxy com
indexador por ela, e escolher quais indexadores precisam do resolvedor é a parte
que fica com quem usa. O endereço é o interno (`http://<cname>:8191`), porque o
serviço é `internal` e não tem rota no nginx; o nome do registro é o título da
instância, então a stack que roda o Byparr com outro nome aparece com ele.

O **qBittorrent** recebe ainda as preferências dele pela API, no
`client_preferences()`: o `app/setPreferences` com o corpo que a página
monta no `qbitPrefs()` — o gerenciamento automático de torrent
(`auto_tmm_enabled` e `torrent_changed_tmm_enabled`, que é o que faz o torrent
seguir a categoria quando ela muda), o `save_path` de dentro do container e o
usuário e a senha da interface. Não é repetição da conf do `patch.rs`: aquela é
o que ele lê ao **nascer**, esta é a mesma decisão aplicada a um qBittorrent que
já existe — e o TMM a conf nem cobre. O caminho sai do `qbitDl()`, o mesmo dos
dois lugares, para conf e API não discordarem.

A **API key** é do app, não nossa: a conf só a recebe quando ele ainda não tem
uma. Quem faz isso é o `keep_keys` do `patch.rs` — a lista de chaves que o merge
**não** sobrescreve quando o arquivo já traz valor —, e a página manda
`keep:['WebUI\\APIKey']` no patch do qBittorrent. Vazia ou ausente ela é
escrita, que é a primeira subida. A razão: uma vez que o app responda por uma
chave, é ela que os clientes dele conhecem, e trocá-la a cada Subir cortaria
quem já falava com ele.

A consequência vem junto, no `adopt_api_key()`: antes de registrar o cliente
em ninguém, o servidor lê a chave que o app tem e passa a usá-la na volta
inteira. Sem isso, "não sobrescrever" viraria o *arr registrado com uma chave
que o app não conhece — pior do que o problema que se queria evitar. App sem
chave nenhuma mantém a nossa, que é a que o `patch.rs` acabou de escrever.

A chave também vai no corpo do `setPreferences`, e o que ela faz ali é nada: medido no 5.2.3,
o `setPreferences` aceita o `web_ui_api_key`, responde 200 e **não muda o
valor** — a propriedade é espelho de leitura do `WebUI\APIKey` da conf, que é
onde ela se escreve de verdade (o `patch.rs`), e endpoint para criá-la não
existe (`apiKeys`, `generateApiKey` e afins dão 404). Por isso o servidor lê as
preferências de volta e **confere**: chave igual à do Ambiente vira linha só no
`-v`; diferente, ou vazia, vira aviso no log do trabalho — é por ela que os
*arr falam com ele, e um cliente registrado com chave que não abre nada falha
depois, longe daqui. Conferir não é falhar: quem manda na chave é a conf.

Duas coisas da API dele: o `setPreferences` recebe **formulário** com um campo
`json` (não um corpo JSON), e a sessão do `auth/login` vem num cookie cujo nome
muda com a porta (`QBT_SID_8181`) — por isso o que se guarda é o par inteiro,
como veio. E o `has_work()` passou a contar cliente com `prefs`: uma
stack de qBittorrent sem *arr nenhum tem trabalho, e antes ela passava em
branco.

No Prowlarr, o Settings → Download Clients recebe **um registro por cliente**,
todos na categoria `CAT_PROWLARR` (`prowlarr`): o que ele pega é avulso, não veio
de instância nenhuma, então fica junto e separado do que cada *arr baixa. O campo
ali é `category`, e o nome do registro é o do cliente — é por ele que o reaplicar
acha o que já está lá. E toda categoria precisa existir *dentro* do cliente, e nos
dois casos isso é feito **pela API do app**: no qBittorrent pelo
`torrents/createCategory` (corpo de formulário, `category=…&savePath=…`; quem
já existe volta **409**, e aí é o `editCategory` com o mesmo corpo, para
reaplicar acertar a pasta em vez de falhar), e no SABnzbd pelo
`mode=set_config&section=categories`. Nenhuma é removida: pode haver torrent
apontado para ela.

O `categories.json` continua **saindo no `.zip`** — é a saída de quem não tem
servidor, e ali não há API a chamar —, mas o servidor não o escreve mais: a
entrada dele no `conf` traz `viaApi`, que é o que o `outPatches()` pula. Essa é
a regra: arquivo do app que tenha endpoint equivalente vai por API, porque
escrever o arquivo exige **parar o container**, e parar o cliente de download no
meio do Aplicar é o que fazia os *arr testarem a conexão contra um app que
estava reiniciando. Quem chama é o **Subir**, sozinho, depois de
gravar as chaves dos `patch` — e antes de qualquer chamada ele espera cada app
responder no `/ping`, porque recém-subido nenhum responde e a volta inteira
falharia por timeout. Os clientes de download também são esperados, e por um
motivo próprio: escrever a conf do qBittorrent **reinicia** o container dele,
logo antes de os *arr o testarem. Em ambos os casos, 5xx conta como "ainda
não" — é o nginx dizendo que o de trás não subiu, e tomar isso por pronto é o
mesmo que não esperar.

O qBittorrent é registrado pela **API key**, não pela senha da interface: ela é
a mesma que a conf dele recebe, não expira quando a senha muda e é o que o campo
`apiKey` do schema espera — conferido nos dois lados: o schema do Sonarr traz
mesmo um campo `apiKey` no `QBittorrent` (ao lado de `username`/`password`), e
com só ele preenchido o teste de conexão passa. Do lado do app, o 5.2.3 lê a
chave do cabeçalho **`Authorization: Bearer`** (`webapplication.cpp`) e só a
considera se ela passar no `Utils::APIKey::isValid()`: prefixo `qbt_` e **32
caracteres no total**. Uma chave fora disso é **descartada em silêncio** na
subida — o app fica sem chave nenhuma, a autenticação por ela nunca entra, e o
*arr leva 403 sem que nada diga por quê. É o que o `api_key_valid()` do
`apply.rs` recusa antes de mandar, e o que o `qbitKeyFrom()` da página já
produz. Usuário e senha só vão para o app cujo schema não tem
esse campo — versão antiga —, e quem decide isso é o próprio schema.

O corpo de cada `downloadclient` nasce do **schema que o app publica**
(`/downloadclient/schema`), com os nossos valores por cima: mandar só os nossos
deixa o resto nulo, e o app estoura ao testar a conexão. O do Prowlarr leva
ainda um `categories: []` — é uma propriedade que os *arr não têm, e ausente ela
vira nula dentro do `ValidateCategories` dele, com um
`NullReferenceException` que não diz nada sobre a causa. Os apps são alcançados
pelo nginx, porque o servidor roda no host e a rede `starrnet` não existe para
ele; aplicar de novo procura pelo nome e atualiza no lugar, e um app fora do ar
vira uma linha no log em vez de derrubar a volta inteira.

**Chamada que não chega se repete**, no `retry()`: dez vezes, cinco segundos
entre elas. E só o que é "não consegui acessar" — erro de transporte e resposta
**5xx**, que atrás do nginx é ele dizendo que o container ainda não subiu. Erro
do app (400, 401, 404, a validação recusando o corpo) **não** se repete: a
resposta seria a mesma dez vezes, e cinquenta segundos por chamada numa volta de
dezenas delas transformaria erro de configuração em espera sem fim. Isso não
substitui o `wait_apps()` — aquele é a espera única, antes de começar; o
`retry()` é a rede para o que cai **no meio**: o qBittorrent reiniciando ao
receber a conf, o *arr ocupado importando. As tentativas saem no `-v`
(`tentativa 3/10`), e o custo do pior caso é somável: app que nunca responde
gasta os 90s da espera **mais** 50s por chamada.

A requisição é montada pela função a cada tentativa, e não clonada: corpo
consumido não se reaproveita, e o `try_clone()` do reqwest devolve `None`
justamente quando há corpo. Ao acrescentar chamada nova, monte-a dentro do
`retry()` em vez de chamar `.send()` direto. Três coisas para não
reaprender: o que vai *dentro* da aplicação do Prowlarr é o endereço interno, de
container para container, e com a base URL junto — sem ela a API do *arr fica na
raiz, onde não existe; `naming` e `mediamanagement` são recursos únicos e cheios
de campo que a página não mostra, então são lidos, mexidos nas chaves do
`naming_map()`/`MEDIA_MANAGEMENT` e devolvidos inteiros, nunca montados do zero;
e as opções de lista viajam pelo nome e chegam como número, pela ordem do
`COLON`/`MULTI_EP`, que é a mesma da página — nome fora da lista é erro, não
zero), `patch.rs` (escreve chaves na configuração que o próprio app cria:
espera o arquivo aparecer, **para** o container, faz o merge no INI e sobe de
novo — parar é o que impede o app de sobrescrever o que gravamos, porque ele
despeja a configuração em memória no disco justamente ao sair, e por isso
também o arquivo é lido *depois* do stop. O merge só troca as chaves que
vieram; comentário, ordem e o que o app guardou ficam), `jobs.rs` (trabalhos numerados com log incremental, em memória
— subir a stack baixa imagem e não cabe numa resposta HTTP), `shots.rs` (cache
em disco das capturas de paleta do theme.park, ao lado do banco, servido em
`api/shot/:app/:theme` — o `ok_segment()` recusa segmento que escaparia da pasta ou
do domínio, e o repositório continua sem redistribuir captura de ninguém: a
primeira visita sai para a documentação deles. Aberta do disco, a página busca
lá direto, como sempre).

O modelo é **normalizado**, uma tabela por conceito do estado da página:
`stack_env` (o `DEFAULTS`, uma coluna por chave, mapeadas em `ENV_COLS`, numa
linha só — o `CHECK (id = 1)` é o que a mantém única), `instance` +
`instance_lib` (o `added`), e `cfg_app`, `cfg_client`, `cfg_client_arr`,
`cfg_mm`, `cfg_naming` (o `CONFIG`). O que pende de outra tabela vai com
`ON DELETE CASCADE`. Três coisas a respeitar:

- **A chave da instância é o `cname()`** — o `container_name`. Editar o título
  muda a chave, então o `PUT` carrega o `old` e o editar vira um renomear.
- **O `PUT /api/settings` é o único caminho que apaga instância sem ninguém ter
  clicado em "Excluir"**: manda a lista de chaves, e o `reconcile()` tira o que
  não veio nela. Página com a lista errada — a que não conseguiu ler o estado,
  uma aba velha voltando à vida — apaga a stack por aí. Por isso cada PUT deixa
  uma linha na saída do servidor, com a hora, quantas chaves vieram e **quais
  saíram**: quando isso acontecer de novo, o log diz quem mandou o quê em vez de
  sobrar especulação. O `reconcile()` devolve o que apagou justamente para essa
  linha.
- **`cfg_mm` é por `service_id`**, não por instância: Media Management é por
  família, como na página.
- **`instance.extra`** guarda o que não virou coluna e volta espalhado no
  objeto. Uma flag nova no `SERVICES` não exige migração — só acrescente à
  `COLUMNS` o que precisar de coluna de verdade.
- **Chave nova no Ambiente é coluna nova, e o `schema.sql` sozinho não a
  acrescenta**: o `CREATE TABLE IF NOT EXISTS` não mexe em tabela que já
  existe. Quem a põe no banco de quem já tinha stack é o `ensure_env_cols()`,
  que roda na abertura, compara o `ENV_COLS` com o `PRAGMA table_info` e faz o
  `ALTER TABLE` do que faltar. Não é zelo: o `SELECT` do `env()` nomeia todas
  as colunas, e uma faltando derruba **toda** leitura do Ambiente — a página
  entende isso como banco vazio e o primeiro save apaga as instâncias. Já
  aconteceu, com o `jf_user`/`jf_pass`.

Tirar um serviço do catálogo não tira a instância dele do banco de quem já a
tinha. Por isso o `applyState()` filtra o que voltou
pelo `svc(id)`: sem isso, a página morre no primeiro render, procurando a cor de
um serviço que não existe, e a lista some inteira. A linha sai da interface e,
no primeiro `saveSettings()`, sai do banco junto.

O `api/health` também devolve o `puid`/`pgid` do processo — lidos do dono de
`/proc/self`, sem crate a mais. São eles o padrão de fábrica do PUID/PGID do
Ambiente quando há servidor: é ele quem cria as pastas da stack, e o app precisa
ser o mesmo dono para escrever nelas. O `detectServer()` os aplica **antes** do
`openStack()`, então o que estiver guardado no banco continua mandando.

`load()` remonta `{added, defaults, config}` na forma exata que a página espera,
e devolve `None` quando o banco ainda não tem nada guardado — assim a página
fica com os próprios padrões em vez de recebê-los em branco de volta. Esse ida e
volta sem perda é o critério do modelo; ao mexer nele, é o que os testes cobrem.

O botão **Aplicar na stack** só aparece com a stack **no ar**: ele fala com os
apps pelo nginx, e com tudo parado a volta seria dezenas de chamadas para
ninguém. Quem responde por isso é o `stackOnline()` da página, sobre o mesmo
`STATUS` do ponto da lista — daí o `paintApply()` ser chamado no
`paintStatus()`, e não só no `openCfg()`: stack que sobe (ou cai) com o modal
aberto muda o botão junto com os pontos.

O modal do log — o **Subindo a stack**, o **Derrubando a stack** e o **Aplicando
a Configuração** — trava o Fechar enquanto o trabalho corre: fechá-lo não
cancelaria nada, porque quem roda o `docker compose` e as chamadas de API é o
servidor, e sumiria com o único lugar em que dá para acompanhar. O `runJob()` o
libera quando o trabalho termina, tendo dado certo ou não — inclusive quando ele
nem começa, que é o caso do servidor fora do ar.

Quem quer sair antes do fim tem o **Parar**, ao lado do Fechar: ele chama
`POST /api/job/:id/stop`, e ali o `jobs.rs` **aborta a tarefa de dentro** — a
mesma que o pânico já matava —, de modo que o trabalho termina como falha
comum, com `done` escrito e o Fechar de volta. Não é o clique que libera o
modal, é o fim do trabalho: assim a tela nunca fica adiantada em relação ao
servidor. O `docker compose` que estiver rodando morre junto pelo
`kill_on_drop(true)` do `deploy.rs` — sem ele o processo continuaria sozinho,
sem ninguém lendo a saída. Parar deixa a stack no meio do caminho (containers
meio subidos, Configuração meio aplicada), e é isso que a `log.stopped` diz e o
`record()` do servidor guarda.

**Toda saída daquele laço tem de passar pelo `endLog()`.** Enquanto ele gira, o
Fechar está desabilitado, então um caminho que não termine prende o modal para
sempre — com a tela parada, que é o pior jeito de falhar. Foram dois buracos
assim, um de cada lado:

- na página, a busca do trabalho que falha (servidor que caiu, trabalho que ele
  não conhece mais — eles vivem em memória) contava como "ainda correndo"; hoje
  ela conta as falhas seguidas e desiste depois de `ATTEMPTS`, com a
  `log.lost` no log;
- no servidor, pânico dentro do trabalho matava a tarefa antes do `done`, e a
  página perguntava por ele para sempre. O `jobs.rs` roda o trabalho numa
  tarefa de dentro e espera pelo `JoinHandle`, o que transforma o pânico numa
  falha comum.

Do lado da página, a seção `/* ---------- servidor ---------- */`:
`detectServer()` só faz algo em `http(s)://` e chama `openStack()`, que carrega
o estado guardado — sem id nenhum, porque a stack é a do servidor.
`putInstance`/`delInstance` mexem numa linha por vez, e `saveSettings()`
(debounce no fim do `render()`) manda Ambiente, Configuração e a lista de chaves — é ela que acerta a ordem e apaga o
que saiu sem passar pelo modal. A flag `loading` existe para o estado que vem do
banco não ser gravado de volta enquanto está sendo aplicado.

A outra flag, a `readOnly`, é a rede de segurança dessa mesma lista de chaves:
carregamento que **falhou** (qualquer resposta do `api/state` que não seja 200
ou 204) deixa a tela com uma stack vazia que não é a do banco, e seguir daí
grava essa lista por cima — o `reconcile()` apaga o que não vier nela. Então o
`openStack()` a liga, o aviso `#noState` aparece e as três funções que gravam
(`putInstance`, `delInstance`, `saveSettings`) desistem até alguém recarregar a
página. 204 é banco vazio e continua sendo começo normal.

## Perfis de qualidade (v0.4)

Os perfis e os custom formats **não são montados aqui**: quem os aplica é o
[Configarr](https://configarr.de), um serviço do catálogo, com os dados do TRaSH
Guides e os templates do Recyclarr. A página só **escolhe** — e é essa a divisão
a preservar. Montar perfil no `apply.rs` seria reimplementar o Recyclarr e
envelhecer junto com o guia.

O que a página tem é o `PROFILE_PRESETS`: por família, um preset é o trio que o
guia recomenda junto — `p` (o perfil), `cf` (os custom formats que o pontuam) e
`q` (a definição de tamanho dos arquivos). Os ids são **nomes de arquivo do
repositório de templates** e por isso não passam pelo `I18N`; traduzido é só o
rótulo (`cfg.p.<chave>`). A página não confere se o nome existe — sem rede não
teria como, e fingir que confere seria pior: nome errado vira erro no log do
Configarr.

O `profTemplates()` monta a lista de uma instância, e a regra que não é óbvia
mora nele: perfil e custom formats de **todos** os presets marcados entram,
porque são recursos separados dentro do app e ter dois perfis é o objetivo; a
**definição de qualidade, não** — ela é única no app, e a de anime não é a de
série. Vale a do primeiro preset marcado, e as outras saem comentadas no
arquivo, que é onde alguém percebe. O Lidarr fica fora da seção inteira: o
Recyclarr não publica template para ele.

São **dois** arquivos, os dois em `<config>/configarr/`: o `config.yml` e o
`secrets.yml`, que é onde a chave dos *arr mora — o `config.yml` a lê com
`!secret`. O `secrets.yml` não é opcional: o `docker run` o monta, e arquivo que
não existe o docker cria como **pasta**, e aí o Configarr sobe sem chave nenhuma.
O `base_url` vai com a **base URL junto**, pela mesma razão do registro do
Prowlarr: sem ela a API do app fica na raiz, onde não existe. Instância
desmarcada sai com `enabled: false` em vez de sumir do arquivo.

Os dois arquivos e as três pastas dele (`repos`, `custom_formats` e a própria)
saem do `outFiles()`/`outDirs()` **à parte**, porque ele não tem entrada no
catálogo — existem porque há perfil a aplicar, não porque um serviço está na
stack.

**O Configarr não é serviço da stack.** Ele não está no `SERVICES`, não entra
no compose e não tem ponto de status: é uma ferramenta que o servidor roda com
um `docker run --rm` avulso (`deploy::configarr`), **depois** do
`wait_apps()` — no fim do Subir e no "Aplicar na stack". Num `up -d` ele
subiria antes de os apps responderem, e como sai sozinho o ponto dele viveria
vermelho. Nos Créditos ele aparece pelo `NGINX_CRED`, ao lado do nginx: crédito
é de quem faz o trabalho, não de quem está no compose.

Três coisas do `docker run` que custaram uma rodada cada:

- **`--network starrnet`** só acha os apps porque o compose fixa o nome da rede
  com `name:`. Sem isso ele a prefixa com o nome do projeto (a pasta do `--dir`)
  e ela vira `stack_starrnet` — quem chega de fora do compose não a encontra
  pelo nome que a página anuncia. Stack que já existia recria os containers na
  primeira subida depois dessa mudança.
- **`--user PUID:PGID`** faz o cache dos repositórios ser de quem o Ambiente
  diz. Cache clonado por outro dono trava o git ("dubious ownership", depois
  "Permission denied") — o `GIT_CONFIG_*` cobre o primeiro caso, e o segundo
  vira uma linha no log dizendo para apagar a pasta, que se refaz sozinha.
- **`--dns`** porque a rede da stack é uma bridge nossa: sem ele o clone do
  TRaSH e do Recyclarr falha em máquina cujo resolvedor não é alcançável de
  dentro dela.

A página manda no corpo do `apply` só o que a linha de comando precisa —
`{dir, network, user, tz}` —, e `null` quando não há perfil nenhum a aplicar.
Nem os perfis nem a chave passam por ali: eles estão nos dois arquivos. No
banco, é a tabela `cfg_profile`, por `arr_key` (o `cname()`), com os presets em
JSON.

Uma ligação que não passou (um app fora do ar) **não cancela os perfis**: são
coisas independentes, e o erro do `download_clients` volta no fim, depois de o
Configarr ter rodado.

## READMEs

`README.pt-BR.md` é a fonte; o `README.md` (inglês, o padrão do repositório) e
o `README.es.md` são traduções.
Mudança de comportamento documentada precisa ir aos três. As capturas em
`docs/` (`screenshot.png`, `services.png`, `theme.png`, `credits.png`,
`config.png`) refletem a interface atual, e há uma seção **Docker** explicando como instalar
o que roda os arquivos gerados. O badge da licença é um SVG local por README
(`docs/badge-licen*.svg`, um por idioma, com o texto em `textLength` fixo) — nada de shields.io: o repositório
não busca imagem de fora.

Para refazer as capturas, copie o HTML para um arquivo temporário fora do
projeto (o chromium do snap não lê `/tmp` nem `/srv`), injete no fim do
`<script>` o que a captura precisa — `setTheme('dark')` (as quatro capturas
estão no tema escuro), o `added` da stack de exemplo,
`$('#combo').classList.add('open')`, `openModal('sonarr',null)` mais
`openShot()` na da paleta, `openCred()` na dos créditos, `openCfg()` mais o `scrollTop` do `#cfgBody` na
da Configuração (e um `SERVER` de mentira mais um `STATUS` com algum
container `running`, senão o "Aplicar na stack" não aparece) — e rode:

```sh
chromium-browser --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1480,760 --virtual-time-budget=4000 \
  --screenshot=$HOME/out.png "file://$HOME/tmp.html"
```

Duas coisas que a `screenshot.png` pede além disso: **forçar o idioma** com um
`setLang('pt-BR')` — ele fica no `localStorage` do perfil do chromium, e uma
captura anterior em outro idioma contamina a próxima, em silêncio — e abrir a
Wishlist pelo `open` do `<details>` dela, deixando o bloco do Docker fechado.

`services.png`, `theme.png` e `credits.png` são 1480×760, a `config.png` é
1480×900 — o modal é denso e em 760 não caberia o que ela mostra — e a
`screenshot.png` acompanha a altura do conteúdo (hoje 1898, com a Wishlist
aberta). A
`theme.png` é a única que precisa de rede: o modal da captura busca a imagem
em `docs.theme-park.dev`. O mesmo truque, com `--dump-dom` no lugar de `--screenshot`, é a
maneira de testar mudanças de comportamento sem navegador interativo. Se o
chromium travar sem escrever nada, passe um `--user-data-dir` próprio.

O favicon não aparece em captura nenhuma: o headless fotografa só o viewport,
sem a barra de abas.

Injeção que depende de trabalho assíncrono — o hash da senha do qBittorrent, por
exemplo — não é confiável com `--dump-dom`: o `--virtual-time-budget` pode
encerrar a página antes de a promessa resolver, e o resultado sai vazio sem erro
nenhum. Estruture a injeção para não depender dela, ou confira o valor por outro
caminho.

Ao injetar código, ancore no fim do `<script>` (`detectServer();\n</script>`): a
linha `applyI18n(); renderCombo(); renderItems(); render();` sozinha também
aparece dentro do `setLang()`, e substituí-la lá dentro leva a recursão
infinita. A página abre sem modal nenhum; a captura que precisa de um chama o
`openEnv()`/`openCfg()` dela na injeção.

Duas armadilhas do `added` injetado, as duas já custaram uma rodada:

- **O nginx não entra nele.** Ele é a linha fixa, montada à parte do catálogo,
  e não tem entrada no `SERVICES` — um `{id:'nginx'}` no `added` estoura o
  render em `.color`, e aí a página fica no estado que tinha antes da injeção:
  só as linhas fixas, que é uma captura plausível o bastante para passar
  despercebida. Envolva a injeção num `try/catch` que escreva o erro no
  `document.title` e leia com `--dump-dom` antes de fotografar.
- **`openShot()` precisa do `picked`.** Ele resolve a instância por
  `editing ? byKey(editing).id : picked`, então `openModal('sonarr', null)`
  sozinho não basta: sem `picked` ele volta calado e a captura sai com o modal
  do serviço, sem o da paleta. A `theme.png` também escolhe a paleta na mão
  (`$('#mTp').value` + `tpShot(id)`) antes de abrir.

## Wishlist

O roadmap fica nos três READMEs, numa tabela por marco de versão; o texto
autoritativo é o do `README.pt-BR.md`, e mexer nele é mexer nos três. Hoje o
repositório é o **v0.5** — a página, o servidor de `backend/`, a Configuração
aplicada nos apps e o TRaSH Guides inteiro pelo Configarr.

- ~~**v0.2**~~ — feito: o backend liga o `hubstarr.html` ao Docker e guarda a
  stack. Uma primeira versão dele existiu e foi removida no `ba54e1a`; a de
  agora é normalizada.
- ~~**v0.3**~~ — feito: a **Configuração** é aplicada pela API de cada app, no
  `apply.rs`, pelo **Subir** (sozinho, quando os apps respondem) e pelo botão
  "Aplicar na stack" do modal dela, que reaplica sem subir nada — as três partes
  (`CONFIG.clients`, `CONFIG.apps` e `CONFIG.mm`) numa passada só. Duas escolhas
  que não são óbvias: as categorias que o Prowlarr sincroniza por família saem
  explícitas do `sync_categories()`, porque campo vazio ali é um Prowlarr que
  não sincroniza indexer nenhum sem dizer nada; e o `useExisting` do Lidarr não
  vira campo de API — ele mostra e esconde os formatos na interface, e o que
  descreve é o `renameTracks` desligado, que já vem do `rename`. A chave da API do SABnzbd é o
  próprio app que a gera na primeira subida, então ela é um campo do modal dele
  (flag `dlKey`) e mora na instância, não no Ambiente — não vai para arquivo
  nenhum, serve só para o Aplicar.
- ~~**v0.4**~~ — feito: perfis de qualidade e custom formats por instância, do
  TRaSH Guides, aplicados pelo Configarr — ver "Perfis de qualidade" acima.
- ~~**v0.5**~~ — feito junto do v0.4, e não por acaso: cada preset manda o trio
  do guia — o perfil, os custom formats **com os scores dele** e a quality
  definition. Era isto que o marco pedia, e veio dos templates do Recyclarr em
  vez do JSON do TRaSH lido na mão.
- **v0.6** — nome de projeto e de container configurável. Hoje o
  `container_name` é o `cname()`, fixo: subir uma stack numa máquina que já
  tenha um `sonarr` ou um `nginx` **toma o nome do que está rodando**, e o
  `docker compose down` de uma pasta chamada `stack` leva junto o que outra
  pasta de mesmo nome criou — o Compose tira o nome do projeto do nome da
  pasta. Aconteceu duas vezes numa sessão só. Resolve com um prefixo no
  Ambiente, e de quebra reabre o caminho para mais de uma stack, que foi
  removido no passado.
- **v0.7** — busca localizada de mídia, com o idioma da busca escolhível.

Marco é ordem, não calendário: cada um depende do anterior. Ao propor mudança
que caia num deles, diga em qual — e não comece o de baixo antes do de cima.

## CI

`.github/workflows/ci.yml` roda a cada push e PR, em dois trabalhos. O do
**servidor** é `cargo build`, `cargo test` e `cargo clippy -D warnings`, todos
com `--locked` — por isso o `backend/Cargo.lock` é versionado. **Não** há
`cargo fmt`: o projeto indenta os comentários de bloco junto do código que eles
explicam, e o rustfmt os joga na margem; o estilo é escolhido.

O da **página** são três checagens, em `tools/`, que rodam iguais na sua
máquina:

- `extract-script.py` tira o `<script>` da página para o `node --check` —
  ele quer um arquivo de verdade, substituição de processo não serve.
- `check-i18n.py` compara as chaves dos três idiomas. Chave que falta num
  idioma aparece na interface como o próprio nome dela, e só quem troca de
  idioma vê. Ele acha também chave repetida no mesmo bloco, onde a segunda
  vence em silêncio.
- `check-compose.py` abre a página num chromium sem tela, monta uma stack de
  exemplo com um pouco de cada forma (duas instâncias da mesma família, VPN,
  porta publicada, GPU, serviço `internal`) e passa o que ela gerou pelo
  `docker compose config`. É a que mais paga: o texto dos arquivos sai do
  `textContent` dos panes, e erro de indentação ou de `${...}` só aparece
  quando o docker recusa o arquivo. A pasta temporária dele vai no `HOME`, não
  em `/tmp` — o chromium do snap não lê `/tmp`, e a página abriria em branco.
  Ele relê o compose gerado (com o `yaml`, que o CI instala) e confere contra os
  binds duas coisas que o app aceitaria calado: as **pastas raiz** dos *arr e as
  **bibliotecas** do Jellyfin. Caminho que o container não enxerga não vira erro
  em lugar nenhum — vira biblioteca vazia.

`release.yml` roda só em tag `v*`: compila o servidor para x86_64 e arm64 (o
arm64 precisa do `gcc-aarch64-linux-gnu`, porque o rusqlite traz o SQLite
embutido), e publica os dois com o `hubstarr.html` da mesma versão.

## Commits

Mensagens em português, no imperativo/terceira pessoa do singular, uma linha
("Copia o link de cada serviço", "Serve a stack por HTTPS, com certificado
configurável"). Corpo só quando explica o porquê, não o quê. Um assunto por
commit, mesmo quando as mudanças estão no mesmo arquivo.

Pedido de commit já inclui o push: `git commit && git push origin master`, num
comando só. Se vier um "push" depois, ele já saiu — responda com o estado
(`git status -sb` e o último commit) em vez de repetir o comando.

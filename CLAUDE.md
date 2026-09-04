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
   pastas avulsas do Jellyfin, do "+ pasta", cada uma um `{name, path}` — o
   `name` é o **alias**, e vazio quer dizer "chame-a pelo nome da própria
   pasta", que é o que toda linha guardada antes do campo existir tem), `picked` (id no
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
   os links no mesmo IP; aberta do disco, sobra o `127.0.0.1`. **`localhost`
   nunca entra**, nem como padrão nem vindo do `location.hostname`: hoje ele
   costuma resolver para `::1`, e a porta que o Docker publicou só em IPv4 não
   tem ninguém do outro lado — é o `LOOPBACK` que o troca pelo endereço), `cfgPath`/`dataPath` (com variáveis
   `${...}` do `.env`) e `cfgReal`/`dataReal` (caminhos resolvidos, para o hint
   do modal). Alterar `cname` afeta compose, nginx e `.env` ao mesmo tempo.
   `dupPaths()` compara os caminhos já resolvidos e avisa, no rodapé da lista,
   quando duas instâncias caem na mesma pasta — Jellyfin e Bazarr ficam fora,
   um monta a biblioteca inteira e o outro segue as instâncias.
   Quem monta pasta de outro serviço passa por `derivedMounts()` (Bazarr) e
   `extraLibs()` (Jellyfin, que junta as pastas de fora das outras instâncias
   com as do "+ pasta") — os dois já devolvem o caminho certo de cada instância,
   literal ou com variável; não remonte `${BASE_MEDIA}/…` na mão. O `name` que o
   `extraLibs()` devolve é o alias da linha, e sem ele o nome da própria pasta:
   é ele o volume (`/data/<name>`) e o nome da biblioteca no Jellyfin, e é ele
   que separa dois discos com uma pasta de mesmo nome em cada.
6. **UI** — as etiquetas da linha do serviço saem todas do `tagHtml(kind,
   texto)`, e o `kind` é o que dá a cor no CSS (`.tag[data-kind=…]`) e o rótulo
   no `I18N` (`tag.<kind>`). O `TAG_KINDS` é a lista deles, na ordem de leitura,
   e é dele que o `renderLegend()` monta a legenda abaixo da lista —
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
   (`ini`, `yaml`, `json` ou `xml`) e quem monta os dados. O `yaml` é o
   `config.yaml` do Bazarr — ver "Idioma da busca" abaixo. O `xml` é o `network.xml` do
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
`slug()` no valor cru; e, com servidor, ganha também o `📁` — ver "Navegador de
arquivo".

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

## Navegador de arquivo

Todo campo de caminho tem um `📁` que abre o `#fbBack`, com as pastas da máquina
em que o **servidor** roda — e por isso ele só existe com servidor
(`body.srvOn`): aberta do disco, a página não alcança sistema de arquivos
nenhum, e os campos continuam digitados à mão. São os do Ambiente (as três
bases, mais o certificado e a chave, que abrem em `mode:'file'`) e os do modal
do serviço: a subpasta de mídia (`#mData`), as duas do SABnzbd (`#mDlIn`,
`#mDlDone`) e cada linha do `libRow()` do Jellyfin. A entrada é o
`openBrowse({path, mode, onPick})`, e o `onPick` **só preenche o campo** — quem
grava continua sendo o Salvar de cada modal.

Botão novo é uma linha de HTML, não um handler: o clique é **delegado** no
`document`, porque a linha do Jellyfin nasce em tempo de execução. O botão diz
qual campo preenche — pelo `data-pick`, quando o campo tem id, e senão por estar
ao lado dele na linha — e o `data-base` diz onde começar quando o campo está
vazio.

Cinco coisas dele:

- **A página não decide onde está.** Quem resolve o caminho é o `browse.rs`: a
  listagem cai na pasta existente mais próxima da pedida, e o `fbAt` é sempre a
  resposta dele, nunca o que se pediu.
- **O que entra no campo é o caminho como está**, sem variável: é o mesmo que
  digitar produz, porque o `expandInto()` resolve o `${...}` enquanto se digita,
  e o `dataOf()`/`realToVars()` põem a variável de volta na frente a caminho do
  compose.
- **Subpasta guarda os níveis.** O `dataOf()` fatia por `/` e passa o `slug()`
  em cada pedaço (`subSlug`): pasta escolhida dois níveis abaixo da base vira
  `series/4k`, e não o `series-4k` de achatar tudo de uma vez — que seria montar
  uma pasta que ninguém escolheu. Todo lugar que usa a subpasta a cola numa
  base, no host e dentro do container, então mais de um nível não custa nada.
- **A lista é montada elemento a elemento**, não com `innerHTML`: os nomes vêm
  do disco e podem ter qualquer coisa dentro.
- **Ele abre por cima de outro modal** — o único que faz isso além da captura da
  paleta —, daí o z-index e o lugar dele na ordem do Escape: fechá-lo não pode
  levar o modal de baixo junto com tudo o que foi digitado ali.

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
- **A conf do nginx é montada *sobre* o `default.conf` da imagem**, como
  arquivo (nunca a pasta inteira — o mount de pasta é o que já fez a nossa conf
  sumir). Ao lado não serve: o arquivo que vem na imagem declara
  `server_name localhost`, e **casar o nome exato ganha do `default_server`** —
  a mesma stack respondia `/sonarr` em `http://127.0.0.1` e um 404 pelado do
  nginx em `http://localhost`, porque o segundo pedido nunca chegava ao nosso
  bloco. Medido com a conf gerada e um Sonarr de verdade: `Host: localhost` →
  404, qualquer outro Host → 200; montando por cima, 200 nos dois. O bloco
  continua `default_server`, que é o que atende quem chega sem casar com o
  `server_name`.
- **Cada subpath do nginx casa com a base URL do app**: nos *arr é a variável
  `<APP>__SERVER__URLBASE`; no Jellyfin é o `BaseUrl` do `network.xml`, que o
  servidor escreve depois de subir (`patch`, formato `xml`) em
  **`/config/network.xml`** — a raiz da pasta de config, ao lado do
  `system.xml`. Montá-lo não serve: o arquivo é do app, que migra a
  configuração de rede ao subir, e num nível errado ele existe e é ignorado —
  o app sobe sem base URL e o subpath responde 404, sem nada no log dizer por
  quê; no SABnzbd é o `url_base` do `sabnzbd.ini` e no Bazarr é o
  `general.base_url` do `config.yaml`, que o servidor escreve depois de subir. Serviço em subpath sem esse ajuste monta os links na raiz e
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

Guia específico do servidor Rust mora em `backend/CLAUDE.md` — carrega só
quando você mexe em arquivos debaixo de `backend/`, não em toda sessão.

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

## Idioma da busca (v0.7)

`DEFAULTS.searchLang` (vazio = "não mexer no idioma de app nenhum") mora no
**Ambiente**, porque é da stack inteira, como o `TZ`. A tabela é o
`SEARCH_LANGS`: por código, o que **cada app espera ouvir** — o nome do idioma
como o Radarr o publica, o BCP-47 do Jellyfin, o código de duas letras do
Bazarr e o sufixo do template do Recyclarr. A tabela é da página, e não passa
pelo `I18N`: são valores de API, como os ids do `PROFILE_PRESETS`.

O marco tem **três metades, e cada uma vai por um caminho diferente** — é essa
a divisão a preservar:

- **metadados**, pela API, no `apply.rs` (`metadata_language()` e, no Jellyfin,
  o `server_language()` mais o `LibraryOptions` de biblioteca nova);
- **lançamento**, pelo **Configarr**, trocando o par do preset pelo do idioma
  (`presetPair()`); montar custom format de idioma no Rust seria reimplementar
  o Recyclarr, que é o que a seção "Perfis de qualidade" já proíbe;
- **legenda**, pela API do Bazarr (`bazarr()` no `apply.rs`).

Três coisas que não são óbvias:

- **O Sonarr não entra na primeira metade.** O `config/ui` dele não tem idioma
  de metadados, só `uiLanguage`, que é a **interface** — conferido no
  `UiConfigResource.cs` das duas casas: o do Radarr traz `MovieInfoLanguage`, o
  do Sonarr não. Trocar a interface de quem pediu títulos em outro idioma é
  surpresa, então o Sonarr recebe idioma só pela metade do lançamento. Isso
  está dito nos READMEs de propósito: sem essa linha, a promessa do marco fica
  maior do que o que o app permite.
- **O `culture` do Jellyfin continua saindo do idioma da página.** Ele é a
  *interface* (`UICulture`), e a decisão de sempre; o que o `searchLang` manda é
  o `metaLang`, à parte, e sem ele tudo volta a ser o que era. Também foi tapado
  ali um buraco antigo: o idioma só era aplicado dentro do `wizard()`, então
  Jellyfin já configurado ignorava o campo — agora o ramo autenticado escreve o
  `/System/Configuration`.
- **Só francês e alemão têm template no guia.** É o que o `TPL_SEED` já
  mostrava, e os nomes do `lang` de cada preset vieram de lá — nada inventado.
  Idioma sem template deixa os perfis como estão; nome errado viraria erro no
  log do Configarr, e a página não tem rede para conferir.

O `config.yaml` do Bazarr leva **duas** chaves, e a segunda é o
`general.base_url`: ele é serviço em subpath como qualquer outro, e estava sem
base URL nenhuma desde sempre — os links dele saíam na raiz. `route()` já dá a
forma que ele quer, com a barra na frente e nenhuma atrás.

A chave de API do **Bazarr** acompanha a `${STARR_APIKEY}`, como a do SABnzbd:
o `bzKey()` cai no `DEFAULTS.apiKey` e o servidor a escreve no `config.yaml` do
próprio app, por `patch`, antes de qualquer chamada. O app gera uma sozinho na
primeira subida — 16 bytes em hexadecimal, a mesma forma do `randKey()` —, e
por isso sobrescrevê-la não custa nada: ninguém além de nós fala com ele. A
flag `appKey` no `SERVICES` dá o campo do modal, que é o **override**, com o
botão **Gerar** que cria outra pelo mesmo método — os mesmos 16 bytes em
hexadecimal do `randKey()`, como o do SABnzbd. Valor igual ao da stack é o
mesmo que vazio: quer dizer "acompanhe a stack". Ela mora no `instance.extra`,
sem coluna nova.

Foi ele que trouxe o quarto formato do `patch.rs`, o **`yaml`**: o Bazarr
largou o INI na 1.4 e escreve `auth:` com a chave indentada embaixo. O
`merge_yaml()` não é parser — é o `merge_ini()` com outra pontuação: seção é
linha sem indentação terminada em `:`, filho é linha indentada, e a indentação
que vale é a que o app já usava. Só vale para dois níveis, que é toda a forma
do `auth.apikey`; valor que precisasse de aspas pede o formato crescer antes,
de propósito — aspas adivinhadas erradas é arquivo que o app recusa carregar.
O `patchText()` conhece os dois, porque o texto do `.zip` e as chaves que o
servidor escreve não podem discordar.

## READMEs

`README.pt-BR.md` é a fonte; o `README.md` (inglês, o padrão do repositório) e
o `README.es.md` são traduções.
Mudança de comportamento documentada precisa ir aos três. As capturas em
`docs/` (`screenshot.png`, `services.png`, `theme.png`, `credits.png`,
`config.png`) refletem a interface atual, e há uma seção **Docker** explicando como instalar
o que roda os arquivos gerados. O badge da licença é um SVG local por README
(`docs/badge-licen*.svg`, um por idioma, com o texto em `textLength` fixo) — nada de shields.io: o repositório
não busca imagem de fora.

Para refazer as capturas em `docs/`, use a skill `docs-screenshots`.

## Nome do projeto (v0.6)

`DEFAULTS.project` (padrão `hubstarr`) é o que separa esta stack das outras **na
máquina**. Antes dele o `container_name` era o `cname()`, fixo: subir numa
máquina que já tivesse um `sonarr` tomava o nome do que estava rodando, e o
`docker compose down` de uma pasta chamada `stack` levava junto o que outra
pasta de mesmo nome criou — o Compose tira o nome do projeto do nome da pasta.

São **três** nomes globais ao daemon, e só esses três o carregam:

- o **projeto**, escrito no topo do compose (`name: <project>`) em vez de
  deduzido da pasta — é o que impede um `down` de levar a stack do vizinho;
- o **`container_name`**, que vira `<project>-<cname>` pelo `dname()`;
- a **rede**, cujo `name:` vira `<project>-starrnet` pelo `netName()`.

O que **não** o carrega é tudo o que já é do escopo da stack: a chave do
serviço no compose, a rota do nginx, a pasta de configuração e a chave no
banco continuam sendo o `cname()`. Não é timidez — é o que deixa a mudança sem
migração nenhuma (as pastas no disco e as linhas no banco ficam com o nome que
sempre tiveram) e funciona porque **o compose dá a cada container um alias de
rede para o nome do *serviço* além do `container_name`**: dentro da rede,
`http://sonarr:8989` continua resolvendo, chame-se o container como se chamar
por fora. Foi medido — os dois aliases respondem. Por isso o upstream do nginx,
o `internalUrl()` do Prowlarr e o `base_url` do Configarr seguem no `cname()`.

Duas consequências a lembrar:

- **`dname()` é para o docker, `cname()` para todo o resto.** Quem precisa
  falar com o daemon por nome — hoje o `remove_container()` do Excluir — recebe
  o `dname()` no corpo da requisição; o `up_one`/`stop_one` e o `compose ps` do
  ponto de status usam nome de *serviço* e não mudaram.
- **Trocar o nome do projeto recria os containers** na subida seguinte, porque
  o projeto do compose passa a ser outro. A configuração dos apps não se move:
  ela está nos binds, e os binds não carregam o prefixo. O mesmo vale para quem
  vem de uma versão anterior: a coluna `project` nasce vazia, o `projSlug()`
  cai no `hubstarr`, e a primeira subida recria os containers com o novo nome.

O campo mora no **Ambiente** porque é da stack inteira, não de um serviço.
Vazio ele volta a `hubstarr`: `-sonarr` e `-starrnet` são nomes que o docker
recusa, e o campo não pode ser um jeito de escrever um compose que não sobe.

## Wishlist

O roadmap fica nos três READMEs, numa tabela por marco de versão; o texto
autoritativo é o do `README.pt-BR.md`, e mexer nele é mexer nos três. Hoje o
repositório é o **v0.6** — a página, o servidor de `backend/`, a Configuração
aplicada nos apps, o TRaSH Guides inteiro pelo Configarr e o nome do projeto
que separa esta stack das outras da máquina.

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
- ~~**v0.6**~~ — feito: o **nome do projeto**, no Ambiente, entra nos três nomes
  que o docker enxerga de fora — ver "Nome do projeto" acima.
- ~~**v0.7**~~ — feito: o **idioma da busca**, no Ambiente, desce para os
  metadados do Radarr, o Jellyfin, as legendas do Bazarr e os perfis do
  Configarr — ver "Idioma da busca" acima.

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

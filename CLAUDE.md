# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Projeto

Hubstarr é um protótipo de página única que gera `docker-compose.yml`, `.env` e
`nginx.conf` de uma stack de mídia (*arr + clientes de download + servidor de
mídia). **A página é um único arquivo**: `hubstarr.html` (~3200 linhas: CSS,
HTML e um `<script>` inline), e é ela que é o produto.

A página não tem build, teste, lint nem package manager: para rodar, abra o
arquivo no navegador. O `.mvn/` é resto de outro projeto e está no `.gitignore`.

Em `backend/` há um **servidor opcional** em Rust (v0.2): guarda as stacks em
SQLite, grava os arquivos e sobe a stack no Docker. Ele tem build e testes
(`cd backend && cargo test`), mas a página continua funcionando inteira sem
ele — ver "Servidor" mais abaixo.

Licença GPL-3.0 (`LICENSE`, texto oficial da FSF). O aviso de copyright fica no
comentário logo depois do `<!DOCTYPE html>` — não o remova ao mexer no arquivo,
é ele que liga o código à licença.

## Arquitetura do script

O script é uma sequência de seções marcadas por comentários `/* ---------- x ---------- */`:

1. **`I18N` / `LANGS`** — dicionário com uma chave por string visível, em
   pt-BR, en e es. Valor é string ou função quando depende de dados. Acesso por
   `t(chave, ...args)`. O HTML estático usa `data-i18n` (e `-html`, `-ph`,
   `-title`), aplicados por `applyI18n()`. As traduções cobrem também os
   comentários dos arquivos gerados. Adicionar idioma = copiar um bloco e traduzir.
2. **`SERVICES`** — catálogo dos serviços disponíveis. Cada entrada traz
   `id, name, port (interna), img, color` e flags que dirigem a geração:
   `media`/`mdir` (subpasta da biblioteca), `needsDl` (monta a árvore de
   downloads inteira), `dlClient`, `vpn`, `hw` (Jellyfin), `library` (Jellyfin:
   monta a base e mais as pastas que ficaram fora dela), `solver` (Prowlarr;
   ver abaixo), `internal` (gluetun e FlareSolverr: sem rota no nginx e sem
   botão de link), `publish` (Seerr: fora do nginx, publica a porta dele no
   host; a porta escolhida fica na instância, em `hostPort`, e o campo dela é
   do modal do serviço), `vpnCfg` (gluetun: as
   credenciais da VPN no modal dele),
   `webAuth` + `conf` (qBittorrent: usuário/senha/API key no modal dele e os
   dois arquivos dele — nenhum dos dois é montado, o servidor os escreve depois
   de subir; ver `patch` abaixo), `cdh` (SABnzbd: gerenciamento de
   downloads concluídos na Configuração), `noVol`, `derived` (Bazarr herda as
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
   `apps` (o que o Prowlarr configura), `clients[cliente] = {arrs, cats, cdh}`
   (com que categoria cada *arr usa o cliente e o gerenciamento de downloads
   concluídos; o `arrs` fica todo `true` — a interface não tem mais caixa de
   quem recebe, porque o cliente entra em todas as instâncias. O padrão da
   categoria sai do `catDefault()`: o `CATEGORY` por app e, para quem tem
   categorias próprias, o `CLIENT_CATEGORY` — o SABnzbd usa as dele, `tv`,
   `movies` e `music`) e `mm[família]` (Media Management mais a nomenclatura,
   cujos campos estão em `NAMING_FIELDS` — os formatos de episódio e de filme
  saem de fábrica com os do TRaSH Guides, variante do Jellyfin com id do TMDb).
   `syncConfig()` o alinha com o `added` a cada abertura do modal, e ele não
   entra nos arquivos gerados — é protótipo de interface.
5. **Derivações** — `slug()` → `cname()` (container_name = chave do serviço =
   pasta de config), `route()`, `url()`, `cfgPath`/`dataPath` (com variáveis
   `${...}` do `.env`) e `cfgReal`/`dataReal` (caminhos resolvidos, para o hint
   do modal). Alterar `cname` afeta compose, nginx e `.env` ao mesmo tempo.
   `dupPaths()` compara os caminhos já resolvidos e avisa, no rodapé da lista,
   quando duas instâncias caem na mesma pasta — Jellyfin e Bazarr ficam fora,
   um monta a biblioteca inteira e o outro segue as instâncias.
   Quem monta pasta de outro serviço passa por `derivedMounts()` (Bazarr) e
   `extraLibs()` (Jellyfin, que junta as pastas de fora das outras instâncias
   com as do "+ pasta") — os dois já devolvem o caminho certo de cada instância,
   literal ou com variável; não remonte `${BASE_MEDIA}/…` na mão.
6. **UI** — `renderCombo()`, `renderItems()`, modal de configuração
   (`openModal` + o handler de `#mSave`), modal de ambiente (`openEnv`), modal
   do nginx (`openNgx`), modal de configuração (`renderConfig`/`openCfg`, com
   backup para o Cancelar), modal da captura da paleta (`openShot`) e o dos
   créditos (`renderCredits`/`openCred`, grade montada do `SERVICES` pelo campo
   `site`), lista de pastas do Jellyfin (`renderLibs`/`libsOf`),
   `buildHelp()` e tema claro/escuro. O hint do modal (`updateHint`) é montado
   à mão com `innerHTML` a cada digitada — quem mexe nos campos chama
   `hintNow()`, e o `applyI18n()` o refaz para acompanhar a troca de idioma.
7. **Geradores** — `build()` (compose), `buildEnv()`, `buildNginx()`,
   `buildQbit()`, `buildQbitCats()` (o `categories.json`, único que sai em JSON:
   sem comentário e sem variável, com as chaves ordenadas para o arquivo não
   mudar de ordem a cada render). Os dois do qBittorrent têm a forma de dados ao
   lado da de painel — `qbitPairs()` e `qbitCats()` —, e é dela que sai tanto a
   aba quanto o que o servidor grava: um lugar só, para os dois nunca
   discordarem e `buildJellyfin()`. Os dois últimos saem só quando o serviço
   está na stack, e a aba aparece e some com ele. Serviço com arquivo próprio
   traz `conf:[{host, target, pane, tab, patch}, …]` no catálogo — é **lista**,
   porque o qBittorrent gera dois (a conf e o `categories.json`), e o
   `confsOf()` é quem a percorre no bind do compose, no `outFiles()` e nas abas.
   `host` é o caminho dentro da pasta de config dele, e vale para quem é
   montado. **`patch`** muda o destino da entrada: em vez de bind, ela vira
   chaves que o servidor escreve na configuração que o próprio app criou, depois
   do `up` — o valor da flag é a chave do `PATCH_DATA`, que diz o formato
   (`ini` ou `json`) e quem monta os dados. O `outPatches()` é o que a página
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
  `HTTP_PORT`/`HTTPS_PORT` no `.env`). O Seerr é o outro caso: em vez de rota,
  ganha `ports` no compose e a variável `<CNAME>_PORT` no `.env`, e o link dele
  aponta para a porta do host — em `http://`, porque o TLS mora no nginx e ele
  não passa por lá. Todo o resto só existe na rede `starrnet` e é alcançado por
  `container:porta-interna`; quem publica sai da contagem de rotas e do
  `buildNginx()` pelo `publishes()`. Quem roteia pela VPN usa
  `network_mode: service:gluetun` e responde no endereço do gluetun.
- **Volumes em sintaxe longa**, com `type: bind` e `bind.propagation: rslave`.
- **Cada subpath do nginx casa com a base URL do app**: nos *arr é a variável
  `<APP>__SERVER__URLBASE`; no Jellyfin é o `BaseUrl` do `network.xml`, que por
  isso é gerado. Serviço em subpath sem esse ajuste monta os links na raiz e
  quebra atrás do proxy — app sem base URL configurável não tem lugar num
  subpath, e é essa a razão de o Seerr publicar porta em vez de virar rota
  (houve um `subpathFix` reescrevendo redirects e HTML dele; foi removido).
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
`include_str!`. `cargo test` roda os testes do modelo e da gravação de arquivos;
`cargo run` serve tudo em `127.0.0.1:7878`. Opções: `--addr`, `--dir` (padrão
`./stack`, a pasta em que os arquivos são gravados), `--db` (padrão
`~/.hubstarr/hubstarr.db`), `--docker`.

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
recusando o que escapa da pasta), `deploy.rs` (`docker compose up -d`/`down` na
pasta da stack, mais o `docker_ok()` que o `api/health` devolve — ele pergunta
por `docker compose version`, não só pelo docker, porque o plugin é pacote à
parte e é ele que sobe a stack; sem ele a página abre o bloco "Precisa instalar
o Docker?" e mostra o aviso `#noDocker`), `apply.rs` (v0.3: a Configuração inteira
aplicada pela API — clientes de download em cada *arr **e no próprio Prowlarr**,
que também tem Settings → Download Clients (mesmo recurso, com `category` no
lugar do campo por família), cada *arr no Prowlarr, e o Media Management mais a
nomenclatura de cada família. Quem chama é o **Subir**, sozinho, depois de
gravar as chaves dos `patch` — e antes de qualquer chamada ele espera cada app
responder no `/ping`, porque recém-subido nenhum responde e a volta inteira
falharia por timeout. Os apps são alcançados
pelo nginx, porque o servidor roda no host e a rede `starrnet` não existe para
ele; aplicar de novo procura pelo nome e atualiza no lugar, e um app fora do ar
vira uma linha no log em vez de derrubar a volta inteira. Três coisas para não
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
`api/shot/:app/:theme` — o `ok_seg()` recusa segmento que escaparia da pasta ou
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
- **`cfg_mm` é por `service_id`**, não por instância: Media Management é por
  família, como na página.
- **`instance.extra`** guarda o que não virou coluna e volta espalhado no
  objeto. Uma flag nova no `SERVICES` não exige migração — só acrescente à
  `COLUMNS` o que precisar de coluna de verdade.

`load()` remonta `{added, defaults, config}` na forma exata que a página espera,
e devolve `None` quando o banco ainda não tem nada guardado — assim a página
fica com os próprios padrões em vez de recebê-los em branco de volta. Esse ida e
volta sem perda é o critério do modelo; ao mexer nele, é o que os testes cobrem.

Do lado da página, a seção `/* ---------- servidor (opcional) ---------- */`:
`detectServer()` só faz algo em `http(s)://` e chama `openStack()`, que carrega
o estado guardado — sem id nenhum, porque a stack é a do servidor.
`putInstance`/`delInstance` mexem numa linha por vez, e `saveSettings()`
(debounce no fim do `render()`) manda Ambiente, Configuração e a lista de chaves — é ela que acerta a ordem e apaga o
que saiu sem passar pelo modal. A flag `loading` existe para o estado que vem do
banco não ser gravado de volta enquanto está sendo aplicado.

## READMEs

`README.md` (pt-BR) é a fonte; `README.en.md` e `README.es.md` são traduções.
Mudança de comportamento documentada precisa ir aos três. As capturas em
`docs/` (`screenshot.png`, `services.png`, `theme.png`, `credits.png`)
refletem a interface atual, e há uma seção **Docker** explicando como instalar
o que roda os arquivos gerados. O badge da licença é um SVG local por README
(`docs/badge-licen*.svg`, um por idioma, com o texto em `textLength` fixo) — nada de shields.io: o repositório
não busca imagem de fora.

Para refazer as capturas, copie o HTML para um arquivo temporário fora do
projeto (o chromium do snap não lê `/tmp` nem `/srv`), injete no fim do
`<script>` o que a captura precisa — `setTheme('dark')` (as quatro capturas
estão no tema escuro), o `added` da stack de exemplo,
`$('#combo').classList.add('open')`, `openModal('sonarr',null)` mais
`openShot()` na da paleta, `openCred()` na dos créditos — e rode:

```sh
chromium-browser --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1480,760 --virtual-time-budget=4000 \
  --screenshot=$HOME/out.png "file://$HOME/tmp.html"
```

`services.png`, `theme.png` e `credits.png` são 1480×760 e a `screenshot.png`
acompanha a altura do conteúdo (hoje 1656, com a Wishlist aberta). A
`theme.png` é a única que precisa de rede: o modal da captura busca a imagem
em `docs.theme-park.dev`. O mesmo truque, com `--dump-dom` no lugar de `--screenshot`, é a
maneira de testar mudanças de comportamento sem navegador interativo. Se o
chromium travar sem escrever nada, passe um `--user-data-dir` próprio.

O favicon não aparece em captura nenhuma: o headless fotografa só o viewport,
sem a barra de abas.

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
autoritativo é o do `README.md`, e mexer nele é mexer nos três. Hoje o
repositório é o **v0.3** — a página, o servidor de `backend/` e a Configuração
aplicada nos apps.

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
- **v0.4** — custom formats e profiles por instância (4K, anime, …), ao lado do
  `NAMING_FIELDS` que já guarda a nomenclatura.
- **v0.5** — compatibilidade com o TRaSH Guides além da nomenclatura, que já
  saiu de fábrica: quality definitions, scores de custom format e o resto do
  guia. O JSON de origem dele (`docs/json/...` no repositório TRaSH-Guides) é
  a fonte, não a página renderizada.
- **v0.6** — busca localizada de mídia, com o idioma da busca escolhível.

Marco é ordem, não calendário: cada um depende do anterior. Ao propor mudança
que caia num deles, diga em qual — e não comece o de baixo antes do de cima.

## Commits

Mensagens em português, no imperativo/terceira pessoa do singular, uma linha
("Copia o link de cada serviço", "Serve a stack por HTTPS, com certificado
configurável"). Corpo só quando explica o porquê, não o quê. Um assunto por
commit, mesmo quando as mudanças estão no mesmo arquivo.

Pedido de commit já inclui o push: `git commit && git push origin master`, num
comando só. Se vier um "push" depois, ele já saiu — responda com o estado
(`git status -sb` e o último commit) em vez de repetir o comando.

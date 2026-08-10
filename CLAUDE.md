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
   botão de link), `noLink` (Heimdall: sem botão de link, mas com rota — ele é a
   raiz, o link seria o endereço da stack), `subpathFix` (Seerr: o `location`
   dele tira o prefixo e reescreve o que volta), `vpnCfg` (gluetun: as
   credenciais da VPN no modal dele),
   `webAuth` + `conf` (qBittorrent: usuário/senha/API key no modal dele e a
   `qBittorrent.conf` gerada e montada), `cdh` (SABnzbd: gerenciamento de
   downloads concluídos na Configuração), `noVol`, `derived` (Bazarr herda as
   subpastas das instâncias de Radarr/Sonarr presentes), `site` (endereço do
   projeto, que é o que põe o serviço na grade dos Créditos) e `credit` (nome
   a creditar quando ele difere do `name` — o FlareSolverr roda o Byparr).
   Adicionar um serviço normalmente é acrescentar uma linha aqui + o ícone em
   `ICONS` + as strings `d.<id>` no `I18N`.
3. **Constantes de convenção** — `STACK`/`NETWORK` (`starrnet`), `NGINX`
   (reverse proxy fixo, fora do combobox, único que publica portas),
   `ROOT_SERVICE` (Heimdall, servido em `/`; com `fixed:true`, o
   `ensureFixed()` o repõe no `added`, e ele fica fora do combobox e sem
   "Excluir"), `MULTI` (serviços com múltiplas instâncias), e os mapas de
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
   `buildQbit()` e `buildJellyfin()`. Os dois últimos saem só quando o serviço
   está na stack, e a aba aparece e some com ele. Serviço com arquivo próprio
   traz `conf:{host, target, pane, tab}` no catálogo — `host` é o caminho dentro
   da pasta de config dele, o mesmo no `.zip` e no bind do compose. Eles
   emitem **HTML com spans de realce** (`<span class="k">`/`v`/`c`); o texto
   puro para copiar/baixar vem de `textContent` dos panes (`plain()`,
   `plainEnv()`, `plainNginx()`). Ao editar um gerador, mantenha a marcação e
   passe strings pelo `t()`.
8. **ZIP** — `makeZip()` é uma implementação própria do formato (método
   "store", CRC32 manual), justamente para não depender de biblioteca externa.

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
logotipo certo. Hoje só o `radarr-uhd` e o `sonarr-uhd` existem; acrescentar uma
variante é pôr o data URI no `ICONS`, mais nada.

**Serviço que entra sozinho.** Um checkbox no modal pode arrastar outro serviço
para a stack no momento do save — `vpn` traz o `gluetun` (obrigatório, porque o
`network_mode` depende dele) e `solver` traz o `flaresolverr`. O padrão é o
mesmo: flag no `SERVICES`, campo no `cfg`, e um `if(cfg.X && !has('y'))` no
handler de `#mSave`.

## Invariantes a preservar

- **Zero dependências externas em runtime**: os logotipos são data URI, o ZIP é
  feito à mão, a lista de fusos vem do `Intl` do navegador. Não introduza CDN,
  npm nem `fetch` — aberta do disco, a página tem de funcionar inteira.
- **Nenhum serviço publica porta no host**, exceto o nginx. Ele ouve em 80/443
  dentro do container e publica no host as portas do modal próprio dele — o
  "Editar" da linha fixa (`DEFAULTS.http`/`https` → `HTTP_PORT`/`HTTPS_PORT` no
  `.env`). Todos os outros só existem na rede `starrnet` e são alcançados por
  `container:porta-interna`. Quem roteia pela VPN usa
  `network_mode: service:gluetun` e responde no endereço do gluetun.
- **Volumes em sintaxe longa**, com `type: bind` e `bind.propagation: rslave`.
- **Cada subpath do nginx casa com a base URL do app**: nos *arr é a variável
  `<APP>__SERVER__URLBASE`; no Jellyfin é o `BaseUrl` do `network.xml`, que por
  isso é gerado; no Seerr, que não tem base URL, é a flag `subpathFix` — o
  `location` tira o prefixo e reescreve redirects e HTML no caminho de volta.
  Serviço em subpath sem esse ajuste monta os links na raiz e quebra atrás do
  proxy.
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
  desenhados para isso e alguns são pretos (Heimdall, SABnzbd, Bazarr), então
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
  quando ela chega. A API key sai da `${STARR_APIKEY}` mapeada no alfabeto dele
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

**O contrato, que é o que não pode mudar: o servidor nunca gera conteúdo.** Ele
recebe pronto o que os geradores do `<script>` montaram (`outFiles()`), grava e
roda o `docker compose`. Os geradores existem num lugar só — se você for tentado
a montar YAML no Rust, é sinal de que a mudança pertence à página.

Módulos: `store/` (o modelo, com `migrate.rs` à parte), `files.rs` (grava o que veio, com `safe_join()`
recusando o que escapa da pasta), `deploy.rs` (`docker compose up -d`/`down` na
pasta da stack), `jobs.rs` (trabalhos numerados com log incremental, em memória
— subir a stack baixa imagem e não cabe numa resposta HTTP).

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

## Wishlist

O roadmap fica nos três READMEs, numa tabela por marco de versão; o texto
autoritativo é o do `README.md`, e mexer nele é mexer nos três. Hoje o
repositório é o **v0.2** — a página, mais o servidor de `backend/`.

- ~~**v0.2**~~ — feito: o backend liga o `hubstarr.html` ao Docker e guarda a
  stack. Uma primeira versão dele existiu e foi removida no `ba54e1a`; a de
  agora é normalizada.
- **v0.3** — aplicar a **Configuração** pela API de cada app. Hoje o `CONFIG` é
  protótipo de interface e não chega a arquivo nenhum; é ele que vira chamada.
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

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Projeto

Hubstarr é um protótipo de página única que gera `docker-compose.yml`, `.env` e
`nginx.conf` de uma stack de mídia (*arr + clientes de download + servidor de
mídia). **Todo o projeto é um único arquivo**: `arr-stack-prototype.html`
(~1700 linhas: CSS, HTML e um `<script>` inline).

Não há build, testes, lint, package manager nem backend. Para rodar, abra o
arquivo no navegador — nada de servidor. O `.mvn/` é resto de outro projeto e
está no `.gitignore`.

O botão "Criar stack" apenas simula o deploy; os arquivos gerados são reais.

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
   ver abaixo), `noVol`, `derived` (Bazarr herda as subpastas das instâncias de
   Radarr/Sonarr presentes).
   Adicionar um serviço normalmente é acrescentar uma linha aqui + o ícone em
   `ICONS` + as strings `d.<id>` no `I18N`.
3. **Constantes de convenção** — `STACK`/`NETWORK` (`starrnet`), `NGINX`
   (reverse proxy fixo, fora do combobox, único que publica portas),
   `ROOT_SERVICE` (Heimdall, servido em `/`), `MULTI` (serviços com múltiplas
   instâncias), e os mapas de variáveis de ambiente `INSTANCE_ENV`,
   `URLBASE_ENV`, `APIKEY_ENV`.
4. **Estado** — três globais mutáveis: `added` (instâncias,
   `{id,title,data,abs,vpn,hw,solver}` — `abs` só quando o caminho da mídia
   sai das bases, e aí é ele que vai literal para o compose), `picked` (id no combobox), `editing` (key
   em edição). `DEFAULTS` guarda o ambiente global (caminhos base, PUID/PGID,
   TZ, portas do host, TLS, VPN, API key). Nem tudo que está no `DEFAULTS` se
   edita no Ambiente: as portas do host saem no modal do nginx.
5. **Derivações** — `slug()` → `cname()` (container_name = chave do serviço =
   pasta de config), `route()`, `url()`, `cfgPath`/`dataPath` (com variáveis
   `${...}` do `.env`) e `cfgReal`/`dataReal` (caminhos resolvidos, para o hint
   do modal). Alterar `cname` afeta compose, nginx e `.env` ao mesmo tempo.
6. **UI** — `renderCombo()`, `renderItems()`, modal de configuração
   (`openModal` + o handler de `#mSave`), modal de ambiente (`openEnv`), modal
   do nginx (`openNgx`), `buildHelp()` e tema claro/escuro.
7. **Geradores** — `build()` (compose), `buildEnv()`, `buildNginx()`. Eles
   emitem **HTML com spans de realce** (`<span class="k">`/`v`/`c`); o texto
   puro para copiar/baixar vem de `textContent` dos panes (`plain()`,
   `plainEnv()`, `plainNginx()`). Ao editar um gerador, mantenha a marcação e
   passe strings pelo `t()`.
8. **ZIP** — `makeZip()` é uma implementação própria do formato (método
   "store", CRC32 manual), justamente para não depender de biblioteca externa.

## Dois padrões que se repetem

**Ajuda por campo.** Marcar uma `.row` de qualquer modal com
`data-help="<chave do I18N>"` basta: `buildHelp()` põe um `?` na terceira coluna
do grid e insere abaixo um parágrafo escondido com `data-i18n-html`, que o botão
liga e desliga e o `applyI18n()` retraduz. Não escreva o parágrafo à mão nem
deixe hint fixo onde cabe um `data-help`.

**Serviço que entra sozinho.** Um checkbox no modal pode arrastar outro serviço
para a stack no momento do save — `vpn` traz o `gluetun` (obrigatório, porque o
`network_mode` depende dele) e `solver` traz o `flaresolverr`. O padrão é o
mesmo: flag no `SERVICES`, campo no `cfg`, e um `if(cfg.X && !has('y'))` no
handler de `#mSave`.

## Invariantes a preservar

- **Zero dependências externas em runtime**: os logotipos são data URI, o ZIP é
  feito à mão, a lista de fusos vem do `Intl` do navegador. Não introduza CDN,
  fetch nem npm.
- **Nenhum serviço publica porta no host**, exceto o nginx. Ele ouve em 80/443
  dentro do container e publica no host as portas do modal próprio dele — o
  "Editar" da linha fixa (`DEFAULTS.http`/`https` → `HTTP_PORT`/`HTTPS_PORT` no
  `.env`). Todos os outros só existem na rede `starrnet` e são alcançados por
  `container:porta-interna`. Quem roteia pela VPN usa
  `network_mode: service:gluetun` e responde no endereço do gluetun.
- **Volumes em sintaxe longa**, com `type: bind` e `bind.propagation: rslave`.
- **Cada subpath do nginx casa com a base URL do app** (`<APP>__SERVER__URLBASE`).
- Toda string visível ao usuário passa pelo `I18N`, nos três idiomas.
- **`id` do serviço ≠ imagem do container**: o `flaresolverr` roda a imagem do
  Byparr (`ghcr.io/thephaseless/byparr`), substituto direto. O `id` é o que vira
  `container_name`, subpath e upstream — trocar de imagem não deve mexer nele.
  O logotipo também segue o nome, não a imagem: o do Byparr é um cookie que aos
  20px da lista vira um ponto laranja, e já foi tentado e revertido.
- **Logotipo sempre sobre fundo claro**: os SVGs do dashboardicons são
  desenhados para isso e alguns são pretos (Heimdall, SABnzbd, Bazarr), então
  `--ico-bg` é claro nos dois temas. Não o amarre ao `--panel`.
- **Favicon em três lugares, uma arte só**: o data URI no `<link rel="icon">`
  (o que faz o arquivo aberto do disco ter ícone), o `favicon.ico` da raiz (para
  quem serve a página) e o `docs/logo.svg` do título do README. Mudou a marca,
  mude os três — o `.ico` sai do SVG, rasterizado em 16…256. O `<link>` do
  `.ico` vem antes do SVG de propósito: o SVG tem precedência.

## READMEs

`README.md` (pt-BR) é a fonte; `README.en.md` e `README.es.md` são traduções.
Mudança de comportamento documentada precisa ir aos três. As capturas em
`docs/` (`screenshot.png`, `services.png`) refletem a interface atual, e há uma
seção **Docker** explicando como instalar o que roda os arquivos gerados.

Para refazer as capturas, copie o HTML para um arquivo temporário fora do
projeto (o chromium do snap não lê `/tmp` nem `/srv`), injete no fim do
`<script>` o que a captura precisa — `setTheme('dark')` (as duas capturas estão
no tema escuro), o `added` da stack de exemplo,
`$('#combo').classList.add('open')` — e rode:

```sh
chromium-browser --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1480,760 --virtual-time-budget=4000 \
  --screenshot=$HOME/out.png "file://$HOME/tmp.html"
```

`services.png` é 1480×760 e `screenshot.png` acompanha a altura do conteúdo
(hoje 1195). O mesmo truque, com `--dump-dom` no lugar de `--screenshot`, é a
maneira de testar mudanças de comportamento sem navegador interativo. Se o
chromium travar sem escrever nada, passe um `--user-data-dir` próprio.

O favicon não aparece em captura nenhuma: o headless fotografa só o viewport,
sem a barra de abas.

Ao injetar código, ancore no fim do `<script>` (`…render();\n</script>`): a
linha `applyI18n(); renderCombo(); renderItems(); render();` sozinha também
aparece dentro do `setLang()`, e substituí-la lá dentro leva a recursão infinita.

## Commits

Mensagens em português, no imperativo/terceira pessoa do singular, uma linha
("Copia o link de cada serviço", "Serve a stack por HTTPS, com certificado
configurável"). Corpo só quando explica o porquê, não o quê. Um assunto por
commit, mesmo quando as mudanças estão no mesmo arquivo.

Pedido de commit já inclui o push: `git commit && git push origin master`, num
comando só. Se vier um "push" depois, ele já saiu — responda com o estado
(`git status -sb` e o último commit) em vez de repetir o comando.

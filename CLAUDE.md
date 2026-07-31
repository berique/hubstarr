# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Projeto

Hubstarr é um protótipo de página única que gera `docker-compose.yml`, `.env` e
`nginx.conf` de uma stack de mídia (*arr + clientes de download + servidor de
mídia). **Todo o projeto é um único arquivo**: `arr-stack-prototype.html`
(~1550 linhas: CSS, HTML e um `<script>` inline).

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
   downloads inteira), `dlClient`, `vpn`, `hw` (Jellyfin), `noVol`, `derived`
   (Bazarr herda as subpastas das instâncias de Radarr/Sonarr presentes).
   Adicionar um serviço normalmente é acrescentar uma linha aqui + o ícone em
   `ICONS` + as strings `d.<id>` no `I18N`.
3. **Constantes de convenção** — `STACK`/`NETWORK` (`starrnet`), `NGINX`
   (reverse proxy fixo, fora do combobox, único que publica portas),
   `ROOT_SERVICE` (Heimdall, servido em `/`), `MULTI` (serviços com múltiplas
   instâncias), e os mapas de variáveis de ambiente `INSTANCE_ENV`,
   `URLBASE_ENV`, `APIKEY_ENV`.
4. **Estado** — três globais mutáveis: `added` (instâncias, `{id,title,data,vpn}`),
   `picked` (id no combobox), `editing` (key em edição). `DEFAULTS` guarda o
   ambiente global (caminhos base, PUID/PGID, TZ, TLS, VPN, API key).
5. **Derivações** — `slug()` → `cname()` (container_name = chave do serviço =
   pasta de config), `route()`, `url()`, `cfgPath`/`dataPath` (com variáveis
   `${...}` do `.env`) e `cfgReal`/`dataReal` (caminhos resolvidos, para o hint
   do modal). Alterar `cname` afeta compose, nginx e `.env` ao mesmo tempo.
6. **UI** — `renderCombo()`, `renderItems()`, modal de configuração
   (`openModal`/`saveModal`), modal de ambiente (`openEnv`), tema claro/escuro.
7. **Geradores** — `build()` (compose), `buildEnv()`, `buildNginx()`. Eles
   emitem **HTML com spans de realce** (`<span class="k">`/`v`/`c`); o texto
   puro para copiar/baixar vem de `textContent` dos panes (`plain()`,
   `plainEnv()`, `plainNginx()`). Ao editar um gerador, mantenha a marcação e
   passe strings pelo `t()`.
8. **ZIP** — `makeZip()` é uma implementação própria do formato (método
   "store", CRC32 manual), justamente para não depender de biblioteca externa.

## Invariantes a preservar

- **Zero dependências externas em runtime**: os logotipos são data URI, o ZIP é
  feito à mão, a lista de fusos vem do `Intl` do navegador. Não introduza CDN,
  fetch nem npm.
- **Nenhum serviço publica porta no host**, exceto o nginx. Ele ouve em 80/443
  dentro do container e publica no host as portas do Ambiente
  (`DEFAULTS.http`/`https` → `HTTP_PORT`/`HTTPS_PORT` no `.env`). Todos os
  outros só existem na rede `starrnet` e são alcançados por
  `container:porta-interna`. Quem roteia pela VPN usa
  `network_mode: service:gluetun` e responde no endereço do gluetun.
- **Volumes em sintaxe longa**, com `type: bind` e `bind.propagation: rslave`.
- **Cada subpath do nginx casa com a base URL do app** (`<APP>__SERVER__URLBASE`).
- Toda string visível ao usuário passa pelo `I18N`, nos três idiomas.

## READMEs

`README.md` (pt-BR) é a fonte; `README.en.md` e `README.es.md` são traduções.
Mudança de comportamento documentada precisa ir aos três. As capturas em
`docs/` (`screenshot.png`, `services.png`) refletem a interface atual.

## Commits

Mensagens em português, no imperativo/terceira pessoa do singular, uma linha
("Copia o link de cada serviço", "Serve a stack por HTTPS, com certificado
configurável").

---
name: docs-screenshots
description: Regenerate the hubstarr.html screenshots in docs/ (screenshot.png, services.png, theme.png, credits.png, config.png, browse.png) via headless chromium. Use when the interface changed and the docs/ captures need to be refreshed.
---

Para refazer as capturas, copie o HTML para um arquivo temporário fora do
projeto (o chromium do snap não lê `/tmp` nem `/srv` — e também não **escreve**
neles: o `--screenshot` apontado para `docs/` falha calado, o arquivo antigo
continua onde estava e a rodada inteira parece ter dado certo. Fotografe para o
`$HOME` e copie depois), injete no fim do
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

`services.png`, `theme.png` e `credits.png` são 1480×760, a `config.png` e a
`browse.png` são 1480×900 — os modais são densos e em 760 não caberia o que
mostram — e a `screenshot.png` acompanha a altura do conteúdo (hoje 1888, com a
Wishlist aberta).

A `browse.png` é o navegador de pastas por cima do Ambiente e pede duas coisas
próprias: um `document.body.classList.add('srvOn')`, que é o que revela o `📁`,
e uma **listagem de exemplo** posta à mão (`fbAt`, `fbEntries` e
`renderBrowse()`, sem passar pelo `openBrowse()`, que sairia buscando o
`api/browse`). O Ambiente fica no topo, como ele abre: mirar numa linha
específica com `scrollTop` não vale a briga, porque no passo do `--screenshot`
o modal rola bem menos do que no `--dump-dom` e a tira visível muda entre os
dois. A
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

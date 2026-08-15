# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — gerador de *arr stack

*🇧🇷 Português (Brasil) · [🇬🇧 English](README.en.md) · [🇪🇸 Español](README.es.md)*

[<img src="docs/badge-licenca.svg" alt="Licença: GPL-3.0" height="20">](LICENSE)

Protótipo que monta e sobe uma stack de mídia (*arr + clientes de download +
servidor de mídia): o `docker-compose.yml`, o `.env` e o `nginx.conf`, e depois
os apps configurados uns nos outros. O [servidor](#servidor) é o Hubstarr —
ele guarda a stack em SQLite, grava os arquivos, sobe no Docker e configura os
apps pela API de cada um.

A interface é uma **página única sem dependência externa**, que o servidor traz
embutida. Ela também abre sozinha, direto do disco, e ali gera os arquivos num
`.zip` — mas só isso: sem servidor não há stack no ar, nem senha do
qBittorrent, nem base URL do Jellyfin, nem perfil do TRaSH Guides. É o modo de
quem quer só os arquivos.

> [!WARNING]
> **Protótipo.** O Hubstarr não foi projetado para uso em produção: os arquivos
> que ele gera são um ponto de partida, sem endurecimento de segurança, backup
> ou monitoramento. Revise tudo — senhas, portas, certificados e permissões —
> antes de expor a stack a qualquer rede que não seja a sua.

## O que ele deixa pronto

Gerar os arquivos é a metade fácil. A outra é a que se faz à mão depois, app por
app, e é ela que o **Subir** faz sozinho pela API de cada um — o botão
**Aplicar na stack** reaplica tudo isto sem subir nada:

- **A configuração básica de cada app**: a base URL igual ao subpath do nginx, a
  mesma API key na stack inteira, fuso, PUID/PGID e as pastas do compose. Nos
  *arr, o *Media Management* completo — hardlink, renomear, permissões, lixeira,
  espaço livre — e a nomenclatura de episódio, filme e faixa já nos formatos do
  [TRaSH Guides](https://trash-guides.info).
- **Clientes de download ligados**: qBittorrent e SABnzbd registrados em cada
  Sonarr, Radarr e Lidarr **e no próprio Prowlarr**, cada um com a categoria que
  você escolheu — e as categorias criadas dentro do cliente, cada uma com a
  pasta dela. O Prowlarr recebe ainda cada *arr para sincronizar, com as
  categorias por família, e o FlareSolverr como proxy de indexador.
- **Pontos de importação prontos**: a pasta raiz de cada *arr, no caminho que o
  container enxerga (`/data/tv`, `/data/movies`, `/data/music`). Sem ela a
  primeira série para num *You must add a root folder* — e o caminho digitado à
  mão costuma ser o do host, que o app aceita e depois não acha.
- **Perfis de qualidade do TRaSH Guides**, por instância: cada preset traz o
  trio que o guia recomenda junto — o perfil, os custom formats **com os scores
  dele** e a definição de tamanho dos arquivos. É assim que a instância de 4K
  deixa de ser igual à de 1080p. Quem aplica é o
  [Configarr](https://configarr.de), com os templates do Recyclarr, e o guia
  continua sendo dele: o Hubstarr escolhe, não reimplementa.
- **Jellyfin pré-configurado**: o assistente inicial (idioma da interface,
  administrador, acesso remoto) e uma biblioteca por instância de *arr, com o
  tipo certo e o caminho **de dentro do container** — o mesmo que o *arr recebe
  como pasta raiz, que é o que faz a biblioteca listar justamente o que ele
  importou.

Sem servidor nada disso acontece: o `.zip` leva os arquivos, e o resto é você
quem faz nas interfaces. É a diferença entre uma stack no ar e uma stack pronta
para usar.

Rode o servidor e abra o endereço dele:

```sh
cd backend
cargo run --release      # http://127.0.0.1:7878
```

O **Ambiente** abre junto: é dali que saem as bases de caminho que todo o resto
usa. Fechou, ele continua no botão do topo. Só os arquivos, sem subir nada?
Abra o `hubstarr.html` direto no navegador — ele é autocontido, com os
logotipos embutidos como data URI.

![A interface: lista de serviços à esquerda, arquivos gerados à direita](docs/screenshot.png)

O combobox lista os serviços disponíveis com seus logotipos e portas padrão:

![O combobox aberto, mostrando os onze serviços disponíveis](docs/services.png)

E o campo **Tema** mostra a captura da paleta escolhida sem sair da página:

![O modal com a captura do tema hotline do Sonarr, por cima do modal do serviço](docs/theme.png)

## O que dá para fazer

- **Escolher serviços** num combobox com logotipos e adicioná-los à stack.
- **Créditos**, no botão ao lado do título: um modal com todos os projetos que
  a stack usa — cada app com link para o site dele —, mais a LinuxServer.io das
  imagens, o theme.park dos temas e a origem dos ícones.

  ![O modal de Créditos, com os projetos da stack e a origem de imagens, temas e ícones](docs/credits.png)
- **Configurar cada instância** num modal: título, subpasta de mídia/downloads
  e roteamento pela VPN.
- **Copiar o link** de cada serviço, já com o esquema, o endereço e o subpath
  pelos quais o nginx vai atendê-lo. O endereço é o domínio do Ambiente quando
  há um; sem ele, é o mesmo por onde você abriu a página — quem chega pelo IP da
  LAN recebe os links nesse IP, e não em `localhost`.
- **Ordenar a lista arrastando**: pegue a linha do serviço em qualquer ponto e
  mova-a; a ordem que você deixar é a ordem em que os serviços saem no
  `docker-compose.yml` e no `.env` — com servidor, ela fica guardada. Começar o
  gesto no Link, no Editar, no Excluir ou no ponto de status continua clicando
  neles. As setas ↑ ↓ fazem o mesmo com a alça (`⁙`) em foco, para quem não usa
  o mouse. O nginx é linha fixa e não se move. Ordem não é ordem de subida:
  quem manda nisso no compose é o `depends_on`.
- **Aviso de conflito**: duas instâncias apontadas para a mesma pasta se
  atropelam na importação, então a lista avisa em vermelho, com os nomes e o
  caminho. O Jellyfin, que monta a biblioteca inteira, e o Bazarr, que segue as
  outras, ficam de fora da checagem.
- **Etiquetas na linha do serviço**, uma cor por tipo: a variante do logotipo,
  o tema da interface, a saída pela VPN, a aceleração por GPU, o endereço na
  stack e as pastas de configuração e de mídia, por extenso. Abaixo da lista,
  ao lado do **Limpar tudo**, uma legenda diz o que cada cor marca — ela some
  com a lista vazia.
- **Múltiplas instâncias** de Sonarr, Radarr, Lidarr, Bazarr e Prowlarr —
  basta o título ser diferente. Sonarr e Radarr recebem também
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Base URL automática**: Sonarr, Radarr, Lidarr e Prowlarr recebem
  `<APP>__SERVER__URLBASE=/<container_name>`, já casando com o subpath do
  nginx. O Bazarr não expõe essa variável — a base fica na interface dele.
- **API key** no Ambiente: uma só para toda a stack. Sonarr, Radarr, Lidarr e
  Prowlarr saem no compose com `<APP>__AUTH__APIKEY=${STARR_APIKEY}`, e o
  SABnzbd com `SAB_API_KEY=${STARR_APIKEY}`; o valor fica no `.env`. A chave já
  nasce sorteada — 16 bytes em hexadecimal, o mesmo que `openssl rand -hex 16`
  — e o botão "Gerar" sorteia outra.
- **Aceleração de hardware do Jellyfin**: CPU, Intel ou NVIDIA. Intel ganha
  `devices: /dev/dri:/dev/dri`; NVIDIA, a reserva de GPU em `deploy` e as
  variáveis `NVIDIA_VISIBLE_DEVICES` / `NVIDIA_DRIVER_CAPABILITIES`.
- **Tema do theme.park**: os serviços de imagem do linuxserver — Sonarr,
  Radarr, Lidarr, Prowlarr, Bazarr, qBittorrent, SABnzbd e Jellyfin —
  saem com `DOCKER_MODS=ghcr.io/themepark-dev/theme.park:<app>`, o mod que
  aplica o tema na interface deles. No Sonarr e no Radarr o modal ainda traz um
  campo **Variante**, que vira `TP_ADDON`: *Padrão* usa o addon escuro
  (`sonarr-darker`), *4K* troca logotipo e favicon pelos do addon de 4K
  (`sonarr-4k-logo|sonarr-4k-favicon`) e *Animes* troca os dois pelos de anime
  (`sonarr-anime-logo|sonarr-anime-favicon`) — útil para distinguir as
  instâncias de uma stack com mais de um Sonarr ou Radarr. Os dois têm também
  um campo **Tema**, a paleta em `TP_THEME`: `aquamarine`, `hotline`, `hotpink`,
  `dracula`, `dark`, `organizr` (o padrão), `space-gray`, `overseerr` e `nord`.
  Abaixo do campo, um link mostra a captura da paleta escolhida num modal
  sobre o do serviço; a imagem vem da documentação do
  [theme.park](https://docs.theme-park.dev/), uma por app. Na lista, a linha
  de quem tem tema traz uma etiqueta com o escolhido, ao lado da variante.
- **FlareSolverr junto do Prowlarr**: no modal do Prowlarr, um checkbox
  marcado por padrão traz o FlareSolverr para a stack — é ele que resolve o
  desafio anti-bot da Cloudflare nos indexadores protegidos. Configure-o no
  Prowlarr em *Settings → Indexers → FlareSolverr*, com a URL
  `http://flaresolverr:8191`. A imagem por trás dele é a do
  [Byparr](https://github.com/ThePhaseless/Byparr), substituto direto e mais
  atual, com a mesma API e a mesma porta.
- **Ajuda por campo** no Ambiente e na Configuração: cada linha tem um `?` que
  abre uma explicação do que aquele valor faz — e, no Ambiente, de como ele sai
  nos arquivos gerados.
- **network.xml do Jellyfin**: com ele na stack, o `BaseUrl` no subpath do
  nginx e o `nginx` em `KnownProxies` — sem o primeiro a interface monta os
  links na raiz e o subpath responde 404, sem o segundo ele registra o IP do
  proxy no lugar do IP de quem pediu. O arquivo **não é montado**: ele é do
  Jellyfin, que migra a configuração de rede ao subir, e montá-lo congelaria o
  que ele guarda ali. Com servidor, o **Subir** espera o app criar o arquivo,
  confere se a base URL está lá, escreve a que falta e reinicia só ele — as
  demais chaves ficam como estavam. Sem servidor, ele sai no `.zip`, no caminho
  em que o app o lê (`/config/network.xml`, ao lado do `system.xml`).
- **qBittorrent.conf pronta**: quando ele está na stack, o Hubstarr monta a
  configuração inicial dele — pastas iguais às do compose, ajustes de proxy
  reverso e as credenciais no formato do próprio qBittorrent 5.2: a senha em
  PBKDF2-SHA512 e a API key `qbt_` + 28 caracteres, derivada da
  `${STARR_APIKEY}` da stack — a conf é lida por ele, não pelo compose, então a
  variável não seria expandida ali. Usuário, senha e chave se editam no modal
  dele — e é a **API key** que os *arr usam para falar com ele, não a senha:
  ela não expira quando a senha da interface muda. O arquivo **não é montado**: quem manda nele é o próprio qBittorrent, e
  montá-lo congelaria tudo o que ele guarda ali. Com servidor, o **Subir**
  escreve essas chaves na conf que o app criou — parando o container, fazendo a
  troca e subindo de novo, porque ele reescreve o arquivo inteiro ao sair. Sem
  servidor, ele sai no `.zip`, no caminho em que o app o lê, para copiar de lá.
- **Preferências do qBittorrent pela API**: com os apps no ar, o **Subir** (e o
  **Aplicar na stack**) ainda ajusta nele o **gerenciamento automático de
  torrent** — ligado, e seguindo a categoria quando ela muda, que é o que faz o
  torrent ir para a pasta certa —, a **pasta de download** (a subpasta do modal
  dele, no caminho que o container enxerga) e o **usuário e a senha** da
  interface. Não é repetição da conf: aquela é o que ele lê ao nascer, esta é a
  mesma decisão aplicada a um qBittorrent que já existe — e o gerenciamento
  automático a conf nem cobre. A **API key** dele é respeitada: se o
  app já tem uma, o Hubstarr **não a troca** — é com ela que os clientes dele já
  falam —, e são os *arr que passam a ser registrados com a chave do app. A
  nossa só entra quando ele ainda não tem nenhuma, que é a primeira subida.
  Quem a grava é a conf: o qBittorrent aceita a propriedade na API e a ignora.
- **sabnzbd.ini do SABnzbd**: a **API key** dele é a mesma da stack — o campo no
  modal mostra a que vale, e o **Gerar** cria outra pelo mesmo método (16 bytes
  em hexadecimal), que então vai para o `.env` —, e as pastas de **download em
  progresso** e **download completo** viram o `download_dir` e o `complete_dir`.
  Vai junto o `url_base` com o subpath em que o nginx o serve — sem ele o
  SABnzbd monta os links na raiz e quebra atrás do proxy. As quatro chaves são
  escritas no `sabnzbd.ini` que o próprio app criou, depois de a stack subir,
  como no qBittorrent.
- **Categorias do qBittorrent**: as que a **Configuração** deu a cada *arr,
  cada uma com a subpasta dela dentro do caminho de download — mesma partição,
  então o *arr continua fazendo hardlink em vez de copiar. Com servidor, elas
  são criadas **pela API do app**, com ele no ar: quem já existe tem a pasta
  atualizada, e nenhuma é removida — pode haver torrent apontado para ela. Sem
  servidor, as mesmas categorias saem no `.zip` como `categories.json`, para
  copiar para a pasta de configuração dele antes da primeira subida.
- **HTTPS opcional**, com o certificado e a chave vindos do host.
- **Configuração** (botão no topo): escolher quais instâncias o Prowlarr vai
  configurar, com que categoria cada *arr usa cada cliente de download —
  `tv-sonarr`, `radarr`, `lidarr` no qBittorrent, e as categorias de fábrica do
  SABnzbd (`tv`, `movies`, `music`), todas editáveis —,
  mais o gerenciamento de downloads concluídos no SABnzbd, e as opções de
  *Media Management* — hardlink, renomear, permissões,
  pastas vazias, o bloco **avançado** (reexaminar a pasta, data do arquivo,
  lixeira e limpeza dela, importar arquivos extras, checagem de espaço livre) e a nomenclatura
  completa de cada app (*Episode Naming*,
  *Nomenclatura de filme*, *Nomeação da faixa*: caracteres ilegais,
  dois-pontos, vários episódios e todos os formatos de arquivo e de pasta) —,
  separadas por família: Sonarr, Radarr e Lidarr. Os formatos de episódio e de
  filme já vêm com os do [TRaSH Guides](https://trash-guides.info), na variante
  do Jellyfin com o id do TMDb. As permissões abrem os campos
  de `chmod` e `chown`, e no Lidarr a caixa de nome existente é quem traz os
  formatos de faixa e a pasta do álbum.

  ![O modal da Configuração, na nomenclatura de episódio do Sonarr](docs/config.png)

  Com mais de um Sonarr na stack, cada formato dele — os três de episódio
  (**padrão**, **diário** e **anime**) e as três pastas (**série**,
  **temporada** e **especiais**) — traz a lista das instâncias que o recebem:
  dá para mandar o formato de anime só para o *Sonarr [Anime]*, por exemplo. De fábrica todas as instâncias recebem todos os
  formatos; pelo menos uma é obrigatória no formato padrão, porque o campo é
  obrigatório no app, e a que você desmarcar mantém o formato que já tem, em vez
  de perdê-lo. As três partes da Configuração chegam aos apps pelo **Subir** e
  pelo **Aplicar na stack**.
- **Pastas raiz prontas**: cada Sonarr, Radarr e Lidarr recebe a pasta que o
  compose monta nele — `/data/tv`, `/data/movies`, `/data/music` —, no caminho
  que o container enxerga. Sem isso a primeira série ou filme para num *You must
  add a root folder*, e o caminho digitado à mão costuma ser o do host, que o
  app aceita e depois não acha. O que já estiver lá fica: pasta raiz se remove
  com a biblioteca junto.
- **Jellyfin pronto para usar**: com ele na stack, o **Subir** passa pelo
  assistente inicial — a interface no idioma da página, o administrador do modal
  dele e o acesso remoto — e cria uma biblioteca por instância de Sonarr, Radarr
  e Lidarr, com o tipo certo (séries, filmes, música) e o caminho **de dentro do
  container**, o mesmo que o *arr recebe como pasta raiz: é essa igualdade que
  faz a biblioteca listar justamente o que o *arr importou. As pastas avulsas do
  modal dele entram como bibliotecas mistas. Usuário e senha em branco significam
  "não mexa no assistente": as bibliotecas entram do mesmo jeito, mas ele fica
  aberto para você terminar no navegador — concluí-lo sem administrador deixaria
  o Jellyfin sem conta nenhuma em que entrar. Num Jellyfin cujo assistente já foi
  concluído, é o usuário e a senha do modal que dão ao Hubstarr o token para
  criar as bibliotecas. Biblioteca que já existe não é tocada, e nenhuma é
  removida.
- **Perfis de qualidade e custom formats** do [TRaSH Guides](https://trash-guides.info),
  por instância: cada Sonarr e cada Radarr da stack escolhe os perfis do guia
  que quer — **HD (1080p)**, **4K (2160p)**, **Remux 4K**, **Anime** —, e é assim
  que a instância de 4K deixa de ser igual à de 1080p. Cada um traz o trio que o
  guia recomenda junto: o perfil, os custom formats que o pontuam e a definição
  de tamanho dos arquivos. Há ainda um campo para escrever outros templates do
  Recyclarr à mão.

  Quem aplica é o [Configarr](https://configarr.de), e ele **não é um serviço
  da stack**: a página gera o `config.yml` e o `secrets.yml` dele, e o servidor
  o roda com um `docker run --rm` avulso depois que os apps respondem, no
  **Subir** e no **Aplicar na stack**. Ele roda uma vez e sai — num `up -d`
  subiria antes de os apps estarem de pé. Entra na rede da stack para alcançar
  cada *arr pelo nome do container, com o PUID/PGID do Ambiente, e guarda o
  cache do TRaSH e do Recyclarr em `<config>/configarr/repos`. A seção existe
  mesmo sem ele em lugar nenhum: o que ela descreve é a stack, não um container.
  O Lidarr fica de fora: não há template para ele.
- **Ambiente global** (botão no topo): bases de caminho, PUID/PGID — que, com
  servidor, já vêm com o usuário e o grupo em que ele roda, o mesmo que cria as
  pastas da stack —, time zone,
  restart policy, API key e TLS. A
  lista de fusos é a IANA inteira, vinda do próprio navegador, e o valor
  inicial é o fuso da máquina.
- **Baixar** `docker-compose.yml`, `.env` e `nginx.conf` juntos
  num `.zip` — o botão fica na barra enquanto não há servidor; com ele, quem
  grava os arquivos é o **Subir**.
- **Passo a passo na primeira visita**: uma volta pela interface acendendo cada
  área e dizendo o que ela faz, com **Pular** a qualquer momento. Concluída ou
  pulada, não aparece de novo — a marca fica no navegador.
- **Trocar o idioma** no seletor do topo: português (Brasil), inglês e
  espanhol.

## Docker

O resumo desta seção também está na própria página, num aviso colapsável acima
dos painéis.

O Hubstarr em si só precisa de um navegador; os arquivos que ele gera é que
precisam do Docker com o plugin Compose. No Linux, o script oficial resolve:

```sh
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

No macOS e no Windows — ou no Linux, se preferir uma instalação gerenciada com
interface gráfica — instale o [Docker Desktop][dd], que já vem com o Compose.

Para usar o `docker` sem `sudo`, ponha o usuário no grupo — a mudança vale na
sessão seguinte:

```sh
sudo usermod -aG docker $USER
```

[dd]: https://docs.docker.com/desktop/

Com o Docker no lugar, descompacte o `.zip` e suba a stack de dentro da pasta
dos arquivos:

```sh
docker compose up -d
```

## Servidor

O servidor em `backend/` é o que faz a stack existir: ele **guarda as stacks
entre sessões**, grava os arquivos em disco, sobe tudo no Docker e configura os
apps uns nos outros — os clientes de download em cada *arr, os *arr no
Prowlarr, o Media Management, a nomenclatura e os perfis do TRaSH Guides. Com
ele no ar, o botão do `.zip` sai da barra, porque o **Subir** grava os mesmos
arquivos.

O que ele **não** faz é gerar conteúdo: recebe pronto o que os geradores da
página montaram. É essa divisão que mantém os geradores num lugar só, e é ela
que deixa a página abrir sozinha do disco quando alguém quer só os arquivos.

Compilar e rodar precisa do [Rust](https://rustup.rs):

```sh
cd backend
cargo run --release
```

Abra `http://127.0.0.1:7878`. A página é a mesma — servida pelo binário, que a
traz embutida —, com duas coisas a mais: o selo **servidor** no topo e, nos
arquivos gerados, os botões **Subir** e **Derrubar**. Ao abrir, ela pergunta ao
servidor se há `docker compose` — ou `podman compose`, que ele procura sozinho
quando o docker não responde — ali; se não houver, avisa e já abre o bloco
**Precisa instalar o Docker?**. Sem `docker compose` ali, **Subir** e
**Derrubar** ficam desabilitados, com a explicação na dica dos botões.

| Opção      | Padrão                   | O que é                                        |
| ---------- | ------------------------ | ---------------------------------------------- |
| `--addr`   | `127.0.0.1:7878`         | endereço em que o servidor atende               |
| `--dir`    | `./stack`                | pasta em que os arquivos gerados são gravados   |
| `--db`     | `~/.hubstarr/hubstarr.db`| banco em que a stack é guardada                 |
| `--docker` | `docker`, ou `podman`    | comando do compose; sem a opção, vale o primeiro dos dois que responder |
| `-v`       | desligado                | diz o passo a passo: arquivos, banco e chamadas de API |

O servidor escreve o que faz na saída e num `servidor.log`, ao lado do banco
(`~/.hubstarr/servidor.log`, com o `--db` de fábrica): a subida, o motor de
container escolhido e cada gravação de estado vinda da página — com quantos
serviços vieram e quais saíram da stack. O arquivo acrescenta, nunca reescreve,
e é onde se olha quando a stack mudou e não se sabe por quê.

Com **`-v`** ele conta o passo a passo, nos dois lugares: cada arquivo gravado
(inclusive as chaves escritas na configuração de cada app), cada linha mexida no
banco — instância, Ambiente, Configuração, a lista de serviços — e **cada
chamada às APIs dos apps**, com método, caminho e status. É o modo de descobrir
por que uma ligação não passou; ligado sempre, ele afogaria as linhas que
importam, porque uma volta do *Aplicar* são dezenas de chamadas. Senha e chave
de API nunca saem no log: do Ambiente vão só os nomes dos campos, e das URLs
sai a parte antes do `?`.

A stack fica no banco com as instâncias, o Ambiente e a Configuração em
tabelas próprias — o estado da página, normalizado, e não um blob de JSON. Ela
é uma só, a da pasta do `--dir`: para manter outra, aponte o `--dir` e o
`--db` para outro lugar.

Um banco de uma versão anterior, que guardava várias stacks, é convertido na
primeira abertura; a stack mais antiga é a que fica, e o servidor diz na saída
em que pasta cada uma das outras gravava, para você não perder os arquivos
delas de vista.

Com servidor, as capturas das paletas do campo **Tema** também passam por ele:
a primeira visita a cada uma sai para a documentação do theme.park e as
seguintes saem do disco, de uma pasta `shots/` ao lado do banco. O repositório
não redistribui captura de ninguém — o cache guarda o que você mesmo abriu, e
se apaga sozinho quando passa de 64 MB, das mais antigas para as mais novas.
Apagar a pasta na mão não quebra nada: custa uma busca a mais. Aberta do disco,
sem servidor, a página continua buscando cada captura direto na documentação
deles.

Antes de subir, o servidor **cria as pastas** que os volumes do compose
esperam — as de configuração, as de mídia e as de download. Sem isso quem as
cria é o Docker, como `root`, e o app não consegue escrever nelas. Caminho que
já existe e não é pasta faz o Subir parar ali, com o nome dele no log.

O **Subir** já deixa a stack configurada: assim que os apps respondem, o
servidor registra cada cliente de download em cada *arr **e no próprio
Prowlarr** — que tem o Settings → Download Clients dele —, cada *arr marcado em
Settings → Apps do Prowlarr, e o *Media Management* com a nomenclatura de cada
família, tudo pela API deles, mostrando o que passou. O botão **Aplicar na
stack**, no modal da Configuração, faz o mesmo sem subir nada — é o caminho
para reaplicar depois de mexer nas escolhas.

Com o **FlareSolverr** na stack, o Prowlarr também recebe o proxy dele em
*Settings → Indexers → Indexer Proxies*, com a etiqueta **flaresolverr**. Falta
só marcar essa etiqueta nos indexadores que precisam dele — é assim que o
Prowlarr decide quando usar o resolvedor.

No Prowlarr, o Settings → Download Clients ganha **um registro por cliente**,
todos na categoria `prowlarr`: o que ele pega é avulso, não veio de um *arr,
então fica junto e separado do que cada instância baixa. E as categorias passam
a existir dentro do cliente — a de cada *arr e a do Prowlarr —, pela API de cada
app: `torrents/createCategory` no qBittorrent (a que já existe tem a pasta
atualizada em vez de falhar) e `set_config&section=categories` no SABnzbd, cada
uma com a pasta de mesmo nome dentro do diretório de downloads concluídos. Os apps são alcançados pelo nginx, na porta que ele
publica no host. Aplicar de novo não duplica — o cliente é procurado pelo nome
e atualizado no lugar —, e um app que ainda não subiu vira uma linha no log em
vez de interromper o resto. Chamada que **não chega** ao app é repetida dez
vezes, com cinco segundos entre elas: vale para falha de acesso — ninguém
escutando, ou o 502 do nginx enquanto o container ainda sobe —, e não para o app
recusando o pedido, que daria a mesma resposta dez vezes. Com `-v`, cada
tentativa aparece no log. O SABnzbd precisa da chave de API dele, que é o
próprio app que gera na primeira subida: copie de *Config → General* e cole no
campo **API key** do modal dele.

> [!WARNING]
> O servidor roda `docker compose` e escreve em disco: não o exponha a uma
> rede em que você não confie. O padrão é atender só em `127.0.0.1`.

## Convenções geradas

O nome da stack e o da rede são fixos: `starrnet`. O título de cada instância
vira um slug (minúsculas, sem acentos, espaços como hífen) usado como
`container_name`, chave do serviço e pasta de config:

| Título          | container_name | config                       |
| --------------- | -------------- | ---------------------------- |
| `Radarr`        | `radarr`       | `${BASE_CONFIG}/radarr`      |
| `Radarr [UHD]`  | `radarr-uhd`   | `${BASE_CONFIG}/radarr-uhd`  |

Os caminhos saem como variáveis resolvidas pelo `.env`:

- `BASE_CONFIG` — raiz das pastas de config, uma por container.
- `BASE_MEDIA` — biblioteca. Cada *arr monta a própria subpasta, que nasce com
  o tipo de conteúdo dele mais o que distingue a instância no título (`Sonarr` →
  `tv`, `Sonarr 4K` → `tv-4k`, `Radarr [UHD]` → `movies-uhd`) e é editável no
  modal; o Jellyfin monta a base inteira e o Bazarr acompanha as subpastas das
  instâncias de Radarr/Sonarr presentes na stack.
- `DOWNLOAD_BASE` — área de download. qBittorrent e SABnzbd montam uma
  subpasta própria (`torrents`, `usenet`); os *arr montam a base inteira em
  `/downloads`, para conseguirem importar.

No modal, o campo da subpasta mostra o caminho já resolvido e aceita as
variáveis: digitar `${BASE_MEDIA}` troca pelo valor dela na hora. Apontar para
fora das bases — `/mnt/disco2/filmes-4k`, por exemplo — é permitido, e aí o
compose sai com esse caminho literal, sem variável nenhuma. O Bazarr acompanha:
monta o caminho de cada instância como ela ficou. O Jellyfin também: além da
base inteira, ganha um volume para cada pasta que ficou fora dela, senão a
biblioteca não apareceria para ele. E o modal dele tem um **+ pasta** para
apontar diretórios que nenhum outro serviço usa — um disco antigo, um
compartilhamento de rede. Cada um vira um volume em `/data/<nome da pasta>`.

Todos os volumes usam a sintaxe longa, com `type: bind` e
`bind.propagation: rslave`. A porta é sempre a original do serviço, dentro do
container: fora o nginx, não há porta de host para escolher, nem conflito
possível.

| Serviço      | Porta interna | Serviço      | Porta interna |
| ------------ | ------------- | ------------ | ------------- |
| Sonarr       | `8989`        | Jellyfin     | `8096`        |
| Radarr       | `7878`        | Seerr        | `5055`        |
| Lidarr       | `8686`        | FlareSolverr | `8191`        |
| Prowlarr     | `9696`        | Gluetun      | `8000`        |
| Bazarr       | `6767`        | Nginx        | `80` / `443`  |
| qBittorrent  | `8181`        |              |               |
| SABnzbd      | `8080`        |              |               |

É essa porta que aparece na lista, ao lado do subpath, e que o `proxy_pass` do
nginx usa. A do qBittorrent é a exceção que não vem de fábrica: ele ouviria na
8080, a mesma do SABnzbd, então sai com `WEBUI_PORT=8181` no compose e o
`WebUI\Port` correspondente na conf gerada. O nginx é o único com duas, e são
as de dentro do container — as publicadas no host saem do modal dele.

## Reverse proxy

O nginx é fixo e obrigatório: entra sempre na stack, não aparece no combobox e
não pode ser removido. Fora ele, só o **Seerr** publica porta no host; todos
os outros ficam só na rede `starrnet`, alcançados pelo nginx por
`nome-do-container:porta-interna`. Quem roteia pela VPN responde no `gluetun`,
que é quem detém a rede.

As duas portas do host ficam no **Editar** da linha do nginx: 80 e 443 por
padrão, mas dá para publicar em 8080 e 8443, por exemplo, se algo já ocupa as
privilegiadas. Elas viram `HTTP_PORT` e `HTTPS_PORT` no `.env`; dentro do
container o nginx continua ouvindo em 80 e 443. A **443 só é publicada com o
"Servir HTTPS" ligado**: sem ele o `nginx.conf` não tem `server` nenhum
escutando ali, e publicá-la seria ocupar a porta da máquina sem nada do outro
lado — e impedir a stack de subir onde algo já a usa. Sem TLS, nem a porta nem
o `HTTPS_PORT` saem. Os links copiados e o
redirecionamento para o https já levam a porta escolhida.

A aba **nginx.conf** gera a configuração correspondente, roteando por subpath
(`/sonarr`, `/radarr`…), um `location` por serviço. O arquivo é montado em
`/etc/nginx/conf.d/nginx.conf` do container, a partir do `nginx.conf` da pasta
da stack — a conf é gerada junto com o compose e mora ao lado dele, não no
`BASE_CONFIG`. Com servidor, o caminho sai por extenso, porque é ele quem sabe
onde a stack mora; sem servidor, sai como `./nginx.conf`, relativo à pasta de
onde o compose for rodado. Cada app precisa da sua *base URL* igual ao
subpath. Nenhum serviço fica na raiz: a `/` da stack não tem `location`, então
é pelo subpath de cada app que se entra.

Nem todo serviço vira rota: o `gluetun` e o FlareSolverr só conversam com os
outros containers, então não ganham `location` nem botão de link — o Prowlarr
fala com o FlareSolverr direto pela rede da stack.

O **Seerr** fica fora do proxy: ele não tem base URL configurável, e app sem
base URL não vive num subpath. Em vez de rota, ele **publica a sua porta no
host** — 5055 por padrão, editável no modal dele, que sai no compose como
`ports` e no `.env` como `SEERR_PORT`. O link dele aponta para essa porta, em
`http://`: sem o proxy na frente, o TLS da stack não o cobre, e a porta precisa
estar livre na máquina. Quem roteia pela VPN publica no `gluetun`, que é quem
detém a rede.

O **qBittorrent** também não tem base URL configurável, mas continua no proxy:
a rota dele é a que **retira o prefixo** no caminho, e assim ele responde na
raiz, sem saber que existe um `/qbittorrent`. O bloco traz um `rewrite` que
corta o prefixo, um `resolver 127.0.0.11` — o DNS do Docker, porque o
`proxy_pass` com variável resolve o nome a cada pedido — e um
`location = /qbittorrent` que redireciona para a barra final, com
`absolute_redirect off` para a porta do host não se perder no caminho. Os
estáticos da interface dele são relativos, então acompanham o prefixo; e a conf
que o Hubstarr escreve já traz as chaves de proxy reverso que a **API** dele
exige — sem elas a interface abre e a API responde 403, que é o que o *arr
consulta.

No Ambiente dá para ligar o **TLS**: o `nginx.conf` passa a ter um `server` na
443 com `ssl_certificate`, TLSv1.2/1.3 e um bloco na 80 que só redireciona para
o https. O certificado e a chave são caminhos do host, informados no mesmo
lugar, e entram no compose como `${TLS_CERT}` e `${TLS_KEY}`, montados
só-leitura em `/etc/nginx/certs`. Sem TLS, a stack fica só na 80. O domínio
informado vira o `server_name` (na falta dele, `_`).

## VPN

Marcar um cliente como "rotear pelo gluetun" faz o serviço usar
`network_mode: service:gluetun`, e o gluetun entra na lista de serviços na
hora, se ainda não estiver lá. Ele passa a ser o endereço desse serviço no
nginx. As credenciais ficam no **Editar do gluetun** — provedor, tipo de túnel,
chaves do WireGuard ou usuário/senha do OpenVPN e os países do servidor — e
saem no `.env` como `VPN_SERVICE_PROVIDER`, `VPN_TYPE`, `WIREGUARD_*` ou
`OPENVPN_*` e `SERVER_COUNTRIES`.

## Idiomas

A interface fala português (Brasil), inglês e espanhol. O idioma inicial vem do
que estiver salvo no `localStorage`, caindo para o do navegador e, por fim,
para o português. A tradução cobre também os comentários dos arquivos gerados —
o YAML, o `.env` e o `nginx.conf` saem no idioma escolhido.

Toda string visível está no dicionário `I18N`, no topo do `<script>`: uma chave
por texto, com valor em string ou função quando depende de algum dado. No HTML,
os textos estáticos são marcados com `data-i18n` (ou `data-i18n-html`,
`data-i18n-ph`, `data-i18n-title`). Adicionar um idioma é copiar um dos blocos
e traduzir os valores.

## Wishlist

O que ainda não existe, na ordem em que faria sentido acontecer. Os marcos são
versões, não datas: cada um só começa depois do anterior, porque depende dele.
Hoje o repositório está no **v0.5** — a página, o servidor que guarda a stack e
a sobe no Docker, a Configuração aplicada nos apps e o TRaSH Guides inteiro pelo
Configarr: perfis, custom formats com os scores do guia e as quality
definitions.

| Marco    | Entrega                                              | Fecha quando                                                                            |
| -------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------- |
| ~~**v0.2**~~ | ~~Backend ligando o `hubstarr.html` ao Docker~~   | ✅ a página grava os arquivos e sobe a stack sem passar pelo `.zip`                       |
| ~~**v0.3**~~ | ~~Configuração automática das stacks pelo backend~~ | ✅ Prowlarr, clientes de download e Media Management saem da interface e viram chamada de API |
| ~~**v0.4**~~ | ~~Custom formats e profiles próprios de cada stack~~ | ✅ os perfis do TRaSH Guides, por instância, aplicados pelo Configarr |
| ~~**v0.5**~~ | ~~Compatibilidade com o TRaSH Guides~~            | ✅ quality definitions e scores de custom format vêm nos templates que o Configarr aplica |
| **v0.6** | Nome de projeto e de container configurável           | o Hubstarr convive com uma stack que já use esses nomes na mesma máquina — e volta a caber mais de uma |
| **v0.7** | Busca localizada de mídia                             | dá para escolher o idioma da busca e os *arr acham o lançamento certo                      |

## Verificações

A cada push, o GitHub Actions roda o que dá para conferir sem uma pessoa olhando
(`.github/workflows/ci.yml`). No servidor, `cargo build`, `cargo test` e
`cargo clippy`. Na página, três checagens que também rodam na sua máquina:

```sh
python3 tools/extract-script.py hubstarr.html > page.js && node --check page.js
python3 tools/check-i18n.py hubstarr.html     # os três idiomas, mesmas chaves
python3 tools/check-compose.py                # o compose gerado, pelo docker
```

A última abre a página num navegador sem tela, monta uma stack de exemplo e
passa o `docker-compose.yml` que ela gerou pelo `docker compose config` — é o
mesmo validador que recusaria o arquivo na sua máquina.

Em tag `v*`, o `release.yml` compila o servidor para x86_64 e arm64 e publica
os dois junto do `hubstarr.html` daquela versão.

## Status

A interface é um protótipo, mas o que ela produz não é: os arquivos são de
verdade, e a **Configuração** vira chamada de API em cada app pelo **Subir** e
pelo **Aplicar na stack**. Rodando o servidor, uma stack sai do nada e chega
configurada. Abrindo só a página, saem os arquivos num `.zip`, para rodar
`docker compose up -d` na mão — e o que depende de API fica para você fazer nos
apps.

## Licença

[GNU General Public License v3.0](LICENSE) ou posterior. Use, estude, modifique
e redistribua à vontade; se distribuir uma versão modificada, ela precisa vir
com o código e sob a mesma licença. Sem garantia — veja as seções 15 e 16 do
texto.

Os logotipos dos serviços são de seus respectivos projetos e vêm do
[dashboardicons.com](https://dashboardicons.com); o do nginx é
[Nginx](https://iconscout.com/icons/nginx), de
[Icon 54](https://iconscout.com/contributors/icon-54), no IconScout. A GPL
cobre o Hubstarr, não eles.

Os temas dos apps são do [theme.park](https://theme-park.dev/), projeto à parte,
também sob GPL-3.0: dele vêm a imagem que o compose usa em `TP_THEME`/`TP_ADDON`,
as paletas listadas no campo **Tema**, as capturas que o Hubstarr mostra e os
logotipos das variantes 4K e Animes do Sonarr e do Radarr.

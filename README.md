# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — gerador de *arr stack

*Português (Brasil) · [English](README.en.md) · [Español](README.es.md)*

[<img src="docs/badge-licenca.svg" alt="Licença: GPL-3.0" height="20">](LICENSE)

Protótipo de página única que monta o `docker-compose.yml`, o `.env` e o
`nginx.conf` de uma stack de mídia (*arr + clientes de download + servidor de
mídia), sem dependências externas. A página funciona sozinha, aberta do disco;
um [servidor opcional](#servidor-opcional) guarda as stacks em SQLite e sobe a
stack no Docker sem passar pelo `.zip`.

> [!WARNING]
> **Protótipo.** O Hubstarr não foi projetado para uso em produção: os arquivos
> que ele gera são um ponto de partida, sem endurecimento de segurança, backup
> ou monitoramento. Revise tudo — senhas, portas, certificados e permissões —
> antes de expor a stack a qualquer rede que não seja a sua.

Abra `hubstarr.html` no navegador. É só isso — o arquivo é
autocontido (os logotipos vêm embutidos como data URI). O **Ambiente** abre
junto: é dali que saem as bases de caminho que todo o resto usa. Fechou, ele
continua no botão do topo.

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
- **Copiar o link** de cada serviço, já com o esquema, o domínio e o subpath
  pelos quais o nginx vai atendê-lo.
- **Aviso de conflito**: duas instâncias apontadas para a mesma pasta se
  atropelam na importação, então a lista avisa em vermelho, com os nomes e o
  caminho. O Jellyfin, que monta a biblioteca inteira, e o Bazarr, que segue as
  outras, ficam de fora da checagem.
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
  [theme.park](https://docs.theme-park.dev/), uma por app.
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
- **network.xml do Jellyfin**: com ele na stack, sai também a configuração de
  rede dele, com o `BaseUrl` no subpath do nginx e o `nginx` em `KnownProxies` —
  sem o primeiro a interface monta os links na raiz, sem o segundo ele registra
  o IP do proxy no lugar do IP de quem pediu. Montada em
  `/config/config/network.xml`.
- **qBittorrent.conf pronta**: quando ele está na stack, uma quarta aba gera a
  configuração inicial dele — pastas iguais às do compose, ajustes de proxy
  reverso e as credenciais no formato do próprio qBittorrent 5.2: a senha em
  PBKDF2-SHA512 e a API key `qbt_` + 28 caracteres, derivada da
  `${STARR_APIKEY}` da stack — a conf é lida por ele, não pelo compose, então a
  variável não seria expandida ali. Usuário, senha e chave se editam no modal
  dele. O arquivo **não é montado**: quem manda nele é o próprio qBittorrent, e
  montá-lo congelaria tudo o que ele guarda ali. Com servidor, o **Subir**
  escreve essas chaves na conf que o app criou — parando o container, fazendo a
  troca e subindo de novo, porque ele reescreve o arquivo inteiro ao sair. Sem
  servidor, é copiar o conteúdo da aba para o caminho indicado no topo dela.
- **categories.json do qBittorrent**: junto da conf sai um segundo arquivo com
  as categorias que a **Configuração** deu a cada *arr, já criadas quando ele
  sobe. Cada uma ganha a subpasta dela dentro do caminho de download — mesma
  partição, então o *arr continua fazendo hardlink em vez de copiar. Como a
  conf, ele não é montado: o **Subir** põe essas categorias nas que o app já
  tem, sem apagar as que você criou na interface dele.
- **HTTPS opcional**, com o certificado e a chave vindos do host.
- **Configuração** (botão no topo): escolher quais instâncias o Prowlarr vai
  configurar, com que categoria cada *arr usa cada cliente de download —
  `tv-sonarr`, `radarr`, `lidarr` no qBittorrent, e as categorias de fábrica do
  SABnzbd (`tv`, `movies`, `music`), todas editáveis —,
  mais o gerenciamento de downloads concluídos no SABnzbd, e as opções de
  *Media Management* — hardlink, renomear, permissões,
  pastas vazias e a nomenclatura completa de cada app (*Episode Naming*,
  *Nomenclatura de filme*, *Nomeação da faixa*: caracteres ilegais,
  dois-pontos, vários episódios e todos os formatos de arquivo e de pasta) —,
  separadas por família: Sonarr, Radarr e Lidarr. Os formatos de episódio e de
  filme já vêm com os do [TRaSH Guides](https://trash-guides.info), na variante
  do Jellyfin com o id do TMDb. As permissões abrem os campos
  de `chmod` e `chown`, e no Lidarr a caixa de nome existente é quem traz os
  formatos de faixa e a pasta do álbum.

  ![O modal da Configuração, na nomenclatura de episódio do Sonarr](docs/config.png)

  Com mais de um Sonarr na stack, cada
  formato de episódio — **padrão**, **diário** e **anime** — traz a lista das
  instâncias que o recebem: dá para mandar o formato de anime só para o
  *Sonarr [Anime]*, por exemplo. Pelo menos uma instância é obrigatória, porque
  o campo é obrigatório no app; a que ficar de fora mantém o formato que já
  tem, em vez de perdê-lo. Por
  enquanto, das três partes, só os clientes de download chegam aos apps, pelo
  **Aplicar na stack**.
- **Ambiente global** (botão no topo): bases de caminho, PUID/PGID, time zone,
  restart policy, API key e TLS. A
  lista de fusos é a IANA inteira, vinda do próprio navegador, e o valor
  inicial é o fuso da máquina.
- **Baixar** `docker-compose.yml`, `.env` e `nginx/conf.d/starrnet.conf` juntos
  num `.zip` — o botão fica na barra enquanto não há servidor; com ele, quem
  grava os arquivos é o **Subir**.
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

## Servidor (opcional)

A página continua sendo o produto: é ela que gera os arquivos, e aberta do
disco funciona inteira, com o `.zip` e mais nada. O servidor em `backend/`
acrescenta o que o navegador não alcança sozinho — **guardar as stacks entre
sessões**, gravar os arquivos em disco e subir a stack no Docker; com ele no
ar, o botão do `.zip` sai da barra, porque o **Subir** grava os mesmos
arquivos. Ele nunca
gera conteúdo: recebe pronto o que os geradores da página montaram.

Compilar e rodar precisa do [Rust](https://rustup.rs):

```sh
cd backend
cargo run --release
```

Abra `http://127.0.0.1:7878`. A página é a mesma — servida pelo binário, que a
traz embutida —, com duas coisas a mais: o selo **servidor** no topo e, nos
arquivos gerados, os botões **Subir** e **Derrubar**. Ao abrir, ela pergunta ao
servidor se há `docker compose` ali; se não houver, avisa e já abre o bloco
**Precisa instalar o Docker?**.

| Opção      | Padrão                   | O que é                                        |
| ---------- | ------------------------ | ---------------------------------------------- |
| `--addr`   | `127.0.0.1:7878`         | endereço em que o servidor atende               |
| `--dir`    | `./stack`                | pasta em que os arquivos gerados são gravados   |
| `--db`     | `~/.hubstarr/hubstarr.db`| banco em que a stack é guardada                 |
| `--docker` | `docker`                 | comando do docker, para quem usa podman         |

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

O **Subir** já deixa a stack configurada: assim que os apps respondem, o
servidor registra cada cliente de download em cada *arr **e no próprio
Prowlarr** — que tem o Settings → Download Clients dele —, cada *arr marcado em
Settings → Apps do Prowlarr, e o *Media Management* com a nomenclatura de cada
família, tudo pela API deles, mostrando o que passou. O botão **Aplicar na
stack**, no modal da Configuração, faz o mesmo sem subir nada — é o caminho
para reaplicar depois de mexer nas escolhas.

No Prowlarr, o Settings → Download Clients ganha **um registro por cliente**,
todos na categoria `prowlarr`: o que ele pega é avulso, não veio de um *arr,
então fica junto e separado do que cada instância baixa. E as categorias passam
a existir dentro do cliente — a de cada *arr e a do Prowlarr: no qBittorrent
pelo `categories.json`, e no SABnzbd criadas pela API dele, cada uma com a pasta
de mesmo nome dentro do diretório de downloads concluídos. Os apps são alcançados pelo nginx, na porta que ele
publica no host. Aplicar de novo não duplica — o cliente é procurado pelo nome
e atualizado no lugar —, e um app que ainda não subiu vira uma linha no log em
vez de interromper o resto. O SABnzbd precisa da chave de API dele, que é o
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
container o nginx continua ouvindo em 80 e 443. Os links copiados e o
redirecionamento para o https já levam a porta escolhida.

A aba **nginx.conf** gera a configuração correspondente, roteando por subpath
(`/sonarr`, `/radarr`…), um `location` por serviço. O arquivo é montado em
`${BASE_CONFIG}/nginx/conf.d` e cada app precisa da sua *base URL* igual ao
subpath. Nenhum serviço fica na raiz: a `/` da stack não tem `location`, então
é pelo subpath de cada app que se entra.

Nem todo serviço vira rota: o `gluetun` e o FlareSolverr só conversam com os
outros containers, então não ganham `location` nem botão de link — o Prowlarr
fala com o FlareSolverr direto pela rede da stack.

O Seerr fica fora do proxy por outro motivo: ele não tem base URL nenhuma, e
app sem base URL não vive num subpath. Em vez de rota, ele **publica a porta no
host** — 5055 por padrão, editável no modal dele, que sai no compose como
`ports` e no `.env` como `SEERR_PORT`. O link dele aponta para essa porta, em
`http://`: sem o proxy na frente, o TLS da stack não o cobre, e a porta precisa
estar livre na máquina.

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
Hoje o repositório está no **v0.3** — a página, o servidor opcional que guarda
as stacks e as sobe no Docker, e a Configuração aplicada nos apps.

| Marco    | Entrega                                              | Fecha quando                                                                            |
| -------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------- |
| ~~**v0.2**~~ | ~~Backend ligando o `hubstarr.html` ao Docker~~   | ✅ a página grava os arquivos e sobe a stack sem passar pelo `.zip`                       |
| ~~**v0.3**~~ | ~~Configuração automática das stacks pelo backend~~ | ✅ Prowlarr, clientes de download e Media Management saem da interface e viram chamada de API |
| **v0.4** | Custom formats e profiles próprios de cada stack      | a instância de 4K, a de anime e as demais nascem com o perfil de qualidade delas           |
| **v0.5** | Compatibilidade com o TRaSH Guides                    | quality definitions, scores de custom format e as demais recomendações do guia saem prontas |
| **v0.6** | Busca localizada de mídia                             | dá para escolher o idioma da busca e os *arr acham o lançamento certo                      |

## Status

A página é um protótipo de interface, mas a **Configuração** não é mais só
interface: as escolhas ficam guardadas — na página, e no banco quando há
servidor — e, com servidor e a stack no ar, as três partes dela viram chamada de
API pelo **Aplicar na stack**. Os arquivos gerados, esses sempre foram
de verdade — é baixar o `.zip` e rodar o `docker compose up -d` na pasta onde
ele foi aberto, ou deixar o servidor fazer os dois.

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

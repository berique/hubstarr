# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — gerador de *arr stack

*Português (Brasil) · [English](README.en.md) · [Español](README.es.md)*

Protótipo de página única que monta o `docker-compose.yml`, o `.env` e o
`nginx.conf` de uma stack de mídia (*arr + clientes de download + servidor de
mídia), sem backend e sem dependências externas.

Abra `hubstarr.html` no navegador. É só isso — o arquivo é
autocontido (os logotipos vêm embutidos como data URI). O **Ambiente** abre
junto: é dali que saem as bases de caminho que todo o resto usa. Fechou, ele
continua no botão do topo.

![A interface: lista de serviços à esquerda, arquivos gerados à direita](docs/screenshot.png)

O combobox lista os serviços disponíveis com seus logotipos e portas padrão:

![O combobox aberto, mostrando os doze serviços disponíveis](docs/services.png)

## O que dá para fazer

- **Escolher serviços** num combobox com logotipos e adicioná-los à stack.
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
  dele, e o arquivo é montado por cima do `/config`.
- **HTTPS opcional**, com o certificado e a chave vindos do host.
- **Configuração** (botão no topo): escolher quais instâncias o Prowlarr vai
  configurar, quais *arr recebem cada cliente de download (qBittorrent,
  SABnzbd) e com que categoria — `tv-sonarr`, `radarr`, `lidarr`, editáveis —,
  mais o gerenciamento de downloads concluídos no SABnzbd, e as opções de
  *Media Management* — hardlink, renomear, permissões,
  pastas vazias e a nomenclatura completa de cada app (*Episode Naming*,
  *Nomenclatura de filme*, *Nomeação da faixa*: caracteres ilegais,
  dois-pontos, vários episódios e todos os formatos de arquivo e de pasta) —,
  separadas por família: Sonarr, Radarr e Lidarr. As permissões abrem os campos
  de `chmod` e `chown`, e no Lidarr a caixa de nome existente é quem traz os
  formatos de faixa e a pasta do álbum. Por
  enquanto as escolhas ficam guardadas na interface; nada é aplicado nos apps.
- **Ambiente global** (botão no topo): bases de caminho, PUID/PGID, time zone,
  restart policy, API key e TLS. A
  lista de fusos é a IANA inteira, vinda do próprio navegador, e o valor
  inicial é o fuso da máquina.
- **Baixar** `docker-compose.yml`, `.env` e `nginx/conf.d/starrnet.conf` juntos
  num `.zip`.
- **Trocar o idioma** no seletor do topo: português (Brasil), inglês e
  espanhol.

## Docker

O Hubstarr em si só precisa de um navegador; os arquivos que ele gera é que
precisam do Docker com o plugin Compose. No Linux, o script oficial resolve:

```sh
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

No macOS e no Windows — ou no Linux, se preferir uma instalação gerenciada com
interface gráfica — instale o [Docker Desktop][dd].

[dd]: https://docs.docker.com/desktop/

Com o Docker no lugar, descompacte o `.zip` e suba a stack de dentro da pasta
dos arquivos:

```sh
docker compose up -d
```

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
- `BASE_MEDIA` — biblioteca. Cada *arr monta a própria subpasta
  (`series`, `movies`, `music`), o Jellyfin monta a base inteira e o Bazarr
  acompanha as subpastas das instâncias de Radarr/Sonarr presentes na stack.
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

## Reverse proxy

O nginx é fixo e obrigatório: entra sempre na stack, não aparece no combobox e
não pode ser removido. O Heimdall também entra sozinho — é o painel de
atalhos que fica na raiz —, mas esse dá para editar; só não sai da lista.
 É o único container que publica portas no host — todos
os outros ficam só na rede `starrnet`, alcançados pelo nginx por
`nome-do-container:porta-interna`. Quem roteia pela VPN responde no `gluetun`,
que é quem detém a rede.

As duas portas do host ficam no **Editar** da linha do nginx: 80 e 443 por
padrão, mas dá para publicar em 8080 e 8443, por exemplo, se algo já ocupa as
privilegiadas. Elas viram `HTTP_PORT` e `HTTPS_PORT` no `.env`; dentro do
container o nginx continua ouvindo em 80 e 443. Os links copiados e o
redirecionamento para o https já levam a porta escolhida.

A aba **nginx.conf** gera a configuração correspondente, roteando por subpath
(`/sonarr`, `/radarr`…), um `location` por serviço. O Heimdall é a exceção:
como painel de atalhos, fica na raiz (`location /`). O arquivo é montado em
`${BASE_CONFIG}/nginx/conf.d` e cada app precisa da sua *base URL* igual ao
subpath.

Nem todo serviço vira rota: o `gluetun` e o FlareSolverr só conversam com os
outros containers, então não ganham `location` nem botão de link — o Prowlarr
fala com o FlareSolverr direto pela rede da stack.

O Seerr é o oposto: não tem base URL nenhuma, então o `location` dele tira o
prefixo na entrada e reescreve o que volta — os cabeçalhos de redirect e os
caminhos que ele escreve no HTML, por `sub_filter`.

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

## Servidor (opcional)

A página continua funcionando sozinha, aberta do disco. Quem quiser que o botão
"Criar stack" crie a stack de verdade pode subir o servidor que está em
`backend/` — um binário em Rust que faz o que o navegador não alcança:

```sh
cd backend
cargo run --release -- --dir ~/starr
```

Depois é só abrir <http://127.0.0.1:7878>. A página servida por ele é a mesma
`hubstarr.html`, embutida no binário, e ela detecta o servidor sozinha: quando
há um, aparece a etiqueta *servidor* no cabeçalho e três coisas mudam.

- **Criar stack** grava os arquivos gerados na pasta do `--dir` e roda
  `docker compose up -d`, com a saída num log ao vivo. Ao lado dele aparece
  **Derrubar**, que roda o `docker compose down`.
- **A stack fica guardada.** As instâncias, o Ambiente e a Configuração vão
  para um SQLite (`stack.db`) na mesma pasta e voltam ao recarregar a página.
  Cada adicionar, editar ou excluir mexe na linha daquele serviço.
- **A Configuração vira realidade.** Com a stack de pé, o botão *Aplicar a
  Configuração* usa a API de cada app para criar o que só existe no banco
  deles: o Prowlarr apontando para cada *arr, os clientes de download com a
  categoria de cada um, e o Media Management com a nomenclatura. É a única
  parte da interface que não cabe em arquivo — e ela é idempotente, então
  aplicar de novo depois de mexer na Configuração é o uso normal.

Sem servidor nada disso aparece, e o comportamento é o de sempre: `.zip` e
deploy simulado. O servidor nunca gera conteúdo — recebe pronto o que a página
montou, para os geradores continuarem existindo num lugar só.

Opções: `--dir` (pasta dos arquivos, padrão `./stack`), `--addr` (endereço,
padrão `127.0.0.1:7878`) e `--docker` (o comando, para quem usa podman).

## Status

A página sozinha é um protótipo de interface: sem servidor, o botão "Criar
stack" apenas simula o deploy e as escolhas da **Configuração** não viram
chamada de API nenhuma. Os arquivos gerados, esses sempre foram de verdade — e
com o servidor de `backend/` o deploy e a Configuração também passam a ser.

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

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
- **Múltiplas instâncias** de Sonarr, Radarr, Lidarr, Bazarr e Prowlarr —
  basta o título ser diferente. Sonarr e Radarr recebem também
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Base URL automática**: Sonarr, Radarr, Lidarr e Prowlarr recebem
  `<APP>__SERVER__URLBASE=/<container_name>`, já casando com o subpath do
  nginx. O Bazarr não expõe essa variável — a base fica na interface dele.
- **API key** no Ambiente: uma só para toda a stack. Sonarr, Radarr, Lidarr e
  Prowlarr saem no compose com `<APP>__AUTH__APIKEY=${STARR_APIKEY}` e o valor
  fica no `.env`. A chave já nasce sorteada — 16 bytes em hexadecimal, o mesmo
  que `openssl rand -hex 16` — e o botão "Gerar" sorteia outra.
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
- **HTTPS opcional**, com o certificado e a chave vindos do host.
- **Configuração** (botão no topo): escolher quais instâncias o Prowlarr vai
  configurar, quais *arr recebem cada cliente de download (qBittorrent,
  SABnzbd) e as opções de *Media Management* — hardlink, renomear, permissões,
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
não pode ser removido. É o único container que publica portas no host — todos
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

## Status

Protótipo de interface: o botão "Criar stack" apenas simula o deploy, e as
escolhas da **Configuração** ainda não viram chamada de API nenhuma. O
`docker-compose.yml`, o `.env` e o `nginx.conf` gerados, esses sim, são de
verdade.

## Licença

[GNU General Public License v3.0](LICENSE) ou posterior. Use, estude, modifique
e redistribua à vontade; se distribuir uma versão modificada, ela precisa vir
com o código e sob a mesma licença. Sem garantia — veja as seções 15 e 16 do
texto.

Os logotipos dos serviços são de seus respectivos projetos e vêm do
[dashboardicons.com](https://dashboardicons.com); a GPL cobre o Hubstarr, não
eles.

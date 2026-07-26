# Hubstarr — gerador de *arr stack

Protótipo de página única que monta um `docker-compose.yml` e o `.env`
correspondente para uma stack de mídia (*arr + clientes de download + servidor
de mídia), sem backend e sem dependências externas.

Abra `arr-stack-prototype.html` no navegador. É só isso — o arquivo é
autocontido (os logotipos vêm embutidos como data URI).

## O que dá para fazer

- **Escolher serviços** num combobox com logotipos e adicioná-los à stack.
- **Configurar cada instância** num modal: título, porta no host, subpasta de
  mídia/downloads e roteamento pela VPN.
- **Múltiplas instâncias** de Sonarr, Radarr, Lidarr, Bazarr e Prowlarr —
  basta o título ser diferente. Sonarr e Radarr recebem também
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Ambiente global** (botão no topo): bases de caminho, PUID/PGID, timezone,
  nome da stack, restart policy e as credenciais do gluetun.
- **Baixar** `docker-compose.yml` e `.env` juntos num `.zip`.

## Convenções geradas

O título de cada instância vira um slug (minúsculas, sem acentos, espaços como
hífen) usado como `container_name`, chave do serviço e pasta de config:

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

Todos os volumes usam a sintaxe longa, com `type: bind` e
`bind.propagation: rslave`. A porta interna do container é sempre a original do
serviço — só a porta publicada no host é editável, e conflitos são apontados na
própria interface.

## VPN

Marcar um cliente como "rotear pelo gluetun" faz o serviço usar
`network_mode: service:gluetun`; o gluetun é adicionado à stack
automaticamente e publica as portas por ele. As credenciais
(`VPN_SERVICE_PROVIDER`, `VPN_TYPE`, chaves do WireGuard ou usuário/senha do
OpenVPN, `SERVER_COUNTRIES`) ficam no `.env`.

## Status

Protótipo de interface: o botão "Criar stack" apenas simula o deploy. O
`docker-compose.yml` e o `.env` gerados, esses sim, são de verdade.

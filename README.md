# Hubstarr — gerador de *arr stack

Protótipo de página única que monta um `docker-compose.yml` e o `.env`
correspondente para uma stack de mídia (*arr + clientes de download + servidor
de mídia), sem backend e sem dependências externas.

Abra `arr-stack-prototype.html` no navegador. É só isso — o arquivo é
autocontido (os logotipos vêm embutidos como data URI).

![A interface: lista de serviços à esquerda, docker-compose.yml gerado à direita](docs/screenshot.png)

O combobox lista os serviços disponíveis com seus logotipos e portas padrão:

![O combobox aberto, mostrando os doze serviços disponíveis](docs/services.png)

## O que dá para fazer

- **Escolher serviços** num combobox com logotipos e adicioná-los à stack.
- **Configurar cada instância** num modal: título, subpasta de mídia/downloads
  e roteamento pela VPN.
- **Múltiplas instâncias** de Sonarr, Radarr, Lidarr, Bazarr e Prowlarr —
  basta o título ser diferente. Sonarr e Radarr recebem também
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Base URL automática**: Sonarr, Radarr, Lidarr e Prowlarr recebem
  `<APP>__SERVER__URLBASE=/<container_name>`, já casando com o subpath do
  nginx. O Bazarr não expõe essa variável — a base fica na interface dele.
- **API keys** no Ambiente: uma por app (Sonarr, Radarr, Lidarr, Prowlarr). O
  compose sai com `<APP>__AUTH__APIKEY=${<APP>_API_KEY}` e o valor fica no
  `.env`. Instâncias do mesmo app compartilham a chave.
- **Ambiente global** (botão no topo): bases de caminho, PUID/PGID, time zone,
  nome da stack, restart policy e as credenciais do gluetun. A lista de fusos é
  a IANA inteira, vinda do próprio navegador, e o valor inicial é o fuso da
  máquina.
- **Baixar** `docker-compose.yml`, `.env` e `nginx/conf.d/starrnet.conf` juntos
  num `.zip`.
- **Trocar o idioma** no seletor do topo: português (Brasil), inglês e
  espanhol.

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
`bind.propagation: rslave`. A porta é sempre a original do serviço, dentro do
container: não há porta de host para escolher, nem conflito possível.

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

## Reverse proxy

O nginx é fixo e obrigatório: entra sempre na stack, não aparece no combobox e
não pode ser removido. É o único container que publica portas no host (80 e
443) — todos os outros ficam só na rede `starrnet`, alcançados pelo nginx por
`nome-do-container:porta-interna`. Quem roteia pela VPN responde no `gluetun`,
que é quem detém a rede.

A aba **nginx.conf** gera a configuração correspondente, roteando por subpath
(`/sonarr`, `/radarr`…), um `location` por serviço. O Heimdall é a exceção:
como painel de atalhos, fica na raiz (`location /`). O arquivo é montado em
`${BASE_CONFIG}/nginx/conf.d` e cada app precisa da sua *base URL* igual ao
subpath.

## VPN

Marcar um cliente como "rotear pelo gluetun" faz o serviço usar
`network_mode: service:gluetun`; o gluetun é adicionado à stack
automaticamente e passa a ser o endereço desse serviço no nginx. As credenciais
(`VPN_SERVICE_PROVIDER`, `VPN_TYPE`, chaves do WireGuard ou usuário/senha do
OpenVPN, `SERVER_COUNTRIES`) ficam no `.env`.

## Status

Protótipo de interface: o botão "Criar stack" apenas simula o deploy. O
`docker-compose.yml` e o `.env` gerados, esses sim, são de verdade.

# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — generador de *arr stack

*[Português (Brasil)](README.md) · [English](README.en.md) · Español*

[<img src="docs/badge-licencia.svg" alt="Licencia: GPL-3.0" height="20">](LICENSE)

Prototipo de página única que arma el `docker-compose.yml`, el `.env` y el
`nginx.conf` de una stack multimedia (*arr + clientes de descarga + servidor
multimedia), sin backend y sin dependencias externas.

> [!WARNING]
> **Prototipo.** Hubstarr no fue diseñado para uso en producción: los archivos
> que genera son un punto de partida, sin endurecimiento de seguridad, copias
> de seguridad ni monitorización. Revísalo todo — contraseñas, puertos,
> certificados y permisos — antes de exponer la stack a cualquier red que no
> sea la tuya.

Abre `hubstarr.html` en el navegador. Eso es todo — el archivo es
autocontenido (los logotipos van incrustados como data URI). El **Entorno** se
abre junto: de ahí salen las rutas base que usa todo lo demás. Si lo cierras,
sigue en el botón de arriba.

![La interfaz: lista de servicios a la izquierda, archivos generados a la derecha](docs/screenshot.png)

El combobox lista los servicios disponibles con sus logotipos y puertos por
defecto:

![El combobox abierto, mostrando los doce servicios disponibles](docs/services.png)

## Qué se puede hacer

- **Elegir servicios** en un combobox con logotipos y añadirlos a la stack.
- **Configurar cada instancia** en un modal: título, subcarpeta multimedia o de
  descargas y enrutamiento por la VPN.
- **Copiar el enlace** de cada servicio, ya con el esquema, el dominio y el
  subpath por los que nginx lo va a atender.
- **Aviso de conflicto**: dos instancias apuntadas a la misma carpeta se pisan
  al importar, así que la lista lo avisa en rojo, con los nombres y la ruta.
  Jellyfin, que monta la biblioteca entera, y Bazarr, que sigue a las otras,
  quedan fuera de la comprobación.
- **Múltiples instancias** de Sonarr, Radarr, Lidarr, Bazarr y Prowlarr — basta
  con que el título sea distinto. Sonarr y Radarr reciben además
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Base URL automática**: Sonarr, Radarr, Lidarr y Prowlarr reciben
  `<APP>__SERVER__URLBASE=/<container_name>`, que ya coincide con el subpath
  del nginx. Bazarr no expone esa variable — su base se configura en su propia
  interfaz.
- **API key** en el Entorno: una sola para toda la stack. Sonarr, Radarr,
  Lidarr y Prowlarr salen en el compose con
  `<APP>__AUTH__APIKEY=${STARR_APIKEY}`, y SABnzbd con
  `SAB_API_KEY=${STARR_APIKEY}`; el valor queda en el `.env`. La clave
  nace sorteada — 16 bytes en hexadecimal, lo mismo que
  `openssl rand -hex 16` — y el botón "Generar" sortea otra.
- **Aceleración por hardware de Jellyfin**: CPU, Intel o NVIDIA. Intel recibe
  `devices: /dev/dri:/dev/dri`; NVIDIA, la reserva de GPU en `deploy` y las
  variables `NVIDIA_VISIBLE_DEVICES` / `NVIDIA_DRIVER_CAPABILITIES`.
- **Tema de theme.park**: los servicios con imagen de linuxserver — Sonarr,
  Radarr, Lidarr, Prowlarr, Bazarr, qBittorrent, SABnzbd, Jellyfin y Heimdall —
  salen con `DOCKER_MODS=ghcr.io/themepark-dev/theme.park:<app>`, el mod que
  aplica el tema en su interfaz. En Sonarr y Radarr el modal trae además un
  campo **Variante**, que se vuelve `TP_ADDON`: *Predeterminado* usa el addon
  oscuro (`sonarr-darker`), *4K* cambia logotipo y favicon por los del addon de 4K
  (`sonarr-4k-logo|sonarr-4k-favicon`) y *Anime* cambia ambos por los de anime
  (`sonarr-anime-logo|sonarr-anime-favicon`) — útil para distinguir las
  instancias de una stack con más de un Sonarr o Radarr. Ambos tienen también un
  campo **Tema**, la paleta en `TP_THEME`: `aquamarine`, `hotline`, `hotpink`,
  `dracula`, `dark`, `organizr` (el predeterminado), `space-gray`, `overseerr` y
  `nord`. Debajo del campo, un enlace abre la captura de la paleta elegida en la
  documentación de [theme.park](https://docs.theme-park.dev/), una por app.
- **FlareSolverr junto a Prowlarr**: en el modal de Prowlarr, una casilla
  marcada por defecto trae FlareSolverr a la stack — es quien resuelve el
  desafío anti-bot de Cloudflare en los indexadores protegidos. Configúralo en
  Prowlarr en *Settings → Indexers → FlareSolverr*, con la URL
  `http://flaresolverr:8191`. La imagen detrás de él es la de
  [Byparr](https://github.com/ThePhaseless/Byparr), reemplazo directo y más
  actual, con la misma API y el mismo puerto.
- **Ayuda por campo** en el Entorno y en la Configuración: cada línea tiene un
  `?` que abre una explicación de lo que hace ese valor — y, en el Entorno, de
  cómo sale en los archivos generados.
- **network.xml de Jellyfin**: con él en la stack, sale también su
  configuración de red, con el `BaseUrl` en el subpath de nginx y `nginx` en
  `KnownProxies` — sin lo primero la interfaz arma los enlaces en la raíz, sin
  lo segundo registra la IP del proxy en vez de la de quien pidió. Montado en
  `/config/config/network.xml`.
- **qBittorrent.conf lista**: cuando está en la stack, una cuarta pestaña
  genera su configuración inicial — rutas iguales a las del compose, ajustes de
  proxy inverso y las credenciales en el formato del propio qBittorrent 5.2: la
  contraseña en PBKDF2-SHA512 y la API key `qbt_` más 28 caracteres, derivada
  de la `${STARR_APIKEY}` de la stack — la conf la lee qBittorrent, no compose,
  así que la variable no se expandiría ahí. Usuario, contraseña y clave se
  editan en su modal, y el archivo se monta sobre `/config`.
- **HTTPS opcional**, con el certificado y la clave provenientes del host.
- **Configuración** (botón arriba): elegir qué instancias configurará Prowlarr,
  qué *arr reciben cada cliente de descarga (qBittorrent, SABnzbd) y con qué
  categoría — `tv-sonarr`, `radarr`, `lidarr`, editables —, además de la
  gestión de descargas completadas en SABnzbd, y las opciones de
  *Media Management* — hardlink, renombrar, permisos, carpetas
  vacías y la nomenclatura completa de cada app (*Episode Naming*,
  *Nomenclatura de película*, *Nomenclatura de pista*: caracteres ilegales, dos
  puntos, varios episodios y todos los formatos de archivo y de carpeta) —,
  separadas por familia: Sonarr, Radarr y Lidarr. Los formatos de episodio y de
  película ya vienen con los de [TRaSH Guides](https://trash-guides.info), en la
  variante de Jellyfin con el id de TMDb. Los permisos abren los campos
  de `chmod` y `chown`, y en Lidarr la casilla de nombre existente es la que
  trae los formatos de pista y la carpeta del álbum. Por ahora las
  opciones se guardan en la interfaz; no se aplica nada en las apps.
- **Entorno global** (botón arriba): rutas base, PUID/PGID, zona horaria,
  restart policy, API key y TLS. La lista de husos es la IANA entera, que viene
  del propio navegador, y arranca en el huso de la máquina.
- **Descargar** `docker-compose.yml`, `.env` y `nginx/conf.d/starrnet.conf`
  juntos en un `.zip`.
- **Cambiar el idioma** en el selector de arriba: portugués (Brasil), inglés y
  español.

## Docker

El resumen de esta sección también está en la propia página, en un aviso
plegable encima de los paneles.

Hubstarr en sí solo necesita un navegador; son los archivos que genera los que
necesitan Docker con el plugin Compose. En Linux, el script oficial lo
resuelve:

```sh
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

En macOS y Windows — o en Linux, si prefieres una instalación gestionada con
interfaz gráfica — instala [Docker Desktop][dd], que ya trae Compose.

Para usar `docker` sin `sudo`, añade tu usuario al grupo — el cambio vale en la
sesión siguiente:

```sh
sudo usermod -aG docker $USER
```

[dd]: https://docs.docker.com/desktop/

Con Docker en su sitio, descomprime el `.zip` y levanta la stack desde la
carpeta de los archivos:

```sh
docker compose up -d
```

## Convenciones generadas

El nombre de la stack y el de la red son fijos: `starrnet`. El título de cada
instancia se convierte en un slug (minúsculas, sin acentos, espacios como
guiones) usado como `container_name`, clave del servicio y carpeta de config:

| Título          | container_name | config                       |
| --------------- | -------------- | ---------------------------- |
| `Radarr`        | `radarr`       | `${BASE_CONFIG}/radarr`      |
| `Radarr [UHD]`  | `radarr-uhd`   | `${BASE_CONFIG}/radarr-uhd`  |

Las rutas salen como variables resueltas por el `.env`:

- `BASE_CONFIG` — raíz de las carpetas de config, una por contenedor.
- `BASE_MEDIA` — la biblioteca. Cada *arr monta su propia subcarpeta, que nace
  con su tipo de contenido más lo que distingue la instancia en el título
  (`Sonarr` → `tv`, `Sonarr 4K` → `tv-4k`, `Radarr [UHD]` → `movies-uhd`) y se
  edita en el modal; Jellyfin monta la base entera y Bazarr sigue las
  subcarpetas de las instancias de Radarr/Sonarr presentes en la stack.
- `DOWNLOAD_BASE` — el área de descargas. qBittorrent y SABnzbd montan su
  propia subcarpeta (`torrents`, `usenet`); los *arr montan la base entera en
  `/downloads`, para poder importar.

En el modal, el campo de la subcarpeta muestra la ruta ya resuelta y acepta las
variables: escribir `${BASE_MEDIA}` la cambia por su valor al instante. Apuntar
fuera de las bases — `/mnt/disco2/peliculas-4k`, por ejemplo — está permitido, y
entonces el compose sale con esa ruta literal, sin ninguna variable. Bazarr
acompaña: monta la ruta de cada instancia tal como quedó. Jellyfin también:
además de la base entera, recibe un volumen por cada carpeta que quedó fuera de
ella, si no esa biblioteca no le aparecería. Y su modal tiene un **+ carpeta**
para apuntar directorios que ningún otro servicio usa — un disco viejo, un
recurso de red. Cada uno se vuelve un volumen en `/data/<nombre de la carpeta>`.

Todos los volúmenes usan la sintaxis larga, con `type: bind` y
`bind.propagation: rslave`. El puerto siempre es el original del servicio,
dentro del contenedor: fuera de nginx, no hay puerto de host que elegir, ni
conflicto posible.

| Servicio     | Puerto interno | Servicio     | Puerto interno |
| ------------ | -------------- | ------------ | -------------- |
| Sonarr       | `8989`         | Jellyfin     | `8096`         |
| Radarr       | `7878`         | Seerr        | `5055`         |
| Lidarr       | `8686`         | Heimdall     | `80`           |
| Prowlarr     | `9696`         | FlareSolverr | `8191`         |
| Bazarr       | `6767`         | Gluetun      | `8000`         |
| qBittorrent  | `8181`         | Nginx        | `80` / `443`   |
| SABnzbd      | `8080`         |              |                |

Es ese puerto el que aparece en la lista, junto al subpath, y el que usa el
`proxy_pass` de nginx. El de qBittorrent es la excepción que no viene de
fábrica: escucharía en el 8080, el mismo del SABnzbd, así que sale con
`WEBUI_PORT=8181` en el compose y el `WebUI\Port` correspondiente en la conf
generada. Nginx es el único con dos, y son los de dentro del contenedor — los
publicados en el host salen de su propio modal.

## Reverse proxy

nginx es fijo y obligatorio: siempre entra en la stack, no aparece en el
combobox y no se puede eliminar. Heimdall también entra solo — es el panel de
accesos que se queda en la raíz —, pero ese sí se puede editar; solo no sale de
la lista. Es el único contenedor que publica puertos en
el host — todos los demás se quedan solo en la red `starrnet`, alcanzados por
nginx en `nombre-del-contenedor:puerto-interno`. Lo que se enruta por la VPN
responde en `gluetun`, que es quien tiene la red.

Los dos puertos del host están en el **Editar** de la línea de nginx: 80 y 443
por defecto, pero se puede publicar en 8080 y 8443, por ejemplo, si algo ya
ocupa los privilegiados. Salen como `HTTP_PORT` y `HTTPS_PORT` en el `.env`;
dentro del contenedor nginx sigue escuchando en 80 y 443. Los enlaces copiados
y la redirección al https ya llevan el puerto elegido.

La pestaña **nginx.conf** genera la configuración correspondiente, enrutando
por subpath (`/sonarr`, `/radarr`…), un `location` por servicio. Heimdall es la
excepción: como panel de accesos, se queda en la raíz (`location /`). El
archivo se monta en `${BASE_CONFIG}/nginx/conf.d` y cada app necesita su *base
URL* igual a su subpath.

No todo servicio se vuelve ruta: `gluetun` y FlareSolverr solo hablan con los
otros contenedores, así que no reciben `location` ni botón de enlace — Prowlarr
llega a FlareSolverr directo por la red de la stack.

Seerr es lo contrario: no tiene base URL, así que su `location` quita el
prefijo a la entrada y reescribe lo que vuelve — las cabeceras de redirect y
las rutas que escribe en el HTML, con `sub_filter`.

En el Entorno se puede activar el **TLS**: el `nginx.conf` pasa a tener un
`server` en el 443 con `ssl_certificate`, TLSv1.2/1.3 y un bloque en el 80 que
solo redirige al https. El certificado y la clave son rutas del host, indicadas
en el mismo lugar, y entran en el compose como `${TLS_CERT}` y `${TLS_KEY}`,
montadas de solo lectura en `/etc/nginx/certs`. Sin TLS, la stack se queda solo
en el 80. El dominio indicado se vuelve el `server_name` (a falta de él, `_`).

## VPN

Marcar un cliente como "enrutar por gluetun" hace que el servicio use
`network_mode: service:gluetun`, y gluetun entra en la lista de servicios en el
acto, si todavía no está. Pasa a ser la dirección de ese servicio en nginx. Las
credenciales están en el **Editar de gluetun** — proveedor, tipo de túnel,
claves de WireGuard o usuario/contraseña de OpenVPN y los países del servidor —
y salen en el `.env` como `VPN_SERVICE_PROVIDER`, `VPN_TYPE`, `WIREGUARD_*` u
`OPENVPN_*` y `SERVER_COUNTRIES`.

## Idiomas

La interfaz habla portugués (Brasil), inglés y español. El idioma inicial viene
de lo que esté guardado en el `localStorage`, cayendo al del navegador y, por
último, al portugués. La traducción cubre también los comentarios de los
archivos generados — el YAML, el `.env` y el `nginx.conf` salen en el idioma
elegido.

Toda string visible está en el diccionario `I18N`, arriba del `<script>`: una
clave por texto, con valor en string o función cuando depende de algún dato. En
el HTML, los textos estáticos van marcados con `data-i18n` (o `data-i18n-html`,
`data-i18n-ph`, `data-i18n-title`). Añadir un idioma es copiar uno de los
bloques y traducir los valores.

## Wishlist

Lo que todavía no existe, en el orden en que tendría sentido que ocurra:

1. **Un backend que conecte `hubstarr.html` con Docker** — para que la página
   grabe los archivos y levante la stack, en vez de quedarse en el `.zip`.
2. **Configuración automática de las stacks desde el backend** — lo que hoy es
   la **Configuración** en la interfaz (Prowlarr apuntando a cada *arr,
   clientes de descarga con categoría, Media Management) aplicado por la API de
   cada app.
3. **Custom formats y profiles propios de cada stack** — el perfil de calidad y
   los formatos personalizados que tienen sentido para 4K, anime y demás, ya
   ligados a la instancia correcta.
4. **Búsqueda localizada de medios** — poder elegir el idioma de la búsqueda
   (portugués, español, …) para que los *arr encuentren el lanzamiento correcto.

## Estado

La página es un prototipo de interfaz: las opciones de **Configuración** se
guardan en ella y no se vuelven ninguna llamada de API. Los archivos generados
siempre fueron de verdad — basta con descargar el `.zip` y ejecutar
`docker compose up -d` en la carpeta donde se descomprimió.

## Licencia

[GNU General Public License v3.0](LICENSE) o posterior. Úsalo, estúdialo,
modifícalo y redistribúyelo libremente; si distribuyes una versión modificada,
tiene que ir con el código y bajo la misma licencia. Sin garantía — mira las
secciones 15 y 16 del texto.

Los logotipos de los servicios son de sus respectivos proyectos y vienen de
[dashboardicons.com](https://dashboardicons.com); el de nginx es
[Nginx](https://iconscout.com/icons/nginx), de
[Icon 54](https://iconscout.com/contributors/icon-54), en IconScout. La GPL
cubre Hubstarr, no a ellos.

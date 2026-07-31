# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — generador de *arr stack

*[Português (Brasil)](README.md) · [English](README.en.md) · Español*

Prototipo de página única que arma el `docker-compose.yml`, el `.env` y el
`nginx.conf` de una stack multimedia (*arr + clientes de descarga + servidor
multimedia), sin backend y sin dependencias externas.

Abre `arr-stack-prototype.html` en el navegador. Eso es todo — el archivo es
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
- **Múltiples instancias** de Sonarr, Radarr, Lidarr, Bazarr y Prowlarr — basta
  con que el título sea distinto. Sonarr y Radarr reciben además
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Base URL automática**: Sonarr, Radarr, Lidarr y Prowlarr reciben
  `<APP>__SERVER__URLBASE=/<container_name>`, que ya coincide con el subpath
  del nginx. Bazarr no expone esa variable — su base se configura en su propia
  interfaz.
- **API key** en el Entorno: una sola para toda la stack. Sonarr, Radarr,
  Lidarr y Prowlarr salen en el compose con
  `<APP>__AUTH__APIKEY=${STARR_APIKEY}` y el valor queda en el `.env`. La clave
  nace sorteada — 16 bytes en hexadecimal, lo mismo que
  `openssl rand -hex 16` — y el botón "Generar" sortea otra.
- **Aceleración por hardware de Jellyfin**: CPU, Intel o NVIDIA. Intel recibe
  `devices: /dev/dri:/dev/dri`; NVIDIA, la reserva de GPU en `deploy` y las
  variables `NVIDIA_VISIBLE_DEVICES` / `NVIDIA_DRIVER_CAPABILITIES`.
- **FlareSolverr junto a Prowlarr**: en el modal de Prowlarr, una casilla
  marcada por defecto trae FlareSolverr a la stack — es quien resuelve el
  desafío anti-bot de Cloudflare en los indexadores protegidos. Configúralo en
  Prowlarr en *Settings → Indexers → FlareSolverr*, con la URL
  `http://flaresolverr:8191`. La imagen detrás de él es la de
  [Byparr](https://github.com/ThePhaseless/Byparr), reemplazo directo y más
  actual, con la misma API y el mismo puerto.
- **Ayuda por campo** en el Entorno: cada línea tiene un `?` que abre una
  explicación de lo que hace ese valor y de cómo sale en los archivos
  generados.
- **HTTPS opcional**, con el certificado y la clave provenientes del host.
- **Entorno global** (botón arriba): rutas base, PUID/PGID, zona horaria,
  restart policy, API key y TLS. La lista de husos es la IANA entera, que viene
  del propio navegador, y arranca en el huso de la máquina.
- **Descargar** `docker-compose.yml`, `.env` y `nginx/conf.d/starrnet.conf`
  juntos en un `.zip`.
- **Cambiar el idioma** en el selector de arriba: portugués (Brasil), inglés y
  español.

## Docker

Hubstarr en sí solo necesita un navegador; son los archivos que genera los que
necesitan Docker con el plugin Compose. En Linux, el script oficial lo
resuelve:

```sh
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

En macOS y Windows — o en Linux, si prefieres una instalación gestionada con
interfaz gráfica — instala [Docker Desktop][dd].

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
- `BASE_MEDIA` — la biblioteca. Cada *arr monta su propia subcarpeta
  (`series`, `movies`, `music`), Jellyfin monta la base entera y Bazarr sigue
  las subcarpetas de las instancias de Radarr/Sonarr presentes en la stack.
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

## Reverse proxy

nginx es fijo y obligatorio: siempre entra en la stack, no aparece en el
combobox y no se puede eliminar. Es el único contenedor que publica puertos en
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

## Estado

Prototipo de interfaz: el botón "Crear stack" solo simula el despliegue. El
`docker-compose.yml`, el `.env` y el `nginx.conf` generados, esos sí, son de
verdad.

## Licencia

[GNU General Public License v3.0](LICENSE) o posterior. Úsalo, estúdialo,
modifícalo y redistribúyelo libremente; si distribuyes una versión modificada,
tiene que ir con el código y bajo la misma licencia. Sin garantía — mira las
secciones 15 y 16 del texto.

Los logotipos de los servicios son de sus respectivos proyectos y vienen de
[dashboardicons.com](https://dashboardicons.com); la GPL cubre Hubstarr, no a
ellos.

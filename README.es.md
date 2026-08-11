# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — generador de *arr stack

*[Português (Brasil)](README.md) · [English](README.en.md) · Español*

[<img src="docs/badge-licencia.svg" alt="Licencia: GPL-3.0" height="20">](LICENSE)

Prototipo de página única que arma el `docker-compose.yml`, el `.env` y el
`nginx.conf` de una stack multimedia (*arr + clientes de descarga + servidor
multimedia), sin dependencias externas. La página funciona sola, abierta desde
el disco; un [servidor opcional](#servidor-opcional) guarda las stacks en SQLite
y levanta la stack en Docker sin pasar por el `.zip`.

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

![El combobox abierto, mostrando los once servicios disponibles](docs/services.png)

Y el campo **Tema** muestra la captura de la paleta elegida sin salir de la página:

![El modal con la captura del tema hotline de Sonarr, sobre el modal del servicio](docs/theme.png)

## Qué se puede hacer

- **Elegir servicios** en un combobox con logotipos y añadirlos a la stack.
- **Créditos**, en el botón junto al título: un modal con todos los proyectos
  que usa la stack — cada app con enlace a su sitio —, más LinuxServer.io de las
  imágenes, theme.park de los temas y el origen de los iconos.

  ![El modal de Créditos, con los proyectos de la stack y el origen de imágenes, temas e iconos](docs/credits.png)
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
  Radarr, Lidarr, Prowlarr, Bazarr, qBittorrent, SABnzbd y Jellyfin —
  salen con `DOCKER_MODS=ghcr.io/themepark-dev/theme.park:<app>`, el mod que
  aplica el tema en su interfaz. En Sonarr y Radarr el modal trae además un
  campo **Variante**, que se vuelve `TP_ADDON`: *Predeterminado* usa el addon
  oscuro (`sonarr-darker`), *4K* cambia logotipo y favicon por los del addon de 4K
  (`sonarr-4k-logo|sonarr-4k-favicon`) y *Anime* cambia ambos por los de anime
  (`sonarr-anime-logo|sonarr-anime-favicon`) — útil para distinguir las
  instancias de una stack con más de un Sonarr o Radarr. Ambos tienen también un
  campo **Tema**, la paleta en `TP_THEME`: `aquamarine`, `hotline`, `hotpink`,
  `dracula`, `dark`, `organizr` (el predeterminado), `space-gray`, `overseerr` y
  `nord`. Debajo del campo, un enlace muestra la captura de la paleta elegida en
  un modal sobre el del servicio; la imagen viene de la documentación de
  [theme.park](https://docs.theme-park.dev/), una por app.
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
  editan en su modal. El archivo **no se monta**: manda en él el propio
  qBittorrent, y montarlo congelaría todo lo que guarda ahí. Con servidor, el
  **Levantar** escribe esas claves en la conf que creó el app — parando el
  contenedor, haciendo el cambio y volviéndolo a subir, porque él reescribe el
  archivo entero al salir. Sin servidor, copia el contenido de la pestaña a la
  ruta indicada arriba en ella.
- **categories.json de qBittorrent**: junto a la conf sale un segundo archivo
  con las categorías que la **Configuración** dio a cada *arr, ya creadas
  cuando arranca. Cada una recibe su subcarpeta dentro de la ruta de descarga
  — misma partición, así que el *arr sigue haciendo hardlink en vez de copiar.
  Como la conf, no se monta: el **Levantar** suma esas categorías a las que el
  app ya tiene, sin borrar las que creaste en su propia interfaz.
- **HTTPS opcional**, con el certificado y la clave provenientes del host.
- **Configuración** (botón arriba): elegir qué instancias configurará Prowlarr,
  con qué categoría usa cada *arr cada cliente de descarga — `tv-sonarr`,
  `radarr`, `lidarr` en qBittorrent, y las categorías de fábrica de SABnzbd
  (`tv`, `movies`, `music`), todas editables —, además de la
  gestión de descargas completadas en SABnzbd, y las opciones de
  *Media Management* — hardlink, renombrar, permisos, carpetas
  vacías, el bloque **avanzado** (volver a examinar la carpeta, fecha del
  archivo, papelera y su limpieza, importar archivos extra, comprobación de
  espacio libre) y la
  nomenclatura completa de cada app (*Episode Naming*,
  *Nomenclatura de película*, *Nomenclatura de pista*: caracteres ilegales, dos
  puntos, varios episodios y todos los formatos de archivo y de carpeta) —,
  separadas por familia: Sonarr, Radarr y Lidarr. Los formatos de episodio y de
  película ya vienen con los de [TRaSH Guides](https://trash-guides.info), en la
  variante de Jellyfin con el id de TMDb. Los permisos abren los campos
  de `chmod` y `chown`, y en Lidarr la casilla de nombre existente es la que
  trae los formatos de pista y la carpeta del álbum.

  ![El modal de Configuración, en la nomenclatura de episodio de Sonarr](docs/config.png)

  Con más de un Sonarr en la
  stack, cada formato de episodio — **estándar**, **diario** y **anime** — trae
  la lista de las instancias que lo reciben: se puede mandar el formato de anime
  solo al *Sonarr [Anime]*, por ejemplo. Al menos una instancia es obligatoria,
  porque el campo es obligatorio en el app; la que quede fuera conserva el
  formato que ya tiene, en vez de perderlo. Por ahora las
  opciones, las tres partes llegan a las apps por el **Aplicar en la stack**.
- **Entorno global** (botón arriba): rutas base, PUID/PGID, zona horaria,
  restart policy, API key y TLS. La lista de husos es la IANA entera, que viene
  del propio navegador, y arranca en el huso de la máquina.
- **Descargar** `docker-compose.yml`, `.env` y `nginx/conf.d/starrnet.conf`
  juntos en un `.zip` — el botón se queda en la barra mientras no hay servidor;
  con él, quien graba los archivos es **Levantar**.
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

## Servidor opcional

La página sigue siendo el producto: es ella la que genera los archivos, y
abierta desde el disco funciona entera, con el `.zip` y nada más. El servidor de
`backend/` añade lo que el navegador no alcanza solo — **guardar las stacks
entre sesiones**, grabar los archivos en disco y levantar la stack en Docker;
con él en marcha, el botón del `.zip` sale de la barra, porque **Levantar** graba
los mismos archivos. Nunca genera contenido: recibe ya listo lo que armaron los
generadores de la página.

Compilarlo y ejecutarlo necesita [Rust](https://rustup.rs):

```sh
cd backend
cargo run --release
```

Abre `http://127.0.0.1:7878`. La página es la misma — servida por el binario,
que la trae embebida — con dos cosas más: el distintivo **servidor** arriba y,
en los archivos generados, los botones **Levantar** y **Derribar**. Al abrirse
le pregunta al servidor si hay `docker compose` ahí; si no lo hay, avisa y abre
ya el bloque **¿Necesitas instalar Docker?**. Sin `docker compose` ahí,
**Levantar** y **Derribar** quedan deshabilitados, con la explicación en la
ayuda de los botones.

| Opción     | Predeterminado            | Qué es                                       |
| ---------- | ------------------------- | -------------------------------------------- |
| `--addr`   | `127.0.0.1:7878`          | dirección en la que atiende el servidor       |
| `--dir`    | `./stack`                 | carpeta en la que se graban los archivos      |
| `--db`     | `~/.hubstarr/hubstarr.db` | base en la que se guarda la stack             |
| `--docker` | `docker`                  | comando de docker, para quien usa podman      |

La stack vive en la base con sus instancias, el Entorno y la Configuración en
tablas propias — el estado de la página, normalizado, y no un blob de JSON. Es
una sola, la de la carpeta del `--dir`: para mantener otra, apunta el `--dir`
y el `--db` a otro sitio.

Una base de una versión anterior, que guardaba varias stacks, se convierte en
la primera apertura; la stack más antigua es la que se queda, y el servidor
indica en la salida en qué carpeta escribía cada una de las otras, para que no
pierdas de vista sus archivos.

Con servidor, las capturas de las paletas del campo **Tema** también pasan por
él: la primera visita a cada una sale a la documentación de theme.park y las
siguientes salen del disco, de una carpeta `shots/` junto a la base. El
repositorio no redistribuye capturas de nadie — la caché guarda lo que abriste
tú, y se poda sola al pasar de 64 MB, de las más antiguas a las más nuevas.
Borrar la carpeta a mano no rompe nada: cuesta una búsqueda más. Abierta desde
el disco, sin servidor, la página sigue buscando cada captura directamente en
su documentación.

El **Levantar** ya deja la stack configurada: en cuanto los apps responden, el
servidor registra cada cliente de descarga en cada *arr **y en el propio
Prowlarr** — que tiene su Settings → Download Clients —, cada *arr marcado en
Settings → Apps de Prowlarr, y el *Media Management* con la nomenclatura de cada
familia, todo por su API, mostrando lo que pasó. El botón **Aplicar en la
stack**, en el modal de la Configuración, hace lo mismo sin levantar nada — es
el camino para volver a aplicar tras cambiar las opciones.

Con **FlareSolverr** en la stack, Prowlarr también recibe su proxy en
*Settings → Indexers → Indexer Proxies*, con la etiqueta **flaresolverr**. Solo
queda poner esa etiqueta en los indexadores que lo necesitan — así decide
Prowlarr cuándo usar el resolvedor.

En Prowlarr, el Settings → Download Clients recibe **un registro por cliente**,
todos en la categoría `prowlarr`: lo que él captura es suelto, no vino de un
*arr, así que queda junto y aparte de lo que baja cada instancia. Y las
categorías pasan a existir dentro del cliente — la de cada *arr y la de
Prowlarr: por el `categories.json` en qBittorrent, y creadas por su propia API
en SABnzbd, cada una con la carpeta del mismo nombre dentro del directorio de
descargas completadas. Los apps se alcanzan por el nginx, en el puerto que
publica en el host. Aplicar de nuevo no duplica — el cliente se busca por el
nombre y se actualiza en su sitio — y un app que todavía no arrancó es una línea
del log en vez de interrumpir el resto. SABnzbd necesita su clave de API, la que
genera el propio app en el primer arranque: cópiala de *Config → General* y
pégala en el campo **API key** de su modal.

> [!WARNING]
> El servidor ejecuta `docker compose` y escribe en disco: no lo expongas a una
> red en la que no confíes. De forma predeterminada solo atiende en `127.0.0.1`.

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
| Lidarr       | `8686`         | FlareSolverr | `8191`         |
| Prowlarr     | `9696`         | Gluetun      | `8000`         |
| Bazarr       | `6767`         | Nginx        | `80` / `443`   |
| qBittorrent  | `8181`         |              |                |
| SABnzbd      | `8080`         |              |                |

Es ese puerto el que aparece en la lista, junto al subpath, y el que usa el
`proxy_pass` de nginx. El de qBittorrent es la excepción que no viene de
fábrica: escucharía en el 8080, el mismo del SABnzbd, así que sale con
`WEBUI_PORT=8181` en el compose y el `WebUI\Port` correspondiente en la conf
generada. Nginx es el único con dos, y son los de dentro del contenedor — los
publicados en el host salen de su propio modal.

## Reverse proxy

nginx es fijo y obligatorio: siempre entra en la stack, no aparece en el
combobox y no se puede eliminar. Aparte de él, solo **Seerr** publica un puerto
en el host; todos los demás se quedan solo en la red `starrnet`, alcanzados por
nginx en `nombre-del-contenedor:puerto-interno`. Lo que se enruta por la VPN
responde en `gluetun`, que es quien tiene la red.

Los dos puertos del host están en el **Editar** de la línea de nginx: 80 y 443
por defecto, pero se puede publicar en 8080 y 8443, por ejemplo, si algo ya
ocupa los privilegiados. Salen como `HTTP_PORT` y `HTTPS_PORT` en el `.env`;
dentro del contenedor nginx sigue escuchando en 80 y 443. Los enlaces copiados
y la redirección al https ya llevan el puerto elegido.

La pestaña **nginx.conf** genera la configuración correspondiente, enrutando
por subpath (`/sonarr`, `/radarr`…), un `location` por servicio. El archivo se
monta en `${BASE_CONFIG}/nginx/conf.d` y cada app necesita su *base URL* igual a
su subpath. Ningún servicio se queda en la raíz: la `/` de la stack no tiene
`location`, así que se entra por el subpath de cada app.

No todo servicio se vuelve ruta: `gluetun` y FlareSolverr solo hablan con los
otros contenedores, así que no reciben `location` ni botón de enlace — Prowlarr
llega a FlareSolverr directo por la red de la stack.

Seerr se queda fuera del proxy por otro motivo: no tiene base URL, y una app
sin ella no vive en un subpath. En vez de ruta, **publica su puerto en el
host** — 5055 por defecto, editable en su propio modal, que sale en el compose
como `ports` y en el `.env` como `SEERR_PORT`. Su enlace apunta a ese puerto,
por `http://`: sin el proxy delante, el TLS de la stack no lo cubre, y el
puerto tiene que estar libre en la máquina.

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

Lo que todavía no existe, en el orden en que tendría sentido que ocurra. Los
hitos son versiones, no fechas: cada uno solo empieza después del anterior, porque
depende de él. Hoy el repositorio está en **v0.3** — la página, el servidor
opcional que guarda las stacks y las levanta en Docker, y la Configuración
aplicada en las apps.

| Hito     | Entrega                                              | Se cierra cuando                                                                        |
| -------- | ---------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| ~~**v0.2**~~ | ~~Un backend que conecte `hubstarr.html` con Docker~~ | ✅ la página graba los archivos y levanta la stack sin pasar por el `.zip`              |
| ~~**v0.3**~~ | ~~Configuración automática de las stacks desde el backend~~ | ✅ Prowlarr, clientes de descarga y Media Management salen de la interfaz y son llamadas de API |
| **v0.4** | Custom formats y profiles propios de cada stack       | la instancia de 4K, la de anime y las demás nacen con su perfil de calidad                 |
| **v0.5** | Compatibilidad con TRaSH Guides                       | quality definitions, scores de custom format y el resto de las recomendaciones de la guía ya listas |
| **v0.6** | Búsqueda localizada de medios                         | se puede elegir el idioma de la búsqueda y los *arr encuentran el lanzamiento correcto     |

## Estado

La página es un prototipo de interfaz, pero la **Configuración** ya no es solo
interfaz: las opciones se guardan — en ella, y en la base cuando hay servidor —
y, con servidor y la stack en marcha, sus tres partes se vuelven llamadas de API
por el **Aplicar en la stack**. Los archivos generados siempre
fueron de verdad — basta con descargar el `.zip` y ejecutar
`docker compose up -d` en la carpeta donde se descomprimió, o dejar que el
servidor haga las dos cosas.

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

Los temas de las apps son de [theme.park](https://theme-park.dev/), un proyecto
aparte, también bajo GPL-3.0: de él vienen la imagen que el compose usa en
`TP_THEME`/`TP_ADDON`, las paletas del campo **Tema**, las capturas que muestra
Hubstarr y los logotipos de las variantes 4K y Anime de Sonarr y Radarr.

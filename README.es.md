# <img src="docs/logo.svg" width="26" align="top" alt=""> HUBSTARR - Generador y configurador de *arr stack

*[🇧🇷 Português (Brasil)](README.pt-BR.md) · [🇬🇧 English](README.md) · 🇪🇸 Español*

[<img src="docs/badge-licencia.svg" alt="Licencia: GPL-3.0" height="20">](LICENSE)

Prototipo que arma y levanta una stack multimedia (*arr + clientes de descarga
+ servidor multimedia): el `docker-compose.yml`, el `.env` y el `nginx.conf`, y
después las apps configuradas unas en otras. El [servidor](#servidor) es
Hubstarr — guarda la stack en SQLite, graba los archivos, la levanta en Docker
y configura las apps por la API de cada una.

La interfaz es una **página única sin dependencia externa**, que el servidor
trae incrustada. También abre sola, directamente desde el disco, y ahí genera
los archivos en un `.zip` — pero solo eso: sin servidor no hay stack en marcha,
ni contraseña de qBittorrent, ni base URL de Jellyfin, ni perfil del TRaSH
Guides. Es el modo de quien quiere solo los archivos.

> [!WARNING]
> **Prototipo.** Hubstarr no fue diseñado para uso en producción: los archivos
> que genera son un punto de partida, sin endurecimiento de seguridad, copias
> de seguridad ni monitorización. Revísalo todo — contraseñas, puertos,
> certificados y permisos — antes de exponer la stack a cualquier red que no
> sea la tuya.

## Video

Un recorrido completo, desde compilar el servidor hasta importar una película
descargada — añadir servicios, configurar las instancias, levantar la stack,
descargar con qBittorrent e importar en Radarr:

https://github.com/user-attachments/assets/bfab344a-0547-4518-bdd3-382e6ef12307

También hay un recorrido rápido por la interfaz (~43 s), sin levantar nada de
verdad — tema, idioma, añadir servicios, editar una instancia, reordenar con
el teclado y los modales de Entorno, nginx, Créditos y Configuración:
[tour por la página](docs/demo-tour.mp4).

## Lo que te deja listo

Generar los archivos es la mitad fácil. La otra es la que se hace a mano
después, app por app — y es la que el **Levantar** hace solo, por la API de cada
una. El botón **Aplicar en la stack** vuelve a aplicar todo esto sin levantar
nada, y solo aparece con la stack en marcha:

- **La configuración básica de cada app**: la base URL igual al subpath de
  nginx, la misma API key en toda la stack, zona horaria, PUID/PGID y las
  carpetas del compose. En los *arr, el *Media Management* completo — hardlink,
  renombrar, permisos, papelera, espacio libre — y la nomenclatura de episodio,
  película y pista ya en los formatos del
  [TRaSH Guides](https://trash-guides.info).
- **Clientes de descarga conectados**: qBittorrent y SABnzbd registrados en cada
  Sonarr, Radarr y Lidarr **y en el propio Prowlarr**, cada uno con la categoría
  que elegiste — y las categorías creadas dentro del cliente, cada una con su
  carpeta. Prowlarr recibe además cada *arr para sincronizar, con las categorías
  por familia, y FlareSolverr como proxy de indexadores.
- **Puntos de importación listos**: la carpeta raíz de cada *arr, en la ruta que
  ve el contenedor (`/data/tv`, `/data/movies`, `/data/music`). Sin ella la
  primera serie se detiene en un *You must add a root folder* — y la ruta escrita
  a mano suele ser la del host, que la app acepta y después no encuentra.
- **Perfiles de calidad del TRaSH Guides**, por instancia: cada preset trae el
  trío que la guía recomienda junto — el perfil, los custom formats **con sus
  scores** y la definición de tamaño de los archivos. Es así como la instancia de
  4K deja de ser igual a la de 1080p. Quien los aplica es
  [Configarr](https://configarr.de), con las plantillas de Recyclarr, y la guía
  sigue siendo de ellos: Hubstarr elige, no reimplementa.
- **Jellyfin preconfigurado**: el asistente inicial (idioma de la interfaz,
  administrador, acceso remoto) y una biblioteca por instancia de *arr, con el
  tipo correcto y la ruta **de dentro del contenedor** — la misma que el *arr
  recibe como carpeta raíz, que es lo que hace que la biblioteca liste justo lo
  que importó.

Sin servidor nada de esto ocurre: el `.zip` lleva los archivos y el resto lo
haces tú, en la interfaz de cada app. Es la diferencia entre una stack en marcha
y una stack lista para usar.

Ejecuta el servidor y abre su dirección:

```sh
cd backend
cargo run --release      # http://127.0.0.1:7878
```

El **Entorno** se abre junto: de ahí salen las rutas base que usa todo lo demás.
Si lo cierras, sigue en el botón de arriba. ¿Solo los archivos, sin levantar
nada? Abre el `hubstarr.html` directamente en el navegador — es autocontenido,
con los logotipos incrustados como data URI.

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
- **Copiar el enlace** de cada servicio, ya con el esquema, la dirección y el
  subpath por los que nginx lo va a atender. La dirección es el dominio del
  Entorno cuando lo hay; sin él, es la misma por la que abriste la página —
  quien llega por la IP de la LAN recibe los enlaces en esa IP. La única
  dirección que nunca se usa es `localhost`: hoy suele resolver a `::1`, y el
  puerto que Docker publicó solo en IPv4 no tiene a nadie del otro lado, así que
  los enlaces llevan `127.0.0.1`.
- **Ordenar la lista arrastrando**: tome la fila del servicio en cualquier
  punto y muévala; el orden que usted deje es el orden en que los servicios
  salen en el `docker-compose.yml` y en el `.env` — con servidor, queda
  guardado. Empezar el gesto en el Link, el Editar, el Eliminar o el punto de
  estado sigue haciendo clic en ellos. Las flechas ↑ ↓ hacen lo mismo con el
  asa (`⁙`) enfocada, para quien no usa el ratón. nginx es fila fija y no se
  mueve. El orden no es el orden de arranque: de eso se encarga el
  `depends_on` del compose.
- **Excluir borra la configuración**: sacar un servicio de la lista se lleva su
  contenedor y su carpeta de configuración — la base de la app, el historial,
  los indexadores —, así que el botón pide confirmación: el primer clic la arma,
  el segundo borra. Solo él lo hace; la lista que el
  servidor reconcilia quita la fila y deja contenedor y carpeta donde están.
  Un contenedor de otra stack que tenga el mismo nombre no se toca — y, como es
  de otro dueño, la carpeta también queda: el servicio sale de la lista y nada
  en el disco se pierde. Sin servidor
  no hay nada que borrar: la página abierta del disco no alcanza tu sistema de
  archivos.
- **Aviso de conflicto**: dos instancias apuntadas a la misma carpeta se pisan
  al importar, así que la lista lo avisa en rojo, con los nombres y la ruta.
  Jellyfin, que monta la biblioteca entera, y Bazarr, que sigue a las otras,
  quedan fuera de la comprobación.
- **Etiquetas en la fila del servicio**, un color por tipo: la variante del
  logotipo, el tema de la interfaz, la salida por la VPN, la aceleración por
  GPU, la dirección en la stack y las carpetas de configuración y multimedia,
  completas. Debajo de la lista, una leyenda dice
  qué marca cada color — desaparece con la lista vacía.
- **Múltiples instancias** de Sonarr, Radarr, Lidarr, Bazarr y Prowlarr — basta
  con que el título sea distinto. Sonarr y Radarr reciben además
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Base URL automática**: Sonarr, Radarr, Lidarr y Prowlarr reciben
  `<APP>__SERVER__URLBASE=/<container_name>`, que ya coincide con el subpath
  del nginx. Bazarr no expone esa variable — su base se configura en su propia
  interfaz.
- **Nombre del proyecto** en el Entorno: es lo que separa esta stack de las
  otras de la misma máquina. Nombra el proyecto de compose y entra en el nombre
  de cada contenedor (`hubstarr-sonarr`) y en el de la red — los tres nombres
  que docker ve desde fuera. Con ellos fijos, desplegar en una máquina que ya
  tuviera un `sonarr` tomaba el contenedor en ejecución, y un `docker compose
  down` en una carpeta llamada `stack` se llevaba lo que otra carpeta del mismo
  nombre había creado. La ruta de nginx (`/sonarr`), la carpeta de
  configuración y lo guardado no cambian con él. Cambiarlo recrea los
  contenedores en el próximo despliegue; la configuración de las apps queda
  donde está.
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
  [theme.park](https://docs.theme-park.dev/), una por app. En la lista, la fila
  de quien tiene tema trae una etiqueta con el elegido, al lado de la variante.
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
- **network.xml de Jellyfin**: con él en la stack, el `BaseUrl` en el subpath
  de nginx y `nginx` en `KnownProxies` — sin lo primero la interfaz arma los
  enlaces en la raíz y el subpath responde 404, sin lo segundo registra la IP
  del proxy en vez de la de quien pidió. El archivo **no se monta**: es de
  Jellyfin, que migra su configuración de red al arrancar, y montarlo congelaría
  lo que guarda ahí. Con servidor, el **Levantar** espera a que el app cree el
  archivo, comprueba si la base URL está, escribe la que falta y reinicia solo
  ese contenedor — las demás claves quedan como estaban. Sin servidor, sale en
  el `.zip`, en la ruta donde el app lo lee (`/config/network.xml`, al lado del
  `system.xml`).
- **qBittorrent.conf lista**: cuando está en la stack, Hubstarr arma su
  configuración inicial — rutas iguales a las del compose, ajustes de
  proxy inverso y las credenciales en el formato del propio qBittorrent 5.2: la
  contraseña en PBKDF2-SHA512 y la API key `qbt_` más 28 caracteres, derivada
  de la `${STARR_APIKEY}` de la stack — la conf la lee qBittorrent, no compose,
  así que la variable no se expandiría ahí. Usuario, contraseña y clave se
  editan en su modal — y es la **API key** la que usan los *arr para hablar con
  él, no la contraseña: no caduca cuando cambia la contraseña de la interfaz. El archivo **no se monta**: manda en él el propio
  qBittorrent, y montarlo congelaría todo lo que guarda ahí. Con servidor, el
  **Levantar** escribe esas claves en la conf que creó el app — parando el
  contenedor, haciendo el cambio y volviéndolo a subir, porque él reescribe el
  archivo entero al salir. Sin servidor, sale en el `.zip`, en la ruta donde el
  app lo lee, para copiarlo de ahí.
- **Preferencias de qBittorrent por la API**: con las apps en marcha, el
  **Levantar** (y el **Aplicar en la stack**) además le ajusta la **gestión
  automática de torrents** — activada, y siguiendo la categoría cuando cambia,
  que es lo que manda el torrent a la carpeta correcta —, la **carpeta de
  descarga** (la subcarpeta de su modal, en la ruta que ve el contenedor) y el
  **usuario y la contraseña** de la interfaz. No es repetición de la conf:
  aquella es lo que lee al nacer, esta es la misma decisión aplicada a un
  qBittorrent que ya existe — y la gestión automática la conf ni la cubre. Su
  **API key** se respeta: si el app ya tiene una, Hubstarr **no la cambia** — es
  la que sus clientes ya hablan — y los *arr pasan a registrarse con la clave del
  app. La nuestra solo entra cuando él todavía no tiene ninguna, que es el primer
  arranque. Quien la graba es la conf: qBittorrent acepta la propiedad por la API
  y la ignora. Esas llamadas entran por la **API key**, y no por la contraseña:
  es la misma vuelta que cambia la contraseña, y entrar con la que está por ser
  sustituida funcionaría una vez y fallaría en la siguiente. La sesión del
  `auth/login` queda como reserva, para el app que no conoce la clave.
- **sabnzbd.ini de SABnzbd**: su **API key** es la misma de la stack — el campo
  del modal muestra la que vale, y el **Generar** crea otra por el mismo método
  (16 bytes en hexadecimal), que entonces va al `.env` —, y las carpetas de
  **descarga en progreso** y **descarga completa** se vuelven el `download_dir` y
  el `complete_dir`. Con ellas va el `url_base`, el subpath en que lo sirve
  nginx — sin él SABnzbd arma los enlaces en la raíz y se rompe detrás del
  proxy. Las cuatro claves se escriben en el `sabnzbd.ini` que creó el propio
  app, después de levantar la stack, como en qBittorrent.
- **Categorías de qBittorrent**: las que la **Configuración** dio a cada *arr,
  cada una con su subcarpeta dentro de la ruta de descarga — misma partición,
  así que el *arr sigue haciendo hardlink en vez de copiar. Con servidor se
  crean **por la API del app**, con él en marcha: la que ya existe recibe la
  carpeta actualizada, y ninguna se elimina — puede haber un torrent apuntado a
  ella. Sin servidor, las mismas categorías salen en el `.zip` como
  `categories.json`, para copiarlo a su carpeta de configuración antes del
  primer arranque.
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
  stack, cada formato suyo — los tres de episodio (**estándar**, **diario** y
  **anime**) y las tres carpetas (**serie**, **temporada** y **especiales**) —
  trae la lista de las instancias que lo reciben: se puede mandar el formato de
  anime solo al *Sonarr [Anime]*, por ejemplo. De fábrica todas las instancias reciben
  todos los formatos; al menos una es obligatoria en el formato predeterminado,
  porque el campo es obligatorio en el app, y la que desmarques conserva el
  formato que ya tiene, en vez de perderlo. Las tres partes de la Configuración
  llegan a las apps por el **Levantar** y por el **Aplicar en la stack**.
- **Carpetas raíz listas**: cada Sonarr, Radarr y Lidarr recibe la carpeta que
  el compose le monta — `/data/tv`, `/data/movies`, `/data/music` —, en la ruta
  que ve el contenedor. Sin eso la primera serie o película se detiene en un
  *You must add a root folder*, y la ruta escrita a mano suele ser la del host,
  que la app acepta y después no encuentra. Lo que ya esté ahí se queda: quitar
  una carpeta raíz se lleva la biblioteca con ella.
- **Jellyfin listo para usar**: con él en la stack, el **Levantar** pasa por el
  asistente inicial — la interfaz en el idioma de la página, el administrador de
  su modal y el acceso remoto — y crea una biblioteca por instancia de Sonarr,
  Radarr y Lidarr, con el tipo correcto (series, películas, música) y la ruta **de
  dentro del contenedor**, la misma que el *arr recibe como carpeta raíz: esa
  igualdad es la que hace que la biblioteca liste justo lo que el *arr importó.
  Las carpetas sueltas de su modal entran como bibliotecas mixtas. Usuario y
  contraseña en blanco significan "no toques el asistente": las bibliotecas
  entran igual, pero él queda abierto para que termines en el navegador —
  concluirlo sin administrador dejaría a Jellyfin sin ninguna cuenta con la que
  entrar. En un Jellyfin cuyo asistente ya se completó, el usuario y la
  contraseña del modal son los que le dan a Hubstarr el token para crear las
  bibliotecas. Una biblioteca que ya existe no se toca, y ninguna se elimina.
- **Perfiles de calidad y custom formats** del [TRaSH Guides](https://trash-guides.info),
  por instancia: cada Sonarr y cada Radarr de la stack elige los perfiles de la
  guía que quiere — **HD (1080p)**, **4K (2160p)**, **Remux 4K**, **Anime** —, y
  así es como la instancia de 4K deja de ser igual a la de 1080p. Cada uno trae
  el trío que la guía recomienda junto: el perfil, los custom formats que lo
  puntúan y la definición de tamaño de los archivos. Hay además un campo para
  escribir otros templates de Recyclarr a mano.

  Quien los aplica es [Configarr](https://configarr.de), y **no es un servicio
  de la stack**: la página genera su `config.yml` y su `secrets.yml`, y el
  servidor lo ejecuta con un `docker run --rm` aparte después de que las apps
  responden, en el **Levantar** y en el **Aplicar en la stack**. Corre una vez y
  sale — en un `up -d` arrancaría antes de que las apps estuvieran de pie. Entra
  en la red de la stack para alcanzar cada *arr por el nombre del contenedor, con
  el PUID/PGID del Entorno, y guarda el caché del TRaSH y de Recyclarr en
  `<config>/configarr/repos`. La sección existe aunque él no esté en ningún
  lado: lo que describe es la stack, no un contenedor. Lidarr queda fuera: no
  hay template para él.
- **Entorno global** (botón arriba): rutas base, PUID/PGID — que, con servidor,
  ya vienen con el usuario y el grupo con que se ejecuta, el mismo que crea las
  carpetas de la stack —, zona horaria,
  restart policy, API key y TLS. La lista de husos es la IANA entera, que viene
  del propio navegador, y arranca en el huso de la máquina.
- **Descargar** `docker-compose.yml`, `.env` y `nginx.conf`
  juntos en un `.zip` — el botón se queda en la barra mientras no hay servidor;
  con él, quien graba los archivos es **Levantar**.
- **Paso a paso en la primera visita**: un recorrido por la interfaz que enciende
  cada área y dice qué hace, con **Saltar** en cualquier momento. Terminado o
  saltado, no vuelve a aparecer — la marca queda en el navegador.
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

## Servidor

El servidor de `backend/` es lo que hace que la stack exista: **guarda las
stacks entre sesiones**, graba los archivos en disco, lo levanta todo en Docker
y configura las apps unas en otras — los clientes de descarga en cada *arr, los
*arr en Prowlarr, el Media Management, la nomenclatura y los perfiles del TRaSH
Guides. Con él en marcha, el botón del `.zip` sale de la barra, porque
**Levantar** graba los mismos archivos.

Lo que **no** hace es generar contenido: recibe ya listo lo que armaron los
generadores de la página. Esa división es la que mantiene los generadores en un
solo lugar, y es la que deja a la página abrir sola desde el disco cuando
alguien quiere solo los archivos.

Compilarlo y ejecutarlo necesita [Rust](https://rustup.rs):

```sh
cd backend
cargo run --release
```

Abre `http://127.0.0.1:7878`. La página es la misma — servida por el binario,
que la trae embebida — con dos cosas más: el distintivo **servidor** arriba y,
en los archivos generados, los botones **Levantar** y **Derribar**. Al abrirse
le pregunta al servidor si hay `docker compose` —o `podman compose`, que busca
solo cuando docker no responde— ahí; si no lo hay, avisa y abre
ya el bloque **¿Necesitas instalar Docker?**. Sin `docker compose` ahí,
**Levantar** y **Derribar** quedan deshabilitados, con la explicación en la
ayuda de los botones.

| Opción     | Predeterminado            | Qué es                                       |
| ---------- | ------------------------- | -------------------------------------------- |
| `--addr`   | `127.0.0.1:7878`          | dirección en la que atiende el servidor       |
| `--dir`    | `./stack`                 | carpeta en la que se graban los archivos      |
| `--db`     | `~/.hubstarr/hubstarr.db` | base en la que se guarda la stack             |
| `--docker` | `docker`, o `podman`      | comando del compose; sin la opción, vale el primero de los dos que responda |
| `-v`       | apagado                   | dice el paso a paso: archivos, base de datos y llamadas de API |

El servidor escribe lo que hace en su salida y en un `server.log` junto a la
base (`~/.hubstarr/server.log` con el `--db` de fábrica): el arranque, el
motor de contenedores elegido y cada guardado de estado que llega de la página
— cuántos servicios vinieron y cuáles salieron de la stack. El archivo añade,
nunca reescribe, y es donde mirar cuando la stack cambió y no se sabe por qué.

Con **`-v`** cuenta el paso a paso, en los dos sitios: cada archivo grabado
(incluidas las claves escritas en la configuración de cada app), cada fila
tocada en la base — instancia, Entorno, Configuración, la lista de servicios — y
**cada llamada a las APIs de las apps**, con método, ruta y estado. Es el modo
de descubrir por qué una conexión no pasó; encendido siempre, ahogaría las
líneas que importan, porque una vuelta del *Aplicar* son decenas de llamadas.
Contraseñas y claves de API nunca salen en el log: del Entorno van solo los
nombres de los campos, y de las URL sale la parte antes del `?`.

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

Antes de levantar nada, el servidor **crea las carpetas** que esperan los
volúmenes del compose — las de configuración, las de medios y las de descargas.
Sin eso las crea Docker, como `root`, y el app no consigue escribir en ellas. Una
ruta que ya existe y no es carpeta detiene el Levantar ahí mismo, con su nombre
en el log.

El **Levantar** ya deja la stack configurada: en cuanto cada app **termina de
iniciar** — el servidor espera a que responda `system/status`, y no solo a que
el puerto atienda —, el
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
Prowlarr: por la API de cada app — `torrents/createCategory` en qBittorrent (la
que ya existe recibe la carpeta actualizada en vez de fallar) y
`set_config&section=categories` en SABnzbd —, cada una con la carpeta del mismo
nombre dentro del directorio de descargas completadas. Los apps se alcanzan por el nginx, en el puerto que
publica en el host. Aplicar de nuevo no duplica — el cliente se busca por el
nombre y se actualiza en su sitio — y un app que todavía no arrancó es una línea
del log en vez de interrumpir el resto. Una llamada que **no llega** a la app se
repite diez veces, con cinco segundos entre ellas: vale para fallos de acceso —
nadie escuchando, o el 502 de nginx mientras el contenedor todavía arranca — y
no para la app rechazando el pedido, que respondería lo mismo diez veces. Con
`-v`, cada intento aparece en el log. SABnzbd necesita su clave de API, la que
genera el propio app en el primer arranque: cópiala de *Config → General* y
pégala en el campo **API key** de su modal.

> [!WARNING]
> El servidor ejecuta `docker compose` y escribe en disco: no lo expongas a una
> red en la que no confíes. De forma predeterminada solo atiende en `127.0.0.1`.

## Convenciones generadas

El nombre del proyecto de compose, el `container_name` de cada contenedor y el
nombre de la red llevan el **Nombre del proyecto** del Entorno (`hubstarr` por
defecto — ver el ítem de arriba): es lo que separa esta stack de otra en la
misma máquina. Lo que no lleva ese prefijo es el resto: el título de cada
instancia se convierte en un slug (minúsculas, sin acentos, espacios como
guiones) usado, siempre de la misma forma, como clave del servicio en el
compose, subpath del nginx y carpeta de config:

| Título          | clave del servicio / config | container_name (proyecto `hubstarr`) |
| --------------- | ----------------------------- | -------------------------------------- |
| `Radarr`        | `radarr`                      | `hubstarr-radarr`                      |
| `Radarr [UHD]`  | `radarr-uhd`                  | `hubstarr-radarr-uhd`                  |

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
ocupa los privilegiados. Salen como `HTTP_PORT` y `HTTPS_PORT` en el `.env`. El
**443 solo se publica con el "Servir HTTPS" activado**: sin él el `nginx.conf`
no tiene ningún `server` escuchando ahí, y publicarlo sería ocupar el puerto de
la máquina sin nada del otro lado — e impedir que la stack levante donde algo ya
lo usa. Sin TLS, ni el puerto ni el `HTTPS_PORT` salen. Dentro del contenedor
nginx sigue escuchando en 80 y 443. Los enlaces copiados
y la redirección al https ya llevan el puerto elegido.

La pestaña **nginx.conf** genera la configuración correspondiente, enrutando
por subpath (`/sonarr`, `/radarr`…), un `location` por servicio. El archivo se
monta en el `/etc/nginx/conf.d/default.conf` del contenedor — encima del archivo
que viene en la imagen, que declara `server_name localhost` y respondería
`http://localhost` con un 404 pelado de nginx —, desde el `nginx.conf`
de la carpeta de la stack — la conf se genera con el compose y vive junto a él,
no en el `BASE_CONFIG`. Con servidor la ruta sale completa, porque es él quien
sabe dónde vive la stack; sin servidor sale como `./nginx.conf`, relativa a la
carpeta desde donde se ejecute el compose. Cada app necesita su *base URL* igual
a
su subpath. Ningún servicio se queda en la raíz: la `/` de la stack no tiene
`location`, así que se entra por el subpath de cada app.

No todo servicio se vuelve ruta: `gluetun` y FlareSolverr solo hablan con los
otros contenedores, así que no reciben `location` ni botón de enlace — Prowlarr
llega a FlareSolverr directo por la red de la stack.

**Seerr** se queda fuera del proxy: no tiene base URL configurable, y una app
sin ella no vive en un subpath. En vez de ruta, **publica su puerto en el
host** — 5055 por defecto, editable en su propio modal, que sale en el compose
como `ports` y en el `.env` como `SEERR_PORT`. Su enlace apunta a ese puerto,
por `http://`: sin el proxy delante, el TLS de la stack no lo cubre, y el puerto
tiene que estar libre en la máquina. Lo que se enruta por la VPN publica en
`gluetun`, que es quien tiene la red.

**qBittorrent** tampoco tiene base URL configurable, pero sigue detrás del
proxy: su ruta es la que **quita el prefijo** por el camino, así que responde en
la raíz, sin saber que existe un `/qbittorrent`. El bloque trae un `rewrite` que
corta el prefijo, un `resolver 127.0.0.11` — el DNS de Docker, porque el
`proxy_pass` con variable resuelve el nombre en cada pedido — y un
`location = /qbittorrent` que redirige a la barra final, con
`absolute_redirect off` para que el puerto del host no se pierda por el camino.
Los estáticos de su interfaz son relativos, así que acompañan al prefijo; y la
conf que escribe Hubstarr ya trae las claves de proxy inverso que su **API**
exige — sin ellas la interfaz abre y la API responde 403, que es lo que
consultan los *arr.

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

La interfaz habla inglés, portugués (Brasil) y español, y el idioma se elige en
el selector del encabezado — o en el paso a paso de la primera visita, que trae
el mismo selector. Abre en **inglés**; lo que se elija queda guardado
en el `localStorage` y vale desde entonces. La traducción cubre también los comentarios de los
archivos generados — el YAML, el `.env` y el `nginx.conf` salen en el idioma
elegido.

Toda string visible está en el diccionario `I18N`, arriba del `<script>`: una
clave por texto, con valor en string o función cuando depende de algún dato. En
el HTML, los textos estáticos van marcados con `data-i18n` (o `data-i18n-html`,
`data-i18n-ph`, `data-i18n-title`). Añadir un idioma es copiar uno de los
bloques y traducir los valores.

### Idioma de la búsqueda

El **idioma de la interfaz** y el **idioma de la búsqueda** son dos cosas
distintas, y el segundo vive en el Entorno. Es el que decide en qué idioma la
stack busca y muestra los títulos, y baja a cuatro lugares:

| Dónde | Qué recibe |
| ----- | ---------- |
| **Radarr** | el *Movie Info Language* de `Settings → UI`, aplicado por el Aplicar en la stack |
| **Jellyfin** | el idioma preferido de metadatos del servidor y de las bibliotecas que nazcan de ahí en adelante |
| **Bazarr** | un perfil de subtítulos en ese idioma, puesto como predeterminado de serie y de película |
| **Configarr** | los perfiles del TRaSH Guides de ese idioma, cuando la guía publica alguno |

Dos salvedades que conviene saber de antemano. **Sonarr** queda fuera: su API
no tiene idioma de metadatos, solo el de la interfaz, y cambiar la interfaz de
quien pidió otra cosa sería una sorpresa — el idioma le llega por los perfiles
del Configarr. Y la guía solo publica perfiles de **francés** y **alemán**; en
los demás idiomas los perfiles se quedan como están, y quien quiera un archivo
concreto todavía tiene el campo de texto libre de la Configuración.

La clave de API de Bazarr acompaña a la **clave de la stack**, como la de los
*arr: el Levantar la escribe en el `config.yaml` del propio app antes de que
nadie hable con él, así que no hay nada que copiar de la interfaz. Quien quiera
otra la cambia en su Editar. Dejar vacío el campo del Entorno no toca el idioma
de ninguna app.

## Wishlist

Lo que todavía no existe, en el orden en que tendría sentido que ocurra. Los
hitos son versiones, no fechas: cada uno solo empieza después del anterior, porque
depende de él. Hoy el repositorio está en **v0.7** — la página, el servidor que
guarda la stack y la levanta en Docker, la Configuración aplicada en las apps, el
TRaSH Guides entero por Configarr (perfiles, custom formats con los scores de la
guía y las quality definitions) y el idioma de la búsqueda.

| Hito     | Entrega                                              | Se cierra cuando                                                                        |
| -------- | ---------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| ~~**v0.2**~~ | ~~Un backend que conecte `hubstarr.html` con Docker~~ | ✅ la página graba los archivos y levanta la stack sin pasar por el `.zip`              |
| ~~**v0.3**~~ | ~~Configuración automática de las stacks desde el backend~~ | ✅ Prowlarr, clientes de descarga y Media Management salen de la interfaz y son llamadas de API |
| ~~**v0.4**~~ | ~~Custom formats y profiles propios de cada stack~~ | ✅ los perfiles del TRaSH Guides, por instancia, aplicados por Configarr |
| ~~**v0.5**~~ | ~~Compatibilidad con TRaSH Guides~~               | ✅ quality definitions y scores de custom format vienen en los templates que aplica Configarr |
| ~~**v0.6**~~ | ~~Nombre de proyecto y de contenedor configurable~~ | ✅ el nombre del proyecto, en el Entorno, entra en el proyecto de compose, los contenedores y la red |
| ~~**v0.7**~~ | ~~Búsqueda localizada de medios~~                 | ✅ el **idioma de la búsqueda**, en el Entorno, baja a los metadatos de Radarr, las bibliotecas de Jellyfin, los subtítulos de Bazarr y los perfiles de Configarr |

## Comprobaciones

En cada push, GitHub Actions ejecuta lo que se puede comprobar sin que nadie
mire (`.github/workflows/ci.yml`). En el servidor, `cargo build`, `cargo test` y
`cargo clippy`. En la página, tres comprobaciones que también corren en su
máquina:

```sh
python3 tools/extract-script.py hubstarr.html > page.js && node --check page.js
python3 tools/check-i18n.py hubstarr.html     # los tres idiomas, mismas claves
python3 tools/check-compose.py                # el compose generado, por docker
```

La última abre la página en un navegador sin pantalla, arma una stack de ejemplo
y pasa el `docker-compose.yml` que generó por el `docker compose config` — el
mismo validador que rechazaría el archivo en su máquina.

En una etiqueta `v*`, el `release.yml` compila el servidor para x86_64 y arm64 y
publica ambos junto al `hubstarr.html` de esa versión.

## Estado

La interfaz es un prototipo, pero lo que produce no lo es: los archivos son de
verdad, y la **Configuración** se vuelve llamada de API en cada app por el
**Levantar** y el **Aplicar en la stack**. Ejecutando el servidor, una stack
sale de la nada y llega configurada. Abriendo solo la página, salen los
archivos en un `.zip`, para ejecutar `docker compose up -d` a mano — y lo que
depende de API queda para que lo hagas en las apps.

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

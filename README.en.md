# <img src="docs/logo.svg" width="26" align="top" alt=""> Hubstarr — *arr stack generator

*[Português (Brasil)](README.md) · English · [Español](README.es.md)*

Single-page prototype that builds the `docker-compose.yml`, the `.env` and the
`nginx.conf` for a media stack (*arr apps + download clients + media server),
with no backend and no external dependencies.

Open `hubstarr.html` in a browser. That's it — the file is
self-contained (the logos are embedded as data URIs). The **Environment** opens
with it: that is where the base paths everything else uses come from. Once
closed, it is still behind the button at the top.

![The interface: service list on the left, generated files on the right](docs/screenshot.png)

The combobox lists the available services with their logos and default ports:

![The open combobox, showing the twelve available services](docs/services.png)

## What you can do

- **Pick services** from a combobox with logos and add them to the stack.
- **Configure each instance** in a modal: title, media/downloads subfolder and
  VPN routing.
- **Copy each service's link**, with the scheme, domain and subpath nginx will
  serve it on.
- **Conflict warning**: two instances pointed at the same folder step on each
  other when importing, so the list says so in red, with the names and the
  path. Jellyfin, which mounts the whole library, and Bazarr, which follows the
  others, are left out of the check.
- **Multiple instances** of Sonarr, Radarr, Lidarr, Bazarr and Prowlarr — they
  only need different titles. Sonarr and Radarr also get
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Automatic base URL**: Sonarr, Radarr, Lidarr and Prowlarr get
  `<APP>__SERVER__URLBASE=/<container_name>`, already matching the nginx
  subpath. Bazarr exposes no such variable — set its base in its own UI.
- **API key** in the Environment: a single one for the whole stack. Sonarr,
  Radarr, Lidarr and Prowlarr land in the compose file with
  `<APP>__AUTH__APIKEY=${STARR_APIKEY}`, and SABnzbd with
  `SAB_API_KEY=${STARR_APIKEY}`; the value stays in `.env`. The key
  is generated up front — 16 random bytes in hex, the same as
  `openssl rand -hex 16` — and the "Generate" button rolls a new one.
- **Jellyfin hardware acceleration**: CPU, Intel or NVIDIA. Intel gets
  `devices: /dev/dri:/dev/dri`; NVIDIA gets the GPU reservation under `deploy`
  plus the `NVIDIA_VISIBLE_DEVICES` / `NVIDIA_DRIVER_CAPABILITIES` variables.
- **FlareSolverr alongside Prowlarr**: in the Prowlarr modal, a checkbox that
  is on by default brings FlareSolverr into the stack — it is what solves
  Cloudflare's anti-bot challenge on protected indexers. Set it up in Prowlarr
  under *Settings → Indexers → FlareSolverr*, with the URL
  `http://flaresolverr:8191`. The image behind it is
  [Byparr](https://github.com/ThePhaseless/Byparr), a drop-in, better-maintained
  replacement with the same API and the same port.
- **Per-field help** in the Environment and in the Configuration: every row has
  a `?` that opens an explanation of what the value does — and, in the
  Environment, of how it lands in the generated files.
- **Jellyfin's network.xml**: with it in the stack, its network configuration
  comes out too, with `BaseUrl` set to the nginx subpath and `nginx` in
  `KnownProxies` — without the first the UI builds its links at the root,
  without the second it logs the proxy's IP instead of the caller's. Mounted at
  `/config/config/network.xml`.
- **A ready qBittorrent.conf**: when it is in the stack, a fourth tab generates
  its initial configuration — paths matching the compose file, reverse-proxy
  settings and the credentials in qBittorrent 5.2's own format: the password as
  PBKDF2-SHA512 and the API key as `qbt_` plus 28 characters, derived from the
  stack's `${STARR_APIKEY}` — the conf is read by qBittorrent, not by compose,
  so the variable would not be expanded there. Username, password and key are
  edited in its modal, and the file is mounted over `/config`.
- **Optional HTTPS**, with the certificate and key coming from the host.
- **Configuration** (button at the top): pick which instances Prowlarr will
  configure, which *arr apps get each download client (qBittorrent, SABnzbd)
  and under which category — `tv-sonarr`, `radarr`, `lidarr`, all editable —,
  plus completed download handling on SABnzbd, and the *Media Management*
  options — hardlinks, renaming, permissions, empty
  folders and each app's full naming section (*Episode*, *Movie*, *Track
  Naming*: illegal characters, colon replacement, multi-episode style and every
  file and folder format) — split per family: Sonarr, Radarr and Lidarr.
  Permissions reveal the `chmod` and `chown` fields, and in Lidarr the existing
  name box is what brings up the track formats and the album folder. For now
  the choices
  are kept in the interface; nothing is applied to the apps.
- **Global environment** (button at the top): base paths, PUID/PGID, time zone,
  restart policy, API key and TLS. The
  time zone list is the whole IANA database, straight from the browser, and it
  starts on the machine's own zone.
- **Download** `docker-compose.yml`, `.env` and `nginx/conf.d/starrnet.conf`
  together in a `.zip`.
- **Switch languages** in the selector at the top: Portuguese (Brazil), English
  and Spanish.

## Docker

This section is also summarised on the page itself, in a collapsible notice
above the panels.

Hubstarr itself only needs a browser; it's the files it generates that need
Docker with the Compose plugin. On Linux, the official script does it:

```sh
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

On macOS and Windows — or on Linux, if you'd rather have a managed install with
a GUI — install [Docker Desktop][dd], which ships with Compose.

To use `docker` without `sudo`, add your user to the group — it takes effect on
the next session:

```sh
sudo usermod -aG docker $USER
```

[dd]: https://docs.docker.com/desktop/

With Docker in place, unzip the `.zip` and bring the stack up from the folder
holding the files:

```sh
docker compose up -d
```

## Generated conventions

The stack and network names are fixed: `starrnet`. Each instance title becomes
a slug (lowercase, no accents, spaces turned into hyphens) used as the
`container_name`, the service key and the config folder:

| Title           | container_name | config                       |
| --------------- | -------------- | ---------------------------- |
| `Radarr`        | `radarr`       | `${BASE_CONFIG}/radarr`      |
| `Radarr [UHD]`  | `radarr-uhd`   | `${BASE_CONFIG}/radarr-uhd`  |

Paths come out as variables resolved by `.env`:

- `BASE_CONFIG` — root of the config folders, one per container.
- `BASE_MEDIA` — the library. Each *arr mounts its own subfolder (`series`,
  `movies`, `music`), Jellyfin mounts the whole base, and Bazarr follows the
  subfolders of the Radarr/Sonarr instances present in the stack.
- `DOWNLOAD_BASE` — the download area. qBittorrent and SABnzbd mount their own
  subfolder (`torrents`, `usenet`); the *arr apps mount the whole base at
  `/downloads`, so they can import.

In the modal, the subfolder field shows the resolved path and takes the
variables: typing `${BASE_MEDIA}` swaps in its value right away. Pointing
outside the bases — `/mnt/disk2/movies-4k`, say — is allowed, and then the
compose file carries that literal path, with no variable at all. Bazarr
follows: it mounts each instance's path as it ended up. So does Jellyfin: on
top of the whole base it gets one volume per folder left outside it, otherwise
that library would be invisible to it. Its modal also has a **+ folder** button
for directories no other service uses — an old disk, a network share. Each one
becomes a volume at `/data/<folder name>`.

Every volume uses the long syntax, with `type: bind` and
`bind.propagation: rslave`. The port is always the service's own, inside the
container: apart from nginx there is no host port to choose, and no conflict
to worry about.

## Reverse proxy

nginx is fixed and mandatory: it is always in the stack, never shows up in the
combobox and cannot be removed. Heimdall comes in on its own too — it is the
dashboard sitting at the root — but that one can be edited; it just cannot be
removed from the list. It is the only container publishing ports on
the host — everything else stays on the `starrnet` network, reached by nginx at
`container-name:internal-port`. Whatever routes through the VPN answers at
`gluetun`, which owns the network.

Both host ports live behind **Edit** on the nginx row: 80 and 443 by default,
but you can publish on 8080 and 8443, say, if something already holds the
privileged ones. They become `HTTP_PORT` and `HTTPS_PORT` in the `.env`; inside
the container nginx keeps listening on 80 and 443. The copied links and the
redirect to https already carry the chosen port.

The **nginx.conf** tab generates the matching configuration, routing by subpath
(`/sonarr`, `/radarr`…), one `location` per service. Heimdall is the exception:
as the dashboard, it sits at the root (`location /`). The file is mounted at
`${BASE_CONFIG}/nginx/conf.d`, and each app needs its *base URL* to match its
subpath.

Not every service becomes a route: `gluetun` and FlareSolverr only talk to the
other containers, so they get no `location` and no link button — Prowlarr
reaches FlareSolverr straight over the stack network.

Seerr is the opposite: it has no base URL at all, so its `location` strips the
prefix on the way in and rewrites what comes back — the redirect headers and
the paths it writes into the HTML, through `sub_filter`.

The Environment can turn **TLS** on: `nginx.conf` then gets a `server` on 443
with `ssl_certificate`, TLSv1.2/1.3, and a block on 80 that only redirects to
https. The certificate and the key are host paths, entered in the same place,
and land in the compose file as `${TLS_CERT}` and `${TLS_KEY}`, mounted
read-only at `/etc/nginx/certs`. Without TLS the stack stays on port 80 only.
The domain becomes the `server_name` (`_` when left empty).

## VPN

Marking a client as "route through gluetun" makes the service use
`network_mode: service:gluetun`, and gluetun joins the service list right
there, if it isn't in it yet. It becomes that service's address in nginx. The
credentials live behind **Edit on gluetun** — provider, tunnel type, WireGuard
keys or the OpenVPN username/password, and the server countries — and land in
`.env` as `VPN_SERVICE_PROVIDER`, `VPN_TYPE`, `WIREGUARD_*` or `OPENVPN_*` and
`SERVER_COUNTRIES`.

## Languages

The interface speaks Portuguese (Brazil), English and Spanish. The initial
language comes from `localStorage`, falling back to the browser's and finally
to Portuguese. The translation also covers the comments in the generated files
— the YAML, the `.env` and the `nginx.conf` come out in the chosen language.

Every visible string lives in the `I18N` dictionary at the top of the
`<script>`: one key per text, holding a string, or a function when it depends
on some data. In the HTML, static texts are marked with `data-i18n` (or
`data-i18n-html`, `data-i18n-ph`, `data-i18n-title`). Adding a language means
copying one of the blocks and translating the values.

## Server (optional)

The page still works on its own, opened straight from disk. If you want the
"Create stack" button to actually create the stack, run the server under
`backend/` — a Rust binary doing what the browser cannot reach:

```sh
cd backend
cargo run --release -- --dir ~/starr
```

Then open <http://127.0.0.1:7878>. The page it serves is the very same
`hubstarr.html`, embedded in the binary, and it detects the server on its own:
when there is one, a *server* badge shows up in the header and three things
change.

- **Create stack** writes the generated files into the `--dir` folder and runs
  `docker compose up -d`, with the output in a live log. Next to it a **Tear
  down** button appears, running `docker compose down`.
- **The stack is remembered.** Instances, Environment and Configuration go into
  a SQLite database (`stack.db`) in that same folder and come back when the page
  reloads. Each add, edit or delete touches that service's row.
- **The configuration becomes real.** With the stack up, the *Apply the
  configuration* button uses each app's API to create what only exists in their
  database: Prowlarr pointing at every *arr, the download clients with each
  one's category, and Media Management along with the naming. It is the only
  part of the interface that does not fit in a file — and it is idempotent, so
  applying it again after changing the Configuration is the normal use.

Without a server none of that shows up and the behaviour is the usual one: the
`.zip` and the simulated deploy. The server never generates content — it
receives what the page built, so the generators keep living in a single place.

Options: `--dir` (folder for the files, default `./stack`), `--addr` (address,
default `127.0.0.1:7878`) and `--docker` (the command, for podman users).

## Status

The page on its own is an interface prototype: with no server, the "Create
stack" button only simulates the deploy and the **Configuration** choices turn
into no API call. The generated files were always the real thing — and with the
server under `backend/`, the deploy and the Configuration become real too.

## License

[GNU General Public License v3.0](LICENSE) or later. Use, study, modify and
redistribute it freely; if you distribute a modified version, it has to ship
with the source and under the same license. No warranty — see sections 15 and
16 of the text.

The service logos belong to their own projects and come from
[dashboardicons.com](https://dashboardicons.com); the nginx one is
[Nginx](https://iconscout.com/icons/nginx) by
[Icon 54](https://iconscout.com/contributors/icon-54), on IconScout. The GPL
covers Hubstarr, not them.

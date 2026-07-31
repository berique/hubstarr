# Hubstarr — *arr stack generator

*[Português (Brasil)](README.md) · English · [Español](README.es.md)*

Single-page prototype that builds the `docker-compose.yml`, the `.env` and the
`nginx.conf` for a media stack (*arr apps + download clients + media server),
with no backend and no external dependencies.

Open `arr-stack-prototype.html` in a browser. That's it — the file is
self-contained (the logos are embedded as data URIs).

![The interface: service list on the left, generated files on the right](docs/screenshot.png)

The combobox lists the available services with their logos and default ports:

![The open combobox, showing the twelve available services](docs/services.png)

## What you can do

- **Pick services** from a combobox with logos and add them to the stack.
- **Configure each instance** in a modal: title, media/downloads subfolder and
  VPN routing.
- **Copy each service's link**, with the scheme, domain and subpath nginx will
  serve it on.
- **Multiple instances** of Sonarr, Radarr, Lidarr, Bazarr and Prowlarr — they
  only need different titles. Sonarr and Radarr also get
  `SONARR__APP__INSTANCENAME` / `RADARR__APP__INSTANCENAME`.
- **Automatic base URL**: Sonarr, Radarr, Lidarr and Prowlarr get
  `<APP>__SERVER__URLBASE=/<container_name>`, already matching the nginx
  subpath. Bazarr exposes no such variable — set its base in its own UI.
- **API key** in the Environment: a single one for the whole stack. Sonarr,
  Radarr, Lidarr and Prowlarr land in the compose file with
  `<APP>__AUTH__APIKEY=${STARR_APIKEY}` and the value stays in `.env`. The key
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
- **Per-field help** in the Environment: every row has a `?` that opens an
  explanation of what the value does and how it lands in the generated files.
- **Optional HTTPS**, with the certificate and key coming from the host.
- **Global environment** (button at the top): base paths, PUID/PGID, time zone,
  restart policy, host ports, API key, TLS and the gluetun credentials. The
  time zone list is the whole IANA database, straight from the browser, and it
  starts on the machine's own zone.
- **Download** `docker-compose.yml`, `.env` and `nginx/conf.d/starrnet.conf`
  together in a `.zip`.
- **Switch languages** in the selector at the top: Portuguese (Brazil), English
  and Spanish.

## Docker

Hubstarr itself only needs a browser; it's the files it generates that need
Docker with the Compose plugin. On Linux, the official script does it:

```sh
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

On macOS and Windows — or on Linux, if you'd rather have a managed install with
a GUI — install [Docker Desktop][dd].

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

Every volume uses the long syntax, with `type: bind` and
`bind.propagation: rslave`. The port is always the service's own, inside the
container: apart from nginx there is no host port to choose, and no conflict
to worry about.

## Reverse proxy

nginx is fixed and mandatory: it is always in the stack, never shows up in the
combobox and cannot be removed. It is the only container publishing ports on
the host — everything else stays on the `starrnet` network, reached by nginx at
`container-name:internal-port`. Whatever routes through the VPN answers at
`gluetun`, which owns the network.

Both host ports live in the Environment, under **Host ports (nginx)**: 80 and
443 by default, but you can publish on 8080 and 8443, say, if something already
holds the privileged ones. They become `HTTP_PORT` and `HTTPS_PORT` in the
`.env`; inside the container nginx keeps listening on 80 and 443. The copied
links and the redirect to https already carry the chosen port.

The **nginx.conf** tab generates the matching configuration, routing by subpath
(`/sonarr`, `/radarr`…), one `location` per service. Heimdall is the exception:
as the dashboard, it sits at the root (`location /`). The file is mounted at
`${BASE_CONFIG}/nginx/conf.d`, and each app needs its *base URL* to match its
subpath.

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
credentials (`VPN_SERVICE_PROVIDER`, `VPN_TYPE`, WireGuard keys or the OpenVPN
username/password, `SERVER_COUNTRIES`) stay in `.env`.

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

## Status

An interface prototype: the "Create stack" button only simulates the deploy.
The generated `docker-compose.yml`, `.env` and `nginx.conf`, on the other hand,
are the real thing.

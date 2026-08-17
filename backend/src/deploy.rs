/* Calling docker compose.

   The compose is run with the files folder as the project directory, so that
   the relative paths in the compose and the `.env` sitting there are found just
   as they would be if someone had run the command by hand in that folder. */

use std::path::Path;
use std::process::Stdio;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::jobs::Log;

/// Does `docker compose` answer? It is what the page uses to warn before even
/// trying to bring the stack up. It asks for the plugin, not just for docker:
/// it is `compose` that brings the stack up, and it is a separate package that
/// may be missing from a docker that is there and working. The same holds for
/// `podman compose`.
/// The engines we know how to run, in the order they are tried when nobody
/// passed `--docker`. `podman` is here because `podman compose` runs the same
/// file — whoever has only that installed has no `docker` to find at all, and
/// the page would open the "install Docker" warning on a machine that is ready
/// to bring the stack up.
pub const ENGINES: [&str; 2] = ["docker", "podman"];

/// Which engine to use: the one from the command line, if it came, otherwise
/// the first of `ENGINES` that answers. With none, `docker` stays — it is what
/// the page's `docker_ok()` message tells people to install.
pub async fn pick_engine(chosen: Option<String>) -> String {
    pick_from(chosen, &ENGINES).await
}

/// The core of `pick_engine()`, with the list passed in — that is how the test
/// gets in, pointing at made-up commands instead of depending on what is
/// installed on the machine.
async fn pick_from(chosen: Option<String>, engines: &[&str]) -> String {
    if let Some(c) = chosen {
        return c;
    }
    for e in engines {
        if docker_ok(e).await {
            return e.to_string();
        }
    }
    engines[0].to_string()
}

pub async fn docker_ok(docker: &str) -> bool {
    Command::new(docker)
        .args(["compose", "version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/* The state of each container of the stack, for the status dot in the list.

   The `compose ps` output is read with `--format json`, which comes out as one
   line per container (in newer versions) or a single array (in older ones) —
   both cases land here. `--all` is what makes a stopped container show up:
   without it, stopped and non-existent would look the same, and that is exactly
   the difference the dot shows.

   The key is the compose `Service`, which is the page's `cname()`; whoever does
   not appear in the answer was never created. */
pub async fn status(docker: &str, dir: &Path) -> Result<Value, String> {
    let out = Command::new(docker)
        .args(["compose", "ps", "--format", "json", "--all"])
        .current_dir(dir)
        .output()
        .await
        .map_err(|e| format!("não consegui rodar o {docker}: {e}"))?;
    // a folder with no compose yet, or docker down: nobody is up, and that is
    // not an error — the page simply paints no dot at all
    if !out.status.success() {
        return Ok(json!({}));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows: Vec<Value> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(a)) => rows.extend(a),
            Ok(v) => rows.push(v),
            Err(_) => {}
        }
    }

    let mut map = serde_json::Map::new();
    for r in rows {
        let key = r
            .get("Service")
            .or_else(|| r.get("Name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if key.is_empty() {
            continue;
        }
        map.insert(
            key.to_string(),
            json!({
                "state":  r.get("State").and_then(|v| v.as_str()).unwrap_or(""),
                "status": r.get("Status").and_then(|v| v.as_str()).unwrap_or(""),
                "health": r.get("Health").and_then(|v| v.as_str()).unwrap_or(""),
            }),
        );
    }
    Ok(Value::Object(map))
}

/// The services of the stack that are up right now, by compose service name —
/// which is the page's `cname()`, and the container name too. It is the same
/// `compose ps` the status dot uses; here it answers a different question:
/// **is there anything to configure?** A container that did not come up (a name
/// taken by another stack, a port already bound) answers nothing over HTTP, and
/// waiting ninety seconds for it to say so is ninety seconds of nothing.
pub async fn running(docker: &str, dir: &Path) -> Vec<String> {
    let Ok(Value::Object(map)) = status(docker, dir).await else {
        return Vec::new();
    };
    map.into_iter()
        .filter(|(_, v)| v.get("state").and_then(Value::as_str) == Some("running"))
        .map(|(k, _)| k)
        .collect()
}

pub async fn up(docker: &str, dir: &Path, log: Log) -> Result<(), String> {
    run(docker, &["compose", "up", "-d", "--remove-orphans"], dir, &log).await?;
    // whatever just got (re)created may have a new IP; nginx needs to forget
    // the old one before wait_apps() starts polling through it
    reload_nginx(docker, dir, &log).await;
    Ok(())
}

pub async fn down(docker: &str, dir: &Path, log: Log) -> Result<(), String> {
    run(docker, &["compose", "down", "--remove-orphans"], dir, &log).await
}

/// A service name that may become a compose argument. The page's `cname()`
/// only produces lowercase letters, digits and hyphens; anything outside that
/// did not come from there and does not go on the command line. A leading
/// hyphen is refused separately: it would be a valid name by the letter rule,
pub fn ok_service(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('-')
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// but the compose would read it as an option.
/// Brings a single container up, without touching the others. `--no-deps` is
/// what keeps the promise of the click: whoever is stopped next to it stays stopped.
pub async fn up_one(docker: &str, dir: &Path, key: &str, log: Log) -> Result<(), String> {
    run(docker, &["compose", "up", "-d", "--no-deps", key], dir, &log).await?;
    // recreated with --no-deps still gets a fresh IP; skip when the service
    // brought up *is* nginx — it just started with nothing cached yet
    if key != NGINX_SERVICE {
        reload_nginx(docker, dir, &log).await;
    }
    Ok(())
}

/// Stops a single container. It is `stop`, not `down`: `down` takes the whole
/// stack away, and what is wanted here is the opposite — only that service goes
/// off the air, and the container keeps existing so the dot goes back to
/// "stopped" instead of "not created".
pub async fn stop_one(docker: &str, dir: &Path, key: &str, log: Log) -> Result<(), String> {
    run(docker, &["compose", "stop", key], dir, &log).await
}

/// What the page sends to run Configarr: paths, network and user — the
/// decisions stay over there, here we only build the command line.
#[derive(serde::Deserialize, Clone)]
pub struct Configarr {
    /// its folder on the host, with `config.yml`, `secrets.yml`, the custom
    /// formats and the repository cache
    pub dir: String,
    /// the stack network, through which it reaches the *arr apps by container name
    pub network: String,
    /// the Environment's `PUID:PGID` — whoever has to own the cache
    pub user: String,
    #[serde(default)]
    pub tz: String,
}

/// Configarr, which applies the TRaSH Guides quality profiles and custom
/// formats from the `config.yml` the page wrote.
///
/// It is a standalone `docker run --rm`, not a service of the stack: it runs
/// once and exits, and in an `up -d` it would start before the apps answer. It
/// joins the stack network to reach each *arr by container name, with the base
/// URL that the `config.yml` already carries. The caller waits for the apps
/// first, like the rest of `apply`.
///
/// `--dns` is what lets it resolve github from inside the stack network, which
/// is a bridge of ours: without it the TRaSH and Recyclarr clone fails on a
/// machine whose resolver is not reachable from there.
pub async fn configarr(docker: &str, cfg: &Configarr, log: &Log) -> Result<(), String> {
    log.line("aplicando os perfis do TRaSH Guides (configarr)…");
    let dir = cfg.dir.trim_end_matches('/');
    let tz = if cfg.tz.is_empty() { "Etc/UTC" } else { &cfg.tz };
    let build = [
        format!("{dir}/config.yml:/app/config/config.yml:ro"),
        format!("{dir}/secrets.yml:/app/config/secrets.yml:ro"),
        format!("{dir}/custom_formats:/app/cfs:ro"),
        format!("{dir}/repos:/app/repos"),
    ];
    let mut args: Vec<String> = vec![
        "run".into(), "--rm".into(),
        "--name".into(), "configarr".into(),
        "--user".into(), cfg.user.clone(),
        "--network".into(), cfg.network.clone(),
        "--dns".into(), "1.1.1.1".into(),
    ];
    for m in &build {
        args.push("-v".into());
        args.push(m.clone());
    }
    /* The cache may have been cloned by another owner — by root, for whoever
       comes from the version in which Configarr was a compose service and ran
       without `--user`. Git refuses to touch a repository owned by someone else
       ("dubious ownership") and Configarr dies before reading the `config.yml`;
       these three variables are the sanctioned way of saying that this is home,
       without needing a `git config` inside the container. */
    for e in [
        "LOG_STACKTRACE=true",
        "LOG_LEVEL=debug",
        "GIT_CONFIG_COUNT=1",
        "GIT_CONFIG_KEY_0=safe.directory",
        "GIT_CONFIG_VALUE_0=*",
    ] {
        args.push("-e".into());
        args.push(e.into());
    }
    args.push("-e".into());
    args.push(format!("TZ={tz}"));
    args.push(CONFIGARR_IMG.into());

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(docker, &refs, Path::new(dir), log).await.inspect_err(|_| {
        /* The docker error does not say what the problem is when it is ownership:
           Configarr dies on a git "Permission denied", in the middle of a node
           stack trace. The known case is a cache cloned by another owner —
           whoever comes from the version in which it was a compose service and
           ran as root. The way out is to delete the cache: it rebuilds itself. */
        if !writable(&format!("{dir}/repos")) {
            log.line(format!(
                "o cache em {dir}/repos é de outro dono e o Configarr roda como {};                  apague a pasta (ela se refaz sozinha) e mande subir de novo",
                cfg.user
            ));
        }
    })
}

/// Does the folder exist and take writes from whoever runs the server? It is
/// the server that creates the stack folders, and Configarr runs with the same PUID/PGID.
fn writable(dir: &str) -> bool {
    let probe = Path::new(dir).join(".hubstarr-escrita");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub const CONFIGARR_IMG: &str = "ghcr.io/raydak-labs/configarr:latest";

/// The reverse proxy's container name — the page's fixed line (`NGINX.id` in
/// the script), always in the compose regardless of what is in `added`.
const NGINX_SERVICE: &str = "nginx";

/// Makes nginx forget whatever IP it had cached for each upstream.
///
/// Every route but the qBittorrent one is a plain `proxy_pass
/// http://cname:port;`, with no variable and no `resolver` — see the
/// `stripBase` invariant in CLAUDE.md for why that one is different. A plain
/// `proxy_pass` is resolved once, at nginx's own startup or last reload, and
/// kept for as long as the worker lives. If some other container gets
/// recreated in the meantime — `up_one` of a single service, a plain `up`
/// that only touches what changed — Docker hands it a new IP on the stack
/// network, and nginx goes on forwarding to the address nobody listens on
/// anymore: a 502 that looks exactly like the app still starting, and
/// `wait_apps()` waits out its whole budget for an app that has been up for
/// a while.
///
/// Called after every `up`/`up_one` that could have handed out a new IP, and
/// before anything starts polling the apps through nginx. Failing this is
/// not fatal — nginx not existing yet (nothing has ever come up) is not an
/// error, and a message in the job log is enough for anyone wondering why a
/// route stayed dead a moment longer than it should have.
async fn reload_nginx(docker: &str, dir: &Path, log: &Log) {
    if let Err(e) = compose(docker, &["exec", "-T", NGINX_SERVICE, "nginx", "-s", "reload"], dir, log).await
    {
        log.line(format!(
            "não consegui recarregar o nginx ({e}); se algum container foi recriado, a rota dele pode ficar respondendo 502 até o próximo Subir"
        ));
    }
}

/// Any `docker compose <args>` in the stack folder — it is how `patch.rs`
/// stops and starts a single container around editing its configuration.
pub async fn compose(docker: &str, args: &[&str], dir: &Path, log: &Log) -> Result<(), String> {
    let mut all = vec!["compose"];
    all.extend_from_slice(args);
    run(docker, &all, dir, log).await
}

/// Runs the command in the stack folder copying both outputs to the log,
/// line by line — the compose writes its progress to stderr.
async fn run(docker: &str, args: &[&str], dir: &Path, log: &Log) -> Result<(), String> {
    log.line(format!("$ {docker} {}", args.join(" ")));

    let mut child = Command::new(docker)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        /* The modal's Stop aborts the job's task, and `child` falls with it:
           without this the `docker compose` would go on running by itself, with
           nobody reading its output. */
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("não consegui rodar o {docker}: {e}"))?;

    let out = child.stdout.take().map(|h| pipe(h, log.clone()));
    let err = child.stderr.take().map(|h| pipe(h, log.clone()));
    if let (Some(a), Some(b)) = (out, err) {
        let _ = tokio::join!(a, b);
    }

    let st = child.wait().await.map_err(|e| e.to_string())?;
    if st.success() {
        log.line("pronto.");
        Ok(())
    } else {
        Err(format!("{docker} terminou com {st}"))
    }
}

async fn pipe<R>(handle: R, log: Log)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(handle).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        log.line(line);
    }
}

#[cfg(test)]
mod tests {
    use super::{ok_service, pick_from};

    /// A made-up command that answers (or not) to `compose version`.
    fn fake(name: &str, responds: bool) -> String {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("hubmotor{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let body = if responds {
            "#!/bin/sh\n[ \"$1 $2\" = 'compose version' ] && exit 0\nexit 1\n"
        } else {
            "#!/bin/sh\nexit 127\n"
        };
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[tokio::test]
    async fn the_engine_chosen_on_the_command_line_wins() {
        let good = fake("bom", true);
        // the one that came is not even asked: whoever passed --docker has decided
        assert_eq!(pick_from(Some("qualquer".into()), &[&good]).await, "qualquer");
    }

    #[tokio::test]
    async fn with_no_option_the_first_that_answers_wins() {
        let broken = fake("quebrado", false);
        let good = fake("bom", true);
        let e: Vec<&str> = vec![&broken, &good];
        assert_eq!(pick_from(None, &e).await, good);
        // and the first one is still the first when it answers
        let e: Vec<&str> = vec![&good, &broken];
        assert_eq!(pick_from(None, &e).await, good);
    }

    #[tokio::test]
    async fn with_no_engine_the_first_of_the_list_is_used() {
        let broken = fake("quebrado", false);
        // it does not even exist as a file: `Command` fails before running
        let e: Vec<&str> = vec![&broken, "hubstarr-motor-que-nao-existe"];
        assert_eq!(pick_from(None, &e).await, broken);
    }

    #[test]
    fn a_service_name_only_accepts_what_cname_produces() {
        assert!(ok_service("sonarr-4k"));
        assert!(!ok_service(""));
        assert!(!ok_service("--rmi")); // the compose would read this as an option
        assert!(!ok_service("sonarr; rm -rf /"));
        assert!(!ok_service("../fora"));
        assert!(!ok_service("Sonarr"));
    }
}

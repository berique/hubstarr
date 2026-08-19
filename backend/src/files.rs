/* Writing the files the page sent.

   The content arrives ready — this module generates nothing, it only decides
   where each file lands. The names come from the same place that feeds the .zip
   (`docker-compose.yml`, `.env`, `nginx.conf`, `<container>/<service conf>`), so
   they carry subfolders and have to be checked before becoming a path.

   Two roots, because the generated tree has two halves. The
   `docker-compose.yml`, the `.env` and the `nginx.conf` stay in the stack
   folder, which is where the compose is run from — and the nginx bind is
   relative to it. The service configurations go to the Environment's
   `BASE_CONFIG`, because that is where the compose itself mounts them into the
   containers: writing them into the stack folder makes the container come up
   with no configuration at all.

   Which is which is said by the page, in each file's `base` field — it is the
   one that knows the layout. The server only resolves `BASE_CONFIG`, which it
   reads from the database instead of accepting an absolute path from the
   browser. */

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::msg;
use crate::msg::Msg;

#[derive(Deserialize)]
pub struct OutFile {
    pub name: String,
    pub text: String,
    /// "config" for what the containers mount; the rest stays in the stack
    #[serde(default)]
    pub base: Option<String>,
}

/// Joins the name to the destination refusing anything that escapes it:
/// absolute path, `..`, Windows root. Without this, a malformed `name` would
/// write anywhere the process has permission to.
fn safe_join(dir: &Path, name: &str) -> Result<PathBuf, Msg> {
    if name.trim().is_empty() {
        return Err(Msg::k("job.files.emptyName"));
    }
    if name.contains('\\') {
        return Err(msg!("job.files.invalidName", name));
    }
    let rel = Path::new(name);
    let mut out = dir.to_path_buf();
    for c in rel.components() {
        match c {
            Component::Normal(part) => out.push(part),
            _ => return Err(msg!("job.files.invalidName", name)),
        }
    }
    Ok(out)
}

/// Writes them all and returns the paths written. `cfg` is the Environment's
/// `BASE_CONFIG`; without it — a stack whose Environment was never saved —
/// everything lands in the stack folder, as before.
pub async fn write_all(
    dir: &Path,
    cfg: Option<&Path>,
    files: &[OutFile],
) -> Result<Vec<String>, Msg> {
    if files.is_empty() {
        return Err(Msg::k("job.files.none"));
    }
    let mut done = Vec::with_capacity(files.len());
    for f in files {
        let root = match f.base.as_deref() {
            Some("config") => cfg.unwrap_or(dir),
            _ => dir,
        };
        let path = safe_join(root, &f.name)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| msg!("job.files.mkdirError", parent.display().to_string(), e.to_string()))?;
        }
        tokio::fs::write(&path, &f.text)
            .await
            .map_err(|e| msg!("job.files.writeError", path.display().to_string(), e.to_string()))?;
        crate::journal::detail(|| {
            format!("file {} ({} bytes)", path.display(), f.text.len())
        });
        done.push(path.display().to_string());
    }
    Ok(done)
}

/// Creates the folders the page listed, when they are missing. A relative path
/// is refused: these come from the compose `source` entries, which belong to
/// the host and are absolute — a relative one here would be a caller's mistake,
/// and would land somewhere unpredictable relative to the server's directory.
pub async fn ensure_dirs(dirs: &[String], log: &crate::jobs::Log) -> Result<(), Msg> {
    for d in dirs {
        let p = Path::new(d);
        if !p.is_absolute() {
            return Err(msg!("job.dirs.relative", d.clone()));
        }
        if p.is_dir() {
            continue;
        }
        if p.exists() {
            // already there and not a folder: the bind is going to fail, and saying so
            // now is better than the docker error three steps later
            return Err(msg!("job.dirs.notADir", d.clone()));
        }
        tokio::fs::create_dir_all(p)
            .await
            .map_err(|e| msg!("job.dirs.mkdirError", d.clone(), e.to_string()))?;
        crate::journal::detail(|| format!("folder {d}"));
        log.line(msg!("job.dirs.created", d.clone()));
    }
    Ok(())
}

/* Deleting an instance's configuration folder — the one the compose binds at
   `/config`, and the whole of what the app knows: its database, its history,
   its indexers. It goes with the row when somebody clicks Delete, and never on
   any other path: `reconcile()` removes rows nobody clicked, and a row is a
   save away from coming back while a Sonarr database is not.

   The page sends the path because the page is what builds paths — the same
   `cfgReal()` the row already shows. It is not taken on trust, though: three
   things have to hold, and any of them failing is a refusal rather than a
   deletion somewhere else.

   - it is absolute, and has no `..` to walk back out of wherever it points;
   - its parent is exactly the Environment's `BASE_CONFIG`, so `/`, `$HOME` and
     the stack folder are all out of reach however wrong the page is;
   - it is named after the instance key, which is the `container_name` and the
     folder the compose mounts — a path that passed the first two but points at
     a neighbouring app's folder is still the wrong folder.

   A folder that is not there is not a failure: the instance may never have been
   deployed. It comes back as `Ok(None)`, and nothing is recorded. */
pub fn remove_config_dir(dir: &str, key: &str, cfg: Option<&str>) -> Result<Option<String>, String> {
    let Some(cfg) = cfg.filter(|c| !c.trim().is_empty()) else {
        return Err("without BASE_CONFIG from the Environment there is no folder to delete".into());
    };
    let p = Path::new(dir);
    if !p.is_absolute() || p.components().any(|c| c == Component::ParentDir) {
        return Err(format!("refused: {dir} is not an absolute path without `..`"));
    }
    if p.parent() != Some(Path::new(cfg)) {
        return Err(format!("refused: {dir} is not inside BASE_CONFIG ({cfg})"));
    }
    if p.file_name() != Some(std::ffi::OsStr::new(key)) {
        return Err(format!("refused: {dir} is not the folder of {key}"));
    }
    if !p.is_dir() {
        return Ok(None);
    }
    std::fs::remove_dir_all(p).map_err(|e| format!("{dir}: {e}"))?;
    Ok(Some(dir.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, base: Option<&str>) -> OutFile {
        OutFile { name: name.into(), text: "x\n".into(), base: base.map(String::from) }
    }

    #[tokio::test]
    async fn creates_what_is_missing_and_refuses_what_is_not_a_folder() {
        let tmp = std::env::temp_dir().join(format!("hubdirs-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&tmp).await;
        let log = crate::jobs::Jobs::new().test_log();
        let a = tmp.join("media/tv");
        ensure_dirs(&[a.display().to_string()], &log).await.unwrap();
        assert!(a.is_dir());
        // doing it again is not an error: what already exists stays as it is
        ensure_dirs(&[a.display().to_string()], &log).await.unwrap();

        // a path taken by a file is refused before docker even tries
        let f = tmp.join("um-arquivo");
        tokio::fs::write(&f, b"x").await.unwrap();
        assert!(ensure_dirs(&[f.display().to_string()], &log).await.is_err());
        // and a relative path too
        assert!(ensure_dirs(&["media/tv".into()], &log).await.is_err());
        tokio::fs::remove_dir_all(&tmp).await.unwrap();
    }

    #[tokio::test]
    async fn the_conf_goes_to_base_config_and_the_compose_stays_in_the_stack() {
        let tmp = std::env::temp_dir().join(format!("hubstarr-teste-{}", std::process::id()));
        let (dir, cfg) = (tmp.join("stack"), tmp.join("appdata"));
        let files = vec![
            f("docker-compose.yml", None),
            f("nginx/conf.d/starrnet.conf", Some("config")),
            f("qbittorrent/qBittorrent.conf", Some("config")),
        ];
        write_all(&dir, Some(&cfg), &files).await.unwrap();
        assert!(dir.join("docker-compose.yml").exists());
        assert!(cfg.join("nginx/conf.d/starrnet.conf").exists());
        assert!(cfg.join("qbittorrent/qBittorrent.conf").exists());
        // the conf must not also land in the stack folder: that is where it gets lost
        assert!(!dir.join("nginx/conf.d/starrnet.conf").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn with_no_base_config_everything_lands_in_the_stack_folder() {
        let tmp = std::env::temp_dir().join(format!("hubstarr-teste2-{}", std::process::id()));
        write_all(&tmp, None, &[f("nginx/conf.d/x.conf", Some("config"))])
            .await
            .unwrap();
        assert!(tmp.join("nginx/conf.d/x.conf").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn refuses_what_leaves_the_destination() {
        let d = Path::new("/tmp/stack");
        assert!(safe_join(d, "docker-compose.yml").is_ok());
        assert!(safe_join(d, "nginx/conf.d/starr.conf").is_ok());
        assert!(safe_join(d, "../fora.yml").is_err());
        assert!(safe_join(d, "/etc/passwd").is_err());
        assert!(safe_join(d, "qbit\\conf").is_err());
        assert!(safe_join(d, "  ").is_err());
    }

    /// The three refusals matter more than the deletion: the path comes from
    /// the browser, and a page with the wrong idea of the layout must not be
    /// able to point this at anything but the instance's own folder.
    #[test]
    fn removing_a_config_folder_refuses_what_is_not_the_instance_folder() {
        let tmp = std::env::temp_dir().join(format!("hubstarr-rm-{}", std::process::id()));
        let cfg = tmp.join("config");
        let mine = cfg.join("sonarr");
        std::fs::create_dir_all(&mine).unwrap();
        let c = cfg.display().to_string();

        // no BASE_CONFIG: nothing to check the path against, so nothing is deleted
        assert!(remove_config_dir(&mine.display().to_string(), "sonarr", None).is_err());
        // a neighbour's folder, right base and all
        assert!(remove_config_dir(&cfg.join("radarr").display().to_string(), "sonarr", Some(&c)).is_err());
        // outside BASE_CONFIG
        assert!(remove_config_dir("/tmp/sonarr", "sonarr", Some(&c)).is_err());
        // relative, and the `..` that would walk back out
        assert!(remove_config_dir("sonarr", "sonarr", Some(&c)).is_err());
        assert!(remove_config_dir(&cfg.join("..").join("sonarr").display().to_string(), "sonarr", Some(&c)).is_err());
        // deeper than one level: the parent is no longer BASE_CONFIG
        assert!(remove_config_dir(&mine.join("sub").display().to_string(), "sub", Some(&c)).is_err());
        assert!(mine.is_dir(), "not one of those may have deleted anything");

        // never deployed: not there is not a failure
        assert_eq!(
            remove_config_dir(&cfg.join("lidarr").display().to_string(), "lidarr", Some(&c)).unwrap(),
            None
        );
        // and the one that does hold
        assert_eq!(
            remove_config_dir(&mine.display().to_string(), "sonarr", Some(&c)).unwrap(),
            Some(mine.display().to_string())
        );
        assert!(!mine.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

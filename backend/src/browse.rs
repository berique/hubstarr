/* The host's folders, so the Environment's paths can be chosen instead of typed.

   The page has no way of its own to do this: a browser cannot list a directory,
   and the field where BASE_CONFIG or BASE_MEDIA goes is a path on the machine
   the server is running on — often not even the machine the page is open on.
   So the two things it needs are here, and nothing else: reading a folder, and
   creating one inside a folder that already exists. No rename, no delete, no
   file contents — the point is choosing a path, not managing the disk.

   It is also the only place the server answers about paths that are not the
   stack's, so the two rules that keep it honest live here rather than at the
   call sites:

   - a listing always lands on a folder that **exists**: what was asked, or the
     nearest existing ancestor of it, up to `/`. Someone who typed
     `/mnt/media/movies` before creating it opens the browser at `/mnt`, which
     is exactly where the "New folder" button is about to be used;
   - a new folder is a single **name** inside a folder that already exists,
     never a path: `..`, a slash or a backslash in it would put the folder
     somewhere other than the one on screen. It is the same rule as
     `files::safe_join()`, for the same reason.

   What it does not do is widen the door. Whoever reaches this port can already
   write files and run `docker compose` here — the `--help` says so, and the
   address it listens on is the whole of the access control. What browsing adds
   is seeing the machine's folder names, which is the same trust. */

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::msg;
use crate::msg::Msg;

/// A listing carries at most this many entries. A folder with tens of thousands
/// of them is not something anybody scrolls, and the answer would be megabytes
/// of names on the way to a click that was never going to happen.
const MAX: usize = 2000;

/// The folder to list. Empty — the field was never filled in — means `$HOME`.
#[derive(Deserialize)]
pub struct Ask {
    #[serde(default)]
    pub path: Option<String>,
}

/// A folder to create: the name, inside the folder on screen.
#[derive(Deserialize)]
pub struct NewDir {
    pub path: String,
    pub name: String,
}

/// Lists a folder: the folders first, then the files, each set in alphabetical
/// order. The files come along because the choice is made by eye — a folder
/// with the season in it is how one tells the library apart from its neighbour
/// — and because the certificate fields choose a file, not a folder.
///
/// Whether an entry is hidden is not decided here: the name comes as it is and
/// the page's checkbox is what filters it. The server sorts and counts; it does
/// not decide what the person is looking for.
pub async fn list(want: &str) -> Result<Value, Msg> {
    let at = settle(want);
    let read = |e: std::io::Error| msg!("fb.e.read", at.display().to_string(), e.to_string());
    let mut rd = tokio::fs::read_dir(&at).await.map_err(read)?;
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    let mut truncated = false;
    while let Some(e) = rd.next_entry().await.map_err(read)? {
        if dirs.len() + files.len() >= MAX {
            truncated = true;
            break;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if is_dir(&e).await {
            dirs.push(name)
        } else {
            files.push(name)
        }
    }
    let key = |s: &String| s.to_lowercase();
    dirs.sort_by_key(key);
    files.sort_by_key(key);
    crate::journal::detail(|| format!("browse {} ({} entries)", at.display(), dirs.len() + files.len()));
    let entries: Vec<Value> = dirs
        .into_iter()
        .map(|n| json!({"name": n, "dir": true}))
        .chain(files.into_iter().map(|n| json!({"name": n, "dir": false})))
        .collect();
    Ok(json!({
        "path": at.display().to_string(),
        // `/` has no parent, and that is what stops the ".." row from showing there
        "parent": at.parent().map(|p| p.display().to_string()),
        "entries": entries,
        "truncated": truncated,
    }))
}

/// Creates one folder inside the one on screen, and answers with its path — the
/// page walks into it, which is what the next click was going to be anyway.
pub async fn mkdir(req: &NewDir) -> Result<Value, Msg> {
    let name = req.name.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(msg!("fb.e.badName", req.name.clone()));
    }
    let at = Path::new(req.path.trim());
    if !at.is_absolute() || at.components().any(|c| c == Component::ParentDir) || !at.is_dir() {
        return Err(msg!("fb.e.notADir", req.path.clone()));
    }
    let p = at.join(name);
    // a folder that is already there is not created again: saying so is better
    // than a silent success over somebody else's folder
    if p.exists() {
        return Err(msg!("fb.e.exists", name.to_string()));
    }
    tokio::fs::create_dir(&p)
        .await
        .map_err(|e| msg!("fb.e.mkdir", p.display().to_string(), e.to_string()))?;
    /* It belongs in the short log: it is a change to the machine, made by a
       click, and the owner is the user running the server — the same one that
       creates the stack folders, which is why the apps can write in them. */
    crate::journal::record(format!("{} folder created: {}", crate::journal::stamp(), p.display()));
    Ok(json!({"ok": true, "path": p.display().to_string()}))
}

/// A symlink to a folder counts as a folder: what matters here is whether it
/// can be walked into, not how the entry is stored.
async fn is_dir(e: &tokio::fs::DirEntry) -> bool {
    match e.file_type().await {
        Ok(t) if !t.is_symlink() => t.is_dir(),
        // a broken link answers no, like anything else that cannot be read
        _ => tokio::fs::metadata(e.path())
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false),
    }
}

/// Where the listing actually lands: what was asked, if it is a folder, and
/// otherwise the nearest ancestor of it that is. A relative path — or none at
/// all — starts at `$HOME`; a machine without one starts at `/`.
fn settle(want: &str) -> PathBuf {
    let want = want.trim();
    let start = match want {
        "" => home(),
        w if Path::new(w).is_absolute() => PathBuf::from(w),
        _ => home(),
    };
    let mut p = start.as_path();
    while !p.is_dir() {
        match p.parent() {
            Some(up) => p = up,
            None => return PathBuf::from("/"),
        }
    }
    // resolves `.`, `..` and links, so the breadcrumb on the page is the real path
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(what: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hubstarr-browse-{what}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// The one that makes the browser open somewhere useful: a path typed
    /// before it existed lands on the closest folder that does.
    #[test]
    fn a_path_that_is_not_there_yet_settles_on_the_nearest_folder_that_is() {
        let base = tmp("settle");
        let deep = base.join("media/movies/4k");
        assert_eq!(settle(&deep.display().to_string()), std::fs::canonicalize(&base).unwrap());
        assert_eq!(settle(&base.display().to_string()), std::fs::canonicalize(&base).unwrap());
        // a relative path is not resolved against the server's directory: it starts at HOME
        assert_eq!(settle("media/tv"), home());
        assert_eq!(settle("   "), home());
        assert!(settle("/").is_dir());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn it_lists_folders_first_and_says_where_it_is() {
        let base = tmp("list");
        std::fs::create_dir(base.join("tv")).unwrap();
        std::fs::create_dir(base.join("Anime")).unwrap();
        std::fs::write(base.join("um-arquivo"), b"x").unwrap();
        let v = list(&base.display().to_string()).await.unwrap();
        let names: Vec<&str> = v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Anime", "tv", "um-arquivo"]);
        assert_eq!(v["entries"][2]["dir"], json!(false));
        assert_eq!(v["path"], json!(std::fs::canonicalize(&base).unwrap().display().to_string()));
        assert!(v["parent"].is_string());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The refusals matter more than the creation: the name comes from the
    /// browser, and it must not be able to reach outside the folder on screen.
    #[tokio::test]
    async fn creating_a_folder_refuses_anything_that_is_not_a_plain_name() {
        let base = tmp("mkdir");
        let at = base.display().to_string();
        let new = |name: &str| NewDir { path: at.clone(), name: name.into() };
        for bad in ["", "  ", ".", "..", "../fora", "a/b", "a\\b"] {
            assert!(mkdir(&new(bad)).await.is_err(), "{bad} should have been refused");
        }
        // a folder that is not there is not a place to create in
        assert!(mkdir(&NewDir { path: base.join("nao-existe").display().to_string(), name: "x".into() })
            .await
            .is_err());
        // and a relative destination is not resolved anywhere
        assert!(mkdir(&NewDir { path: "stack".into(), name: "x".into() }).await.is_err());
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 0, "none of those may have created anything");

        let v = mkdir(&new(" filmes ")).await.unwrap();
        assert!(base.join("filmes").is_dir());
        assert_eq!(v["path"], json!(base.join("filmes").display().to_string()));
        // twice is a refusal, not a silent success over what is already there
        assert!(mkdir(&new("filmes")).await.is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}

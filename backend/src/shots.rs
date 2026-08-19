/* Cache of the theme.park palette screenshots.

   The repository redistributes nobody's screenshots: the images belong to the
   theme.park documentation, and that is where they come from the first time.
   What this module adds is the cache — the page opened from disk still fetches
   straight from their documentation, and with a server behind it the second
   visit to the same palette never leaves the machine.

   The entry point is `GET /api/shot/:app/:theme`, and the app/theme pair comes
   from the page's combobox. It comes from the browser, so it is checked here:
   only lowercase letters, digits and hyphens make it into the source URL and
   into the cache file name — that way neither does the path escape the folder
   nor does the address stop being theme.park's. */

use std::path::{Path, PathBuf};
use std::time::Duration;

const DOCS: &str = "https://docs.theme-park.dev";
/// Ceiling for the cache folder. Each screenshot is about 2.5 MB, and the
/// palettes of every app together would go past 200 MB — this keeps the last
/// two dozen, which is as many as anyone ever looks at in one sitting.
const CACHE_MAX: u64 = 64 * 1024 * 1024;

/// Does it fit in a file name and in a URL segment? It is the same alphabet as
/// the page's service ids and palette names — anything else is refused whole,
/// rather than sanitized, because there is no legitimate request outside it.
fn ok_segment(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The cache folder sits next to the database: it is server state, not stack
/// state, and wiping it only costs one extra fetch.
pub fn cache_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("shots")
}

/// The palette PNG, from the cache or from the theme.park documentation. When
/// the fetch fails the error bubbles up: the page already has the "could not
/// load" of `#shotErr`.
pub async fn fetch(dir: &Path, app: &str, theme: &str) -> Result<Vec<u8>, String> {
    if !ok_segment(app) || !ok_segment(theme) {
        return Err("invalid app or theme".into());
    }
    let file = dir.join(format!("{app}-{theme}.png"));
    if let Ok(b) = tokio::fs::read(&file).await {
        if !b.is_empty() {
            return Ok(b);
        }
    }

    let url = format!("{DOCS}/site_assets/{app}/{theme}.png");
    let r = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("{url}: {}", r.status()));
    }
    let body = r.bytes().await.map_err(|e| e.to_string())?.to_vec();
    if body.is_empty() {
        return Err(format!("{url}: resposta vazia"));
    }

    /* The cache is a convenience: if the write fails — folder with no permission,
       full disk — the screenshot is still served, and the next visit tries again. */
    if tokio::fs::create_dir_all(dir).await.is_ok() {
        let _ = tokio::fs::write(&file, &body).await;
        prune(dir, CACHE_MAX).await;
    }
    Ok(body)
}

/// Deletes the oldest screenshots until the folder fits in `max`. The order is
/// the one they were written in, not the one they were last used in: keeping an
/// LRU would cost a disk write on every cache hit, and what is lost in the worst
/// case is one fetch over the network.
///
/// Since the ceiling here is a convenience and not a guarantee, every error is
/// swallowed — the screenshot has already been served, and the next write tries
/// to prune again.
async fn prune(dir: &Path, max: u64) {
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else {
        return;
    };
    let mut files = Vec::new();
    let mut total = 0u64;
    while let Ok(Some(e)) = rd.next_entry().await {
        let Ok(m) = e.metadata().await else { continue };
        if !m.is_file() {
            continue;
        }
        total += m.len();
        files.push((m.modified().ok(), m.len(), e.path()));
    }
    if total <= max {
        return;
    }
    /* With no mtime it goes to the front of the queue: it is the one we can least defend. */
    files.sort_by_key(|(t, _, _)| *t);
    for (_, len, path) in files {
        if total <= max {
            break;
        }
        if tokio::fs::remove_file(&path).await.is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_a_segment_that_would_escape_the_folder_or_the_domain() {
        assert!(ok_segment("sonarr"));
        assert!(ok_segment("space-gray"));
        assert!(!ok_segment(""));
        assert!(!ok_segment(".."));
        assert!(!ok_segment("a/b"));
        assert!(!ok_segment("a.png"));
        assert!(!ok_segment("Sonarr"));
        assert!(!ok_segment("x?y=1"));
    }

    #[tokio::test]
    async fn serves_from_cache_without_hitting_the_network() {
        let dir = std::env::temp_dir().join(format!("hubshots{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("sonarr-nord.png"), b"png")
            .await
            .unwrap();
        assert_eq!(fetch(&dir, "sonarr", "nord").await.unwrap(), b"png");
        assert!(fetch(&dir, "..", "nord").await.is_err());
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn pruning_takes_the_oldest_shots_and_keeps_the_new_ones() {
        let dir = std::env::temp_dir().join(format!("hubpoda{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        for (i, name) in ["velha", "media", "nova"].iter().enumerate() {
            tokio::fs::write(dir.join(format!("sonarr-{name}.png")), vec![b'x'; 100])
                .await
                .unwrap();
            /* mtime has filesystem granularity: without the pause the three would land
               on the same instant and the order would be arbitrary. */
            if i < 2 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        prune(&dir, 250).await;
        assert!(!dir.join("sonarr-velha.png").exists());
        assert!(dir.join("sonarr-media.png").exists());
        assert!(dir.join("sonarr-nova.png").exists());

        prune(&dir, 10_000).await; // it fits already: nothing is deleted
        assert!(dir.join("sonarr-media.png").exists());
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}

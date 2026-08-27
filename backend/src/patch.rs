/* Keys written into the configuration the app itself created.

   qBittorrent keeps user, password and API key in an INI inside its config
   folder, and rewrites that whole file when it exits — mounting ours on top
   would freeze everything it writes there (torrents in progress, windows,
   preferences changed in the interface). So, instead of mounting, the server
   waits for the stack to come up and writes only the keys Hubstarr governs.

   Two things the order demands:

   - the container **stops** before the edit and comes back after. With it up,
     whatever we wrote would be overwritten on its next shutdown, which is when
     it dumps the in-memory configuration to disk;
   - the file may not exist yet on the first `up` — the app takes a few seconds
     to create it — so there is a short wait. If it still does not show up, the
     file is created with our keys and the app completes it later, which is what
     it does with an empty `/config`.

   What to write arrives ready from the page, section by section: only the INI
   format lives here. */

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::jobs::Log;
use crate::msg;
use crate::msg::Msg;

#[derive(Deserialize)]
pub struct Patch {
    /// the compose service to stop and start again around the edit
    pub service: String,
    /// path relative to `BASE_CONFIG`, as the page built it
    pub path: String,
    /// `ini` (the default) or `json`
    #[serde(default)]
    pub format: Option<String>,
    /// INI: `[Section]` → list of `key=value`, in the order the page wants them
    #[serde(default)]
    pub sections: Vec<(String, Vec<(String, String)>)>,
    /// What separates key from value in that app's file. The default is `=`, from
    /// the Qt INI qBittorrent uses; SABnzbd writes `key = value`, with the spaces,
    /// and that is how it reads back what is there — the one that knows this is
    /// the page, which owns the format of each file.
    #[serde(default)]
    pub sep: Option<String>,
    /* The keys that are **not** overwritten when the file already carries a
       value for them. It is qBittorrent's API key: once the app has one, that
       is the key its clients know, and swapping it on every Deploy would cut
       off whoever was already talking to it. A missing or empty key is still
       written — the first-deploy case. */
    #[serde(default)]
    pub keep: Vec<String>,
    /// JSON: the top-level keys to lay over the ones already there
    #[serde(default)]
    pub json: Option<Value>,
    /// XML: top-level elements. A text value becomes `<K>v</K>`; a list becomes
    /// `<K><string>a</string>…</K>`, which is how Jellyfin stores its own.
    #[serde(default)]
    pub xml: Option<Map<String, Value>>,
}

impl Patch {
    /// The new content of the file, starting from what was already in it.
    fn merge(&self, current: &str) -> Result<String, Msg> {
        match self.format.as_deref() {
            Some("json") => merge_json(current, self.json.as_ref().unwrap_or(&Value::Null)),
            Some("xml") => merge_xml(current, self.xml.as_ref().unwrap_or(&Map::new())),
            Some("yaml") => Ok(merge_yaml(
                current,
                &self.sections,
                self.sep.as_deref().unwrap_or(": "),
                &self.keep,
            )),
            _ => Ok(merge_ini(
                current,
                &self.sections,
                self.sep.as_deref().unwrap_or("="),
                &self.keep,
            )),
        }
    }

    /// How many keys this file receives — that is what goes to the log.
    fn keys(&self) -> usize {
        match self.format.as_deref() {
            Some("json") => self
                .json
                .as_ref()
                .and_then(|v| v.as_object())
                .map(|m| m.len())
                .unwrap_or(0),
            Some("xml") => self.xml.as_ref().map(|m| m.len()).unwrap_or(0),
            _ => self.sections.iter().map(|(_, v)| v.len()).sum(),
        }
    }
}

/// Same rule as `files::safe_join`: the path comes from the browser, so no
/// absolute paths and no `..`.
fn safe_join(dir: &Path, name: &str) -> Result<PathBuf, Msg> {
    if name.trim().is_empty() || name.contains('\\') {
        return Err(msg!("job.patch.invalidPath", name));
    }
    let mut out = dir.to_path_buf();
    for c in Path::new(name).components() {
        match c {
            Component::Normal(part) => out.push(part),
            _ => return Err(msg!("job.patch.invalidPath", name)),
        }
    }
    Ok(out)
}

/// Replaces in the INI the keys that came, leaving everything else where it
/// was: the other keys, the comments, the sections we do not know about and
/// even the order of the lines. A key that does not exist lands at the end of
/// its section; a section that does not exist lands at the end of the file.
pub fn merge_ini(
    current: &str,
    sections: &[(String, Vec<(String, String)>)],
    sep: &str,
    keep_keys: &[String],
) -> String {
    let mut lines: Vec<String> = current.lines().map(String::from).collect();

    for (secao, pares) in sections {
        let header = format!("[{secao}]");
        let start = lines.iter().position(|l| l.trim() == header);
        let (start, mut end) = match start {
            Some(i) => {
                // the section runs to the next header, or to the end
                let end = lines
                    .iter()
                    .enumerate()
                    .skip(i + 1)
                    .find(|(_, l)| l.trim().starts_with('[') && l.trim().ends_with(']'))
                    .map(|(j, _)| j)
                    .unwrap_or(lines.len());
                (i, end)
            }
            None => {
                // a new section: it lands at the end, separated by a blank line
                if lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                    lines.push(String::new());
                }
                lines.push(header);
                (lines.len() - 1, lines.len())
            }
        };

        for (key, value) in pares {
            let line = format!("{key}{sep}{value}");
            let found = lines[start + 1..end]
                .iter()
                .position(|l| key_of(l).as_deref() == Some(key.as_str()))
                .map(|k| start + 1 + k);
            match found {
                /* A key the app has already answered for: if it is in `keep_keys` and
                   has a value, it stays as it is. Empty does not count — that is
                   the line it leaves ready waiting for someone to fill in. */
                Some(k)
                    if keep_keys.iter().any(|m| m == key)
                        && value_of(&lines[k], sep).is_some_and(|v| !v.trim().is_empty()) => {}
                Some(k) => lines[k] = line,
                None => {
                    // at the end of the section, but before the blank lines that
                    // separate it from the next one
                    let mut pos = end;
                    while pos > start + 1 && lines[pos - 1].trim().is_empty() {
                        pos -= 1;
                    }
                    lines.insert(pos, line);
                    end += 1;
                }
            }
        }
    }

    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/* The same idea as the INI merge, in the two-level YAML the apps that dropped
   the INI now write — today Bazarr's `config.yaml`.

   It is not a YAML parser, and it does not need to be: what is governed here is
   a **top-level key with indented children**, which is the whole shape of
   `auth:` / `apikey:`. A section is a line with no indentation ending in `:`,
   and it runs until the next one; inside it, a child is matched by its own key,
   replaced in place with the indentation it already had, and a missing one
   lands at the end of the section with the indentation of its neighbours. What
   the app stored elsewhere — order, comments, every section we know nothing
   about — stays exactly as it was, because the file is the app's.

   A value is written plain: the keys that come through here are hexadecimal and
   paths, which YAML reads as strings with no quoting. Something that needed
   escaping would need this to grow first, and that is deliberate — quoting
   guessed wrong is a file the app refuses to load. */
pub fn merge_yaml(
    current: &str,
    sections: &[(String, Vec<(String, String)>)],
    sep: &str,
    keep_keys: &[String],
) -> String {
    let mut lines: Vec<String> = current.lines().map(String::from).collect();
    // a section header: no indentation, and nothing after the colon
    let is_header = |l: &str| {
        !l.starts_with(' ') && !l.starts_with('\t') && l.trim_end().ends_with(':') && !l.trim_start().starts_with('#')
    };

    for (section, pairs) in sections {
        let header = format!("{section}:");
        let (start, mut end) = match lines.iter().position(|l| l.trim_end() == header) {
            Some(i) => {
                let end = lines
                    .iter()
                    .enumerate()
                    .skip(i + 1)
                    .find(|(_, l)| !l.trim().is_empty() && is_header(l))
                    .map(|(j, _)| j)
                    .unwrap_or(lines.len());
                (i, end)
            }
            None => {
                lines.push(header);
                (lines.len() - 1, lines.len())
            }
        };
        // the indentation the app itself used inside this section, two spaces otherwise
        let indent = lines[start + 1..end]
            .iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| l[..l.len() - l.trim_start().len()].to_string())
            .filter(|i| !i.is_empty())
            .unwrap_or_else(|| "  ".to_string());

        for (key, value) in pairs {
            let found = lines[start + 1..end]
                .iter()
                .position(|l| l.trim_start().split(':').next().map(str::trim) == Some(key.as_str()))
                .map(|k| start + 1 + k);
            match found {
                // the same rule as the INI's: a key the app has answered for stays
                Some(k)
                    if keep_keys.iter().any(|m| m == key)
                        && lines[k]
                            .split_once(':')
                            .is_some_and(|(_, v)| !v.trim().is_empty()) => {}
                Some(k) => {
                    let own = lines[k][..lines[k].len() - lines[k].trim_start().len()].to_string();
                    lines[k] = format!("{own}{key}{sep}{value}");
                }
                None => {
                    let mut pos = end;
                    while pos > start + 1 && lines[pos - 1].trim().is_empty() {
                        pos -= 1;
                    }
                    lines.insert(pos, format!("{indent}{key}{sep}{value}"));
                    end += 1;
                }
            }
        }
    }

    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Lays our top-level keys over the ones already in the file, leaving the
/// others where they were — in `categories.json`, that is what preserves the
/// category someone created in the app's interface.
///
/// An unreadable file is no reason to stop: the app rewrites it whole next
/// time, and what matters is that ours gets there.
/* The elements Hubstarr governs inside a configuration XML — today Jellyfin's
   `network.xml`.

   It is not an XML parser: it is the same idea as the INI merge, line by line.
   An element that already exists is replaced in place, with its indentation;
   what is missing lands before the root's closing tag. All the rest of the file
   — order, comments, whatever the app stored — stays as it was, because the app
   is the one that owns it.

   It only holds for a top-level element whose value fits on one line, which is
   the case of `BaseUrl`. A list becomes `<K><string>a</string>…</K>`, in the
   shape Jellyfin writes them. */
pub fn merge_xml(current: &str, ours: &Map<String, Value>) -> Result<String, Msg> {
    if ours.is_empty() {
        return Ok(current.to_string());
    }
    let mut lines: Vec<String> = current.lines().map(String::from).collect();

    for (key, value) in ours {
        let body = match value {
            Value::Array(items) => items
                .iter()
                .map(|i| format!("<string>{}</string>", esc_xml(&text_of(i))))
                .collect::<String>(),
            v => esc_xml(&text_of(v)),
        };

        let open_tag = format!("<{key}>");
        let empty = format!("<{key} />");
        let found = lines.iter().position(|l| {
            let t = l.trim_start();
            t.starts_with(&open_tag) || t.starts_with(&empty)
        });
        match found {
            Some(i) => {
                // the element may be opened on one line and closed on another:
                // whatever is between the two goes along
                let level: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();
                let close_tag = format!("</{key}>");
                let end = lines[i..]
                    .iter()
                    .position(|l| l.contains(&close_tag))
                    .map(|k| i + k)
                    .unwrap_or(i);
                lines.splice(i..=end, [format!("{level}<{key}>{body}</{key}>")]);
            }
            None => {
                // it lands before the root closes, with the indentation of whoever is already there
                let end = lines
                    .iter()
                    .rposition(|l| l.trim_start().starts_with("</"))
                    .unwrap_or(lines.len());
                lines.insert(end, format!("  <{key}>{body}</{key}>"));
            }
        }
    }
    let mut out = lines.join("\n");
    if current.ends_with('\n') || !current.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

/// The text of a JSON value: a string without the quotes, the rest as it came.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        outro => outro.to_string(),
    }
}

fn esc_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn merge_json(current: &str, ours: &Value) -> Result<String, Msg> {
    let mut obj: Map<String, Value> = serde_json::from_str(current).unwrap_or_default();
    let ours = ours.as_object().ok_or_else(|| Msg::k("job.patch.jsonNotObject"))?;
    for (k, v) in ours {
        obj.insert(k.clone(), v.clone());
    }
    /* Four spaces, which is how qBittorrent writes this file and how the page's
       tab shows it: following its style avoids a whole-file diff every time one
       of the two writes. */
    let mut buf = Vec::new();
    let indent = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, indent);
    Value::Object(obj)
        .serialize(&mut ser)
        .map_err(|e| msg!("job.patch.jsonEncodeError", e.to_string()))?;
    let mut txt = String::from_utf8(buf).map_err(|e| msg!("job.patch.jsonEncodeError", e.to_string()))?;
    txt.push('\n');
    Ok(txt)
}

/// The key of a `key=value` line, ignoring comments and blank lines.
/// The value of a `key=value` line, to know whether the app has answered for it.
/// The separator is that file's own, but the INI `=` serves as a fallback: it
/// is what splits the line in either of the two formats we generate.
fn value_of(line: &str, sep: &str) -> Option<String> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with('[') {
        return None;
    }
    l.split_once(sep.trim())
        .or_else(|| l.split_once('='))
        .map(|(_, v)| v.trim().to_string())
}

fn key_of(line: &str) -> Option<String> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with('[') {
        return None;
    }
    l.split_once('=').map(|(k, _)| k.trim().to_string())
}

/// Waits for the app to create its configuration. When it does not create it
/// in time, the file is born right here, with only our keys — which is what it
/// completes on the next deploy.
async fn wait(path: &Path, log: &Log) {
    for attempt in 0..30 {
        if let Ok(txt) = tokio::fs::read_to_string(path).await {
            if !txt.trim().is_empty() {
                return;
            }
        }
        if attempt == 0 {
            log.line(msg!("job.patch.waiting", path.display().to_string()));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    log.line(Msg::k("job.patch.timeout"));
}

/// Writes what the page sent, one service at a time: the container stops once,
/// all of its files are merged, and it comes back up. Stopping per file would
/// mean two cycles for qBittorrent, which has two files.
pub async fn apply_all(
    docker: &str,
    dir: &Path,
    cfg: Option<&Path>,
    patches: &[Patch],
    log: &Log,
) -> Result<(), Msg> {
    if patches.is_empty() {
        return Ok(());
    }
    let root = cfg.ok_or_else(|| Msg::k("job.patch.noBaseConfig"))?;

    // in the order the page sent them, grouped by service
    let mut services: Vec<&str> = Vec::new();
    for p in patches {
        if !services.contains(&p.service.as_str()) {
            services.push(&p.service);
        }
    }

    for service in services {
        let ours: Vec<&Patch> = patches.iter().filter(|p| p.service == service).collect();
        let paths: Vec<PathBuf> = ours
            .iter()
            .map(|p| safe_join(root, &p.path))
            .collect::<Result<_, _>>()?;

        /* Wait once per service, for the first file: it is the sign that the app
           came up and created its config folder. The second file may legitimately
           not exist — qBittorrent only writes `categories.json` when it has a
           category. */
        wait(&paths[0], log).await;
        crate::deploy::compose(docker, &["stop", service], dir, log).await?;
        for (p, path) in ours.iter().zip(&paths) {
            let current = tokio::fs::read_to_string(path).await.unwrap_or_default();
            let new_value = p.merge(&current)?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| msg!("job.patch.mkdirError", parent.display().to_string(), e.to_string()))?;
            }
            tokio::fs::write(path, &new_value).await.map_err(|e| {
                // the file belongs to the container: if it created it with another owner,
                // the server does not rewrite it — it is the Environment's PUID/PGID that
                // makes the two match
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    msg!("job.patch.writeErrorPerm", path.display().to_string(), e.to_string())
                } else {
                    msg!("job.patch.writeError", path.display().to_string(), e.to_string())
                }
            })?;
            crate::journal::detail(|| {
                format!(
                    "file {} ({} bytes, keys written into the app's conf)",
                    path.display(),
                    new_value.len()
                )
            });
            log.line(msg!("job.patch.written", path.display().to_string(), p.keys().to_string()));
        }
        crate::deploy::compose(docker, &["start", service], dir, log).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sections() -> Vec<(String, Vec<(String, String)>)> {
        vec![(
            "Preferences".into(),
            vec![
                ("WebUI\\Username".into(), "admin".into()),
                ("WebUI\\APIKey".into(), "qbt_novo".into()),
            ],
        )]
    }

    #[test]
    fn replaces_the_existing_key_and_leaves_the_rest_alone() {
        let current = "[BitTorrent]\nSession\\Port=6881\n\n[Preferences]\nWebUI\\Username=velho\nWebUI\\Port=8181\n";
        let new_value = merge_ini(current, &sections(), "=", &[]);
        assert!(new_value.contains("WebUI\\Username=admin"));
        assert!(!new_value.contains("velho"));
        // what belongs to the app stays where it was
        assert!(new_value.contains("Session\\Port=6881"));
        assert!(new_value.contains("WebUI\\Port=8181"));
    }

    #[test]
    fn a_missing_key_lands_in_its_own_section() {
        let current = "[Preferences]\nWebUI\\Username=velho\n\n[Network]\nProxy\\Type=0\n";
        let new_value = merge_ini(current, &sections(), "=", &[]);
        let lines: Vec<&str> = new_value.lines().collect();
        let i = lines.iter().position(|l| l.starts_with("WebUI\\APIKey")).unwrap();
        let secs = lines.iter().position(|l| *l == "[Preferences]").unwrap();
        let next = lines.iter().position(|l| *l == "[Network]").unwrap();
        assert!(i > secs && i < next, "a chave nova caiu fora da seção: {new_value}");
        assert!(new_value.contains("Proxy\\Type=0"));
    }

    /* The API key the app already has stays as it is: once it answers for it,
       that is the key its clients know, and swapping it on every Deploy would
       cut off whoever was already talking to it. */
    #[test]
    fn a_kept_key_is_not_overwritten() {
        let current = "[Preferences]\nWebUI\\APIKey=qbt_do_proprio_app\nWebUI\\Port=8080\n";
        let keep_keys = vec!["WebUI\\APIKey".to_string()];
        let new_value = merge_ini(current, &sections(), "=", &keep_keys);
        assert!(new_value.contains("WebUI\\APIKey=qbt_do_proprio_app"), "{new_value}");
        assert!(!new_value.contains("qbt_novo"), "{new_value}");
        // the rest goes on being written normally
        assert!(new_value.contains("WebUI\\Username=admin"), "{new_value}");
    }

    /// A key that exists but is empty is the line the app leaves waiting for
    /// someone to fill in — that one we do fill.
    #[test]
    fn an_empty_kept_key_is_filled_in() {
        let current = "[Preferences]\nWebUI\\APIKey=\n";
        let keep_keys = vec!["WebUI\\APIKey".to_string()];
        let new_value = merge_ini(current, &sections(), "=", &keep_keys);
        assert!(new_value.contains("WebUI\\APIKey=qbt_novo"), "{new_value}");
    }

    #[test]
    fn a_missing_section_lands_at_the_end() {
        let new_value = merge_ini("[BitTorrent]\nSession\\Port=6881\n", &sections(), "=", &[]);
        assert!(new_value.contains("[Preferences]"));
        assert!(new_value.trim_end().ends_with("WebUI\\APIKey=qbt_novo"));
    }

    #[test]
    fn an_empty_file_is_born_with_only_our_keys() {
        let new_value = merge_ini("", &sections(), "=", &[]);
        assert_eq!(
            new_value,
            "[Preferences]\nWebUI\\Username=admin\nWebUI\\APIKey=qbt_novo\n"
        );
    }

    #[test]
    fn applying_again_neither_duplicates_nor_reorders() {
        let current = "[Preferences]\nWebUI\\Username=admin\nWebUI\\APIKey=qbt_novo\n";
        assert_eq!(merge_ini(current, &sections(), "=", &[]), current);
    }

    #[test]
    fn comments_and_blank_lines_survive() {
        let current = "# escrito pelo app\n[Preferences]\nWebUI\\Username=velho\n";
        let new_value = merge_ini(current, &sections(), "=", &[]);
        assert!(new_value.starts_with("# escrito pelo app\n"));
    }

    /// The separator is no decoration: SABnzbd writes `key = value`, with the
    /// spaces, and that is the shape in which it reads the file back. Writing
    /// `key=value` there left the key where the app cannot find it.
    #[test]
    fn sabnzbd_keeps_a_space_on_both_sides_of_the_equals() {
        let current = "[misc]\nhost_whitelist = velho,\ninet_exposure = 0\n";
        let sections = vec![(
            "misc".to_string(),
            vec![
                ("host_whitelist".to_string(), "sabnzbd,localhost".to_string()),
                ("api_key".to_string(), "abc123".to_string()),
            ],
        )];
        let new_value = merge_ini(current, &sections, " = ", &[]);
        assert!(new_value.contains("host_whitelist = sabnzbd,localhost"));
        assert!(new_value.contains("api_key = abc123"));
        // what was already there, and did not come in the patch, stays as it was
        assert!(new_value.contains("inet_exposure = 0"));
        // and the default is still the Qt one, with no spaces
        let qt = merge_ini("[BitTorrent]\n", &sections, "=", &[]);
        assert!(qt.contains("api_key=abc123"));
    }

    /* Bazarr's `config.yaml`, which is what the YAML merge exists for. The
       shape is the real one: a top-level key with indented children, several
       sections, and things we know nothing about that have to survive. */
    fn bazarr() -> Vec<(String, Vec<(String, String)>)> {
        vec![(
            "auth".into(),
            vec![("apikey".into(), "0123456789abcdef0123456789abcdef".into())],
        )]
    }

    const BAZARR_YAML: &str = "\
analytics:
  enabled: true
auth:
  apikey: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  type: null
  username: ''
general:
  port: 6767
";

    #[test]
    fn the_yaml_replaces_the_key_and_leaves_the_rest_of_the_file_alone() {
        let out = merge_yaml(BAZARR_YAML, &bazarr(), ": ", &[]);
        assert!(out.contains("  apikey: 0123456789abcdef0123456789abcdef"));
        // the neighbours of the section, and the sections around it
        assert!(out.contains("  type: null"));
        assert!(out.contains("  username: ''"));
        assert!(out.contains("analytics:\n  enabled: true"));
        assert!(out.contains("general:\n  port: 6767"));
        assert!(!out.contains("aaaaaaaa"));
    }

    /// The real pair Bazarr receives: the key and the base URL, in two
    /// different sections of the same file, one existing and one to be created.
    #[test]
    fn the_yaml_writes_both_sections_bazarr_gets() {
        let ours = vec![
            ("auth".to_string(), vec![("apikey".to_string(), "0123456789abcdef0123456789abcdef".to_string())]),
            ("general".to_string(), vec![("base_url".to_string(), "/bazarr".to_string())]),
        ];
        let out = merge_yaml(BAZARR_YAML, &ours, ": ", &[]);
        assert!(out.contains("  apikey: 0123456789abcdef0123456789abcdef"));
        assert!(out.contains("  base_url: /bazarr"));
        // the base URL joins the section the app already had, next to its port
        assert!(out.contains("general:\n  port: 6767\n  base_url: /bazarr"), "{out}");
        assert_eq!(out, merge_yaml(&out, &ours, ": ", &[]));
    }

    #[test]
    fn a_missing_yaml_key_lands_in_its_own_section() {
        let without = "auth:\n  type: null\ngeneral:\n  port: 6767\n";
        let out = merge_yaml(without, &bazarr(), ": ", &[]);
        // inside `auth`, not after `general`
        let key = out.find("apikey").unwrap();
        assert!(key < out.find("general:").unwrap());
        assert!(out.contains("  apikey: 0123456789abcdef0123456789abcdef"));
    }

    #[test]
    fn a_missing_yaml_section_lands_at_the_end() {
        let out = merge_yaml("general:\n  port: 6767\n", &bazarr(), ": ", &[]);
        assert!(out.contains("auth:\n  apikey: 0123456789abcdef0123456789abcdef"));
        assert!(out.contains("general:\n  port: 6767"));
    }

    /// The app owns the file, so its own indentation is the one that is kept —
    /// a four-space file does not come back mixed.
    #[test]
    fn the_yaml_keeps_the_indentation_the_app_used() {
        let four = "auth:\n    type: null\n";
        let out = merge_yaml(four, &bazarr(), ": ", &[]);
        assert!(out.contains("    apikey: 0123456789abcdef0123456789abcdef"), "{out}");
        assert!(out.contains("    type: null"));
    }

    #[test]
    fn applying_the_yaml_again_neither_duplicates_nor_reorders() {
        let once = merge_yaml(BAZARR_YAML, &bazarr(), ": ", &[]);
        assert_eq!(once, merge_yaml(&once, &bazarr(), ": ", &[]));
        assert_eq!(once.matches("apikey").count(), 1);
    }

    #[test]
    fn a_kept_yaml_key_is_not_overwritten_but_an_empty_one_is() {
        let out = merge_yaml(BAZARR_YAML, &bazarr(), ": ", &["apikey".to_string()]);
        assert!(out.contains("  apikey: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let empty = merge_yaml("auth:\n  apikey:\n", &bazarr(), ": ", &["apikey".to_string()]);
        assert!(empty.contains("  apikey: 0123456789abcdef0123456789abcdef"));
    }

    #[test]
    fn an_empty_yaml_file_is_born_with_only_our_keys() {
        let out = merge_yaml("", &bazarr(), ": ", &[]);
        assert_eq!(out, "auth:\n  apikey: 0123456789abcdef0123456789abcdef\n");
    }

    #[test]
    fn the_json_writes_our_categories_and_leaves_the_others_alone() {
        let current = r#"{"minha":{"save_path":"/downloads/minha"},"tv-sonarr":{"save_path":"/velho"}}"#;
        let ours = serde_json::json!({"tv-sonarr": {"save_path": "/downloads/torrents/tv-sonarr"}});
        let new_value: Value = serde_json::from_str(&merge_json(current, &ours).unwrap()).unwrap();
        // the category created in the app's interface stays
        assert_eq!(new_value["minha"]["save_path"], "/downloads/minha");
        // ours wins for the one with the same name
        assert_eq!(new_value["tv-sonarr"]["save_path"], "/downloads/torrents/tv-sonarr");
        // and the indentation is the app's, not serde's
        assert!(merge_json(current, &ours).unwrap().contains("\n    \"minha\""));
    }

    #[test]
    fn unreadable_or_empty_json_does_not_break_the_write() {
        let ours = serde_json::json!({"tv": {"save_path": "/downloads/tv"}});
        for current in ["", "nem json é", "[]"] {
            let new_value: Value = serde_json::from_str(&merge_json(current, &ours).unwrap()).unwrap();
            assert_eq!(new_value["tv"]["save_path"], "/downloads/tv", "veio de {current:?}");
        }
    }

    #[test]
    fn the_format_picks_the_merge_and_ini_is_the_default() {
        let ini = Patch { service: "qbittorrent".into(), path: "x".into(), format: None,
                          sections: sections(), sep: None, json: None, xml: None, keep: vec![] };
        assert!(ini.merge("").unwrap().contains("[Preferences]"));
        assert_eq!(ini.keys(), 2);

        let js = Patch { service: "qbittorrent".into(), path: "x".into(),
                         format: Some("json".into()), sections: vec![], sep: None, xml: None, keep: vec![],
                         json: Some(serde_json::json!({"tv": {"save_path": "/d"}})) };
        assert!(js.merge("{}").unwrap().contains("\"tv\""));
        assert_eq!(js.keys(), 1);
    }

    #[test]
    fn a_path_escaping_the_folder_is_refused() {
        let dir = Path::new("/tmp/x");
        assert!(safe_join(dir, "qbittorrent/qBittorrent/qBittorrent.conf").is_ok());
        assert!(safe_join(dir, "../fora.conf").is_err());
        assert!(safe_join(dir, "/etc/passwd").is_err());
        assert!(safe_join(dir, "").is_err());
    }

    /// Jellyfin's `network.xml`: what is already there is replaced in place, what
    /// is missing lands before the root closes, and the rest of the file is untouched.
    #[test]
    fn the_xml_replaces_what_exists_and_adds_what_is_missing() {
        let current = "<?xml version=\"1.0\"?>\n<NetworkConfiguration>\n  \
                     <EnableIPv6>true</EnableIPv6>\n  <BaseUrl></BaseUrl>\n\
                     </NetworkConfiguration>\n";
        let mut ours = Map::new();
        ours.insert("BaseUrl".into(), Value::String("/jellyfin".into()));
        ours.insert(
            "KnownProxies".into(),
            Value::Array(vec![Value::String("nginx".into())]),
        );
        let new_value = merge_xml(current, &ours).unwrap();
        assert!(new_value.contains("<BaseUrl>/jellyfin</BaseUrl>"));
        assert!(new_value.contains("<KnownProxies><string>nginx</string></KnownProxies>"));
        // what the app stored stays
        assert!(new_value.contains("<EnableIPv6>true</EnableIPv6>"));
        // and it does not duplicate: applying again gives the same file
        assert_eq!(merge_xml(&new_value, &ours).unwrap(), new_value);
        assert_eq!(new_value.matches("<BaseUrl>").count(), 1);
    }

    /// An element spanning several lines — as Jellyfin writes lists — is replaced
    /// whole, without leaving half of it behind.
    #[test]
    fn the_xml_replaces_a_multiline_element_whole() {
        let current = "<Config>\n  <KnownProxies>\n    <string>velho</string>\n  \
                     </KnownProxies>\n</Config>\n";
        let mut ours = Map::new();
        ours.insert(
            "KnownProxies".into(),
            Value::Array(vec![Value::String("nginx".into())]),
        );
        let new_value = merge_xml(current, &ours).unwrap();
        assert!(new_value.contains("<KnownProxies><string>nginx</string></KnownProxies>"));
        assert!(!new_value.contains("velho"));
        assert_eq!(new_value.matches("KnownProxies").count(), 2);   // opens and closes, once
    }
}

/* Estado da página entre sessões.

   Guarda o JSON como a página o mandou (`added`, `DEFAULTS`, `CONFIG`), sem
   olhar dentro: quem entende dessa forma é o script, e ela muda com ele. O
   arquivo fica junto dos gerados, para a stack inteira caber numa pasta só. */

use std::path::{Path, PathBuf};

use serde_json::Value;

const NAME: &str = "hubstarr.json";

pub fn path(dir: &Path) -> PathBuf {
    dir.join(NAME)
}

pub async fn save(dir: &Path, state: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    // grava ao lado e renomeia: um Ctrl-C no meio não deixa o arquivo pela metade
    let tmp = path(dir).with_extension("json.tmp");
    tokio::fs::write(&tmp, text)
        .await
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    tokio::fs::rename(&tmp, path(dir))
        .await
        .map_err(|e| format!("{}: {e}", path(dir).display()))
}

pub async fn load(dir: &Path) -> Result<Option<Value>, String> {
    let p = path(dir);
    if !p.exists() {
        return Ok(None);
    }
    let text = tokio::fs::read_to_string(&p)
        .await
        .map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{}: {e}", e))
}

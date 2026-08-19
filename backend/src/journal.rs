/* What the server writes: the output and `servidor.log`.

   Two heights. The **normal** one is what always comes out — the startup, the
   container engine chosen, every state write coming from the page. The
   **detailed** one, which only exists with `-v`, is the step by step: every
   file written, every row touched in the database and every API call to an app
   of the stack.

   The difference is on purpose. The normal one answers "what changed in my
   stack?", and so it is short enough to be read days later; the detailed one
   answers "why did this not work?", and for that it has to say everything the
   server touched, in order. Turning the second one on by default would drown
   the first — one round of Apply is dozens of calls.

   Both go to the same place: the output **and** the file, next to the database.
   The output alone gets lost (whoever starts it through `systemd` or in a
   session that closed has nowhere to look afterwards), and afterwards is when
   we look. The file **appends**, never rewrites: the history across restarts is
   what gives it value. And a log that fails does not bring the server down —
   at worst we are back to the output only. */

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static VERBOSE: AtomicBool = AtomicBool::new(false);

/// The log belongs to the server, not to the stack: it sits next to the
/// database, which is what lasts — the `--dir` folder is wiped and remade.
pub fn path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("servidor.log")
}

pub fn open(db_path: &Path) {
    let p = path(db_path);
    match std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        Ok(f) => {
            let _ = LOG.set(Mutex::new(f));
        }
        // without the file the server stays whole, just without history
        Err(e) => println!("(no log at {}: {e})", p.display()),
    }
}

pub fn set_detail(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub fn detailed() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn record(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    println!("{msg}");
    if let Some(f) = LOG.get() {
        if let Ok(mut f) = f.lock() {
            let _ = writeln!(f, "{msg}");
        }
    }
}

/* One step of the way, only with `-v`. It goes indented and with the stamp, so
   it can be told apart at a glance from the normal lines when the two mix in
   the same file.

   The argument is a **function**, not the finished message: without `-v`
   nothing is formatted, and so this can be called from inside hot loops without
   paying for text nobody will read. */
pub fn detail(msg: impl FnOnce() -> String) {
    if detailed() {
        record(format!("  · {} {}", stamp(), msg()));
    }
}

/// The time in UTC, `2026-08-13 22:41:07Z`, with no extra crate — the log only
/// needs this, and a dependency to format a date would cost dearly in the CI's `--locked`.
pub fn stamp() -> String {
    stamp_of(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
}

fn stamp_of(s: u64) -> String {
    let (dias, hora) = (s / 86_400, s % 86_400);
    // days since 1970 → civil date, Howard Hinnant's algorithm
    let z = dias as i64 + 719_468;
    let was = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = was * 400 + yoe + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z",
        hora / 3600,
        (hora % 3600) / 60,
        hora % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The log belongs to the server, not to the stack: it sits next to the
    /// database, which is what lasts — the `--dir` folder is wiped and remade.
    #[test]
    fn the_log_sits_next_to_the_database() {
        assert_eq!(
            path(Path::new("/home/x/.hubstarr/hubstarr.db")),
            Path::new("/home/x/.hubstarr/servidor.log")
        );
        // a database with no folder in the path: the log stays in the current directory
        assert_eq!(path(Path::new("hubstarr.db")), Path::new("servidor.log"));
    }

    #[test]
    fn the_log_stamp_is_the_date_in_utc() {
        assert_eq!(stamp_of(0), "1970-01-01 00:00:00Z");
        // checked against `date -u -d @1786992067`
        assert_eq!(stamp_of(1_786_992_067), "2026-08-17 18:41:07Z");
        // leap year, the 29th of February that date maths usually loses
        assert_eq!(stamp_of(1_709_164_800), "2024-02-29 00:00:00Z");
    }

    /// Without `-v` the message is never even built — that is what keeps
    /// `detail()` cheap inside a loop.
    #[test]
    fn without_verbose_the_message_is_not_built() {
        set_detail(false);
        let mut built = false;
        detail(|| {
            built = true;
            String::new()
        });
        assert!(!built);
    }
}

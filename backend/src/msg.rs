/*! A message meant for the browser: an I18N key plus its arguments, in the
    shape `t()` on the page already expects for a function-valued string. The
    wording for each key lives in `hubstarr.html`'s `I18N`, in the three
    languages the page speaks — never here, so nothing user-facing gets typed
    once in Portuguese on this side and then again, differently, on that one.

    An argument can itself be a `Msg`: the page resolves it first, through the
    same `t()`, and splices the *translated* text into the outer one — which is
    what lets "Sonarr → SABnzbd: <the reason>" compose out of an outer template
    and whatever inner one produced the reason, without Rust ever holding
    translated prose for either half. */

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Arg {
    /// Opaque text: a name, a path, a number already turned to string, or raw
    /// text from something we do not author — a `reqwest` error, the OS, the
    /// body of another app's own HTTP response. Shown exactly as it arrives.
    Text(String),
    /// A nested, still-translatable message.
    Msg(Msg),
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Msg {
    pub key: &'static str,
    pub args: Vec<Arg>,
}

impl Msg {
    pub fn new(key: &'static str, args: Vec<Arg>) -> Self {
        Self { key, args }
    }

    /// A key with no arguments.
    pub fn k(key: &'static str) -> Self {
        Self { key, args: Vec::new() }
    }

    /// Escape hatch for text that truly has no template of its own — shown
    /// as-is, in whatever language it already happens to be in. Only ever
    /// meant for the migration away from the old `Log::line(String)`; a
    /// leftover call after that migration is a message someone forgot to give
    /// a real key.
    pub fn raw(text: impl Into<String>) -> Self {
        Self { key: "raw", args: vec![Arg::Text(text.into())] }
    }
}

impl From<String> for Arg {
    fn from(s: String) -> Self {
        Arg::Text(s)
    }
}
impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Arg::Text(s.to_string())
    }
}
impl From<Msg> for Arg {
    fn from(m: Msg) -> Self {
        Arg::Msg(m)
    }
}

// Numbers only ever show up as text — an HTTP status, a count, a byte size.
macro_rules! arg_from_display {
    ($($t:ty),+ $(,)?) => {
        $( impl From<$t> for Arg { fn from(v: $t) -> Self { Arg::Text(v.to_string()) } } )+
    };
}
arg_from_display!(u16, u32, u64, usize, i64);

// Migration aid, not a design to keep leaning on: a call site still holding a
// plain `String`/`&str` (or a `format!()`) compiles as-is, landing under the
// `raw` key. Once every site is converted, `grep -rn '"raw"' src/` should come
// back empty.
impl From<String> for Msg {
    fn from(s: String) -> Self {
        Msg::raw(s)
    }
}
impl From<&str> for Msg {
    fn from(s: &str) -> Self {
        Msg::raw(s.to_string())
    }
}
impl From<&String> for Msg {
    fn from(s: &String) -> Self {
        Msg::raw(s.clone())
    }
}

/// Builds a `Msg` tersely: `msg!("job.arr.registered", arr.name.clone(), client.name.clone())`.
/// Each argument goes through `Into<Arg>`, so a `String`, a number, or another
/// `Msg` (for nesting) all just work.
#[macro_export]
macro_rules! msg {
    ($key:expr) => {
        $crate::msg::Msg::new($key, ::std::vec![])
    };
    ($key:expr, $($arg:expr),+ $(,)?) => {
        $crate::msg::Msg::new($key, ::std::vec![$(::std::convert::Into::<$crate::msg::Arg>::into($arg)),+])
    };
}

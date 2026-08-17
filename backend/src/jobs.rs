/* Long-running jobs.

   Bringing the stack up pulls images and configuring the *arr apps waits for
   them to answer: both go beyond what fits in an HTTP response. So each one
   becomes a numbered job, the page asks about the number and shows the log as
   it grows. */

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

#[derive(Serialize, Clone, Default)]
pub struct JobState {
    /// finished already?
    pub done: bool,
    /// finished well? only means anything together with `done`
    pub ok: bool,
    pub log: Vec<String>,
}

/// The end the job uses to write to the log while it runs.
#[derive(Clone)]
pub struct Log(Arc<Mutex<JobState>>);

impl Log {
    pub fn line(&self, s: impl Into<String>) {
        if let Ok(mut st) = self.0.lock() {
            st.log.push(s.into());
        }
    }
}

pub struct Jobs {
    next: AtomicU64,
    map: Mutex<HashMap<u64, Arc<Mutex<JobState>>>>,
    /* The end through which Stop kills the job. It belongs to the *inner* task,
       the one running `f`: aborting it makes the outer `await` receive a
       cancellation `JoinError`, and the job ends as an ordinary failure — the
       `done` gets written and Close comes back, by the same path as a panic. */
    stop: Mutex<HashMap<u64, tokio::task::AbortHandle>>,
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            map: Mutex::new(HashMap::new()),
            stop: Mutex::new(HashMap::new()),
        }
    }

    /// Sets the job running and returns its number right away.
    pub fn spawn<F, Fut>(&self, f: F) -> u64
    where
        F: FnOnce(Log) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let slot = Arc::new(Mutex::new(JobState::default()));
        if let Ok(mut m) = self.map.lock() {
            m.insert(id, slot.clone());
        }
        let log = Log(slot.clone());
        /* The job runs in a task of its own only so that `done` is **always**
           written: a panic in there would kill the task before that line, and
           the job would stay forever "running" — on the page side, that is the
           log modal with Close disabled and nothing else happening. Waiting for
           the `JoinHandle` turns the panic into an ordinary failure, with a
           line in the log — and it is the same path as Stop, which aborts that
           inner task. */
        let inside = log.clone();
        let internal = tokio::spawn(async move { f(inside).await });
        if let Ok(mut s) = self.stop.lock() {
            s.insert(id, internal.abort_handle());
        }
        tokio::spawn(async move {
            let res = match internal.await {
                Ok(r) => r,
                Err(e) if e.is_cancelled() => Err("parado a pedido".to_string()),
                Err(e) => Err(format!("o trabalho morreu no meio ({e})")),
            };
            if let Err(e) = &res {
                log.line(format!("erro: {e}"));
            }
            if let Ok(mut st) = slot.lock() {
                st.ok = res.is_ok();
                st.done = true;
            }
        });
        id
    }

    /// A loose log end, for tests: nobody reads what it writes.
    #[cfg(test)]
    pub fn test_log(&self) -> Log {
        Log(Arc::new(Mutex::new(JobState::default())))
    }

    /// Kills the job. Returns `false` for a number it does not know; for one that
    /// already finished it does nothing, and that is not an error — Stop arrives
    /// when it arrives. Whoever was running a `docker compose` takes the process
    /// along, through `kill_on_drop` in `deploy.rs`.
    pub fn stop(&self, id: u64) -> bool {
        let Ok(s) = self.stop.lock() else { return false };
        match s.get(&id) {
            Some(h) => {
                h.abort();
                true
            }
            None => false,
        }
    }

    pub fn get(&self, id: u64) -> Option<JobState> {
        let m = self.map.lock().ok()?;
        let slot = m.get(&id)?;
        slot.lock().ok().map(|st| st.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn wait(jobs: &Jobs, id: u64) -> JobState {
        for _ in 0..100 {
            let st = jobs.get(id).expect("o trabalho existe");
            if st.done {
                return st;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("o trabalho nunca terminou");
    }

    #[tokio::test]
    async fn a_job_that_fails_ends_as_a_failure() {
        let jobs = Jobs::new();
        let id = jobs.spawn(|log| async move {
            log.line("indo");
            Err("não deu".to_string())
        });
        let st = wait(&jobs, id).await;
        assert!(!st.ok);
        assert_eq!(st.log, vec!["indo", "erro: não deu"]);
    }

    /// A panic inside the job was the case that used to trap the log modal:
    /// without `done`, the page keeps asking forever and Close never comes back.
    #[tokio::test]
    async fn a_job_that_panics_also_ends() {
        let jobs = Jobs::new();
        let id = jobs.spawn(|_log| async move {
            panic!("estourou");
        });
        let st = wait(&jobs, id).await;
        assert!(!st.ok);
        assert!(st.log.iter().any(|l| l.contains("morreu no meio")), "{:?}", st.log);
    }

    /// Stop: the job ends as a failure, and that is what gives back the log
    /// modal's Close.
    #[tokio::test]
    async fn a_stopped_job_ends_as_a_failure() {
        let jobs = Jobs::new();
        let id = jobs.spawn(|log| async move {
            log.line("indo");
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok(())
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(jobs.stop(id));
        let st = wait(&jobs, id).await;
        assert!(!st.ok);
        assert!(st.log.iter().any(|l| l.contains("parado a pedido")), "{:?}", st.log);
        assert!(!jobs.stop(999), "número que ele não conhece");
    }
}

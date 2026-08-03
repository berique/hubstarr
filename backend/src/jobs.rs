/* Trabalhos de longa duração.

   Subir a stack baixa imagem e configurar os *arr espera app responder: os
   dois passam do que cabe numa resposta HTTP. Então cada um vira um trabalho
   com número, a página pergunta pelo número e vai mostrando o log. */

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

#[derive(Serialize, Clone, Default)]
pub struct JobState {
    /// já terminou?
    pub done: bool,
    /// terminou bem? só quer dizer alguma coisa com `done`
    pub ok: bool,
    pub log: Vec<String>,
}

/// A ponta que o trabalho usa para escrever no log enquanto roda.
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
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            map: Mutex::new(HashMap::new()),
        }
    }

    /// Põe o trabalho para rodar e devolve o número dele na hora.
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
        tokio::spawn(async move {
            let res = f(log.clone()).await;
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

    pub fn get(&self, id: u64) -> Option<JobState> {
        let m = self.map.lock().ok()?;
        let slot = m.get(&id)?;
        slot.lock().ok().map(|st| st.clone())
    }
}

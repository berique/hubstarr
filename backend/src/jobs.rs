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
            /* O trabalho roda numa tarefa própria só para que o `done` seja
               escrito **sempre**: um pânico lá dentro mataria a tarefa antes
               desta linha, e o trabalho ficaria eternamente "correndo" — do
               lado da página, isso é o modal do log com o Fechar desabilitado
               e nada mais acontecendo. Esperar pelo `JoinHandle` transforma o
               pânico numa falha comum, com uma linha no log. */
            let dentro = log.clone();
            let res = match tokio::spawn(async move { f(dentro).await }).await {
                Ok(r) => r,
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

    /// Uma ponta de log solta, para teste: ninguém lê o que ela escreve.
    #[cfg(test)]
    pub fn log_de_teste(&self) -> Log {
        Log(Arc::new(Mutex::new(JobState::default())))
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

    async fn esperar(jobs: &Jobs, id: u64) -> JobState {
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
    async fn trabalho_que_falha_termina_como_falha() {
        let jobs = Jobs::new();
        let id = jobs.spawn(|log| async move {
            log.line("indo");
            Err("não deu".to_string())
        });
        let st = esperar(&jobs, id).await;
        assert!(!st.ok);
        assert_eq!(st.log, vec!["indo", "erro: não deu"]);
    }

    /// Pânico dentro do trabalho é o caso que prendia o modal do log: sem o
    /// `done`, a página fica perguntando para sempre e o Fechar nunca volta.
    #[tokio::test]
    async fn trabalho_que_entra_em_panico_tambem_termina() {
        let jobs = Jobs::new();
        let id = jobs.spawn(|_log| async move {
            panic!("estourou");
        });
        let st = esperar(&jobs, id).await;
        assert!(!st.ok);
        assert!(st.log.iter().any(|l| l.contains("morreu no meio")), "{:?}", st.log);
    }
}

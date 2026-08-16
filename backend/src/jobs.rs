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
    /* A ponta pela qual o Parar mata o trabalho. É da tarefa *de dentro*, a
       que roda o `f`: abortá-la faz o `await` de fora receber um `JoinError`
       de cancelamento, e o trabalho termina como falha comum — o `done` sai
       escrito e o Fechar volta, pelo mesmo caminho do pânico. */
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
        /* O trabalho roda numa tarefa própria só para que o `done` seja
           escrito **sempre**: um pânico lá dentro mataria a tarefa antes
           dessa linha, e o trabalho ficaria eternamente "correndo" — do
           lado da página, isso é o modal do log com o Fechar desabilitado
           e nada mais acontecendo. Esperar pelo `JoinHandle` transforma o
           pânico numa falha comum, com uma linha no log — e é o mesmo
           caminho do Parar, que aborta essa tarefa de dentro. */
        let dentro = log.clone();
        let interna = tokio::spawn(async move { f(dentro).await });
        if let Ok(mut s) = self.stop.lock() {
            s.insert(id, interna.abort_handle());
        }
        tokio::spawn(async move {
            let res = match interna.await {
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

    /// Uma ponta de log solta, para teste: ninguém lê o que ela escreve.
    #[cfg(test)]
    pub fn log_de_teste(&self) -> Log {
        Log(Arc::new(Mutex::new(JobState::default())))
    }

    /// Mata o trabalho. Devolve `false` para número que ele não conhece; para
    /// um que já terminou não faz nada, e isso não é erro — o Parar chega
    /// quando chega. Quem estava rodando um `docker compose` leva o processo
    /// junto, pelo `kill_on_drop` do `deploy.rs`.
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

    /// O Parar: o trabalho termina como falha, e é isso que devolve o Fechar
    /// do modal do log.
    #[tokio::test]
    async fn trabalho_parado_termina_como_falha() {
        let jobs = Jobs::new();
        let id = jobs.spawn(|log| async move {
            log.line("indo");
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok(())
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(jobs.stop(id));
        let st = esperar(&jobs, id).await;
        assert!(!st.ok);
        assert!(st.log.iter().any(|l| l.contains("parado a pedido")), "{:?}", st.log);
        assert!(!jobs.stop(999), "número que ele não conhece");
    }
}

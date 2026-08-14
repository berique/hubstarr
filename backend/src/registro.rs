/* O que o servidor escreve: a saída e o `servidor.log`.

   Duas alturas. O **normal** é o que sempre sai — a subida, o motor de
   container escolhido, cada gravação de estado vinda da página. O
   **detalhado**, que só existe com `-v`, é o passo a passo: cada arquivo
   gravado, cada linha mexida no banco e cada chamada de API a um app da stack.

   A diferença é de propósito. O normal responde "o que mudou na minha stack?",
   e por isso é curto o bastante para se ler dias depois; o detalhado responde
   "por que isso não funcionou?", e para isso ele precisa dizer tudo o que o
   servidor tocou, na ordem. Ligar o segundo por padrão afogaria o primeiro —
   uma volta de Aplicar são dezenas de chamadas.

   Os dois vão para o mesmo lugar: a saída **e** o arquivo, ao lado do banco. A
   saída sozinha se perde (quem sobe por `systemd` ou numa sessão que fechou não
   tem onde olhar depois), e é depois que a gente olha. O arquivo **acrescenta**,
   nunca reescreve: o histórico entre reinícios é o que dá valor a ele. E o log
   que falha não derruba o servidor — no máximo se volta a ter só a saída. */

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static VERBOSO: AtomicBool = AtomicBool::new(false);

/// O log é do servidor, não da stack: ele fica ao lado do banco, que é o que
/// dura — a pasta do `--dir` se apaga e se refaz.
pub fn caminho(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("servidor.log")
}

pub fn abrir(db_path: &Path) {
    let p = caminho(db_path);
    match std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        Ok(f) => {
            let _ = LOG.set(Mutex::new(f));
        }
        // sem o arquivo o servidor continua inteiro, só sem histórico
        Err(e) => println!("(sem log em {}: {e})", p.display()),
    }
}

pub fn ligar_detalhe(on: bool) {
    VERBOSO.store(on, Ordering::Relaxed);
}

pub fn detalhado() -> bool {
    VERBOSO.load(Ordering::Relaxed)
}

pub fn registra(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    println!("{msg}");
    if let Some(f) = LOG.get() {
        if let Ok(mut f) = f.lock() {
            let _ = writeln!(f, "{msg}");
        }
    }
}

/* Um passo do caminho, só com o `-v`. Vai recuado e com o carimbo, para se
   distinguir de relance das linhas normais quando os dois se misturam no mesmo
   arquivo.

   O argumento é uma **função**, e não a mensagem pronta: sem o `-v` nada é
   formatado, e assim dá para chamar isto de dentro de laços quentes sem pagar
   por texto que ninguém vai ler. */
pub fn detalhe(msg: impl FnOnce() -> String) {
    if detalhado() {
        registra(format!("  · {} {}", carimbo(), msg()));
    }
}

/// A hora em UTC, `2026-08-13 22:41:07Z`, sem crate a mais — o log só precisa
/// disso, e uma dependência para formatar data sairia caro no `--locked` do CI.
pub fn carimbo() -> String {
    carimbo_de(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
}

fn carimbo_de(s: u64) -> String {
    let (dias, hora) = (s / 86_400, s % 86_400);
    // dias desde 1970 → data civil, o algoritmo do Howard Hinnant
    let z = dias as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = era * 400 + yoe + i64::from(m <= 2);
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

    /// O log é do servidor, não da stack: ele fica ao lado do banco, que é o
    /// que dura — a pasta do `--dir` se apaga e se refaz.
    #[test]
    fn o_log_fica_ao_lado_do_banco() {
        assert_eq!(
            caminho(Path::new("/home/x/.hubstarr/hubstarr.db")),
            Path::new("/home/x/.hubstarr/servidor.log")
        );
        // banco sem pasta nenhuma no caminho: o log fica no diretório atual
        assert_eq!(caminho(Path::new("hubstarr.db")), Path::new("servidor.log"));
    }

    #[test]
    fn o_carimbo_do_log_e_a_data_em_utc() {
        assert_eq!(carimbo_de(0), "1970-01-01 00:00:00Z");
        // conferido com `date -u -d @1786992067`
        assert_eq!(carimbo_de(1_786_992_067), "2026-08-17 18:41:07Z");
        // ano bissexto, o 29 de fevereiro que os cálculos de data costumam perder
        assert_eq!(carimbo_de(1_709_164_800), "2024-02-29 00:00:00Z");
    }

    /// Sem o `-v`, a mensagem nem chega a ser montada — é o que deixa o
    /// `detalhe()` barato dentro de laço.
    #[test]
    fn sem_o_verbose_a_mensagem_nao_e_montada() {
        ligar_detalhe(false);
        let mut montou = false;
        detalhe(|| {
            montou = true;
            String::new()
        });
        assert!(!montou);
    }
}

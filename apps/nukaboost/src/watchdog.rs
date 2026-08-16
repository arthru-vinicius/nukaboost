//! Watchdog de processo (seção 9 do plano): `NukaBoost.exe --watchdog <pid>
//! <session-id>`.
//!
//! Este arquivo cobre as duas pontas do watchdog: [`spawn`], chamado pelo
//! processo principal durante a ativação (seção 7, passo 9) para lançar o
//! processo filho, e [`run`], executado dentro desse processo filho — o
//! próprio `NukaBoost.exe` relançado com `--watchdog`.

use uuid::Uuid;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_FAILED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateProcessW, OpenProcess, TerminateProcess, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE,
    PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, STARTUPINFOW,
};

use nukaboost_core::error::{NukaError, NukaResult};
use nukaboost_core::recovery::{self, RecoveryJournal};
use nukaboost_win32::wide::to_wide_null;

/// Alça do processo watchdog desta sessão, mantida pelo `ActiveSession` do
/// orquestrador para poder desarmá-lo na desativação (seção 8, passo 10).
pub struct WatchdogHandle {
    process: HANDLE,
}

// SAFETY: `HANDLE` é um identificador de kernel opaco; seguro entre threads.
unsafe impl Send for WatchdogHandle {}

impl WatchdogHandle {
    /// Encerra o processo watchdog desta sessão. O watchdog não tem
    /// recursos próprios para limpar (só observa), então terminação
    /// abrupta é segura.
    pub fn terminate(&self) -> NukaResult<()> {
        // SAFETY: `self.process` foi obtido de `CreateProcessW` em `spawn`
        // e permanece válido até este `WatchdogHandle` ser descartado.
        let wait = unsafe { WaitForSingleObject(self.process, 0) };
        if wait == WAIT_FAILED {
            return Err(NukaError::from_win32(
                "WaitForSingleObject(watchdog)",
                &windows::core::Error::from_thread(),
            ));
        }
        if wait != WAIT_OBJECT_0 {
            unsafe { TerminateProcess(self.process, 0) }
                .map_err(|e| NukaError::from_win32("TerminateProcess(watchdog)", &e))?;
            let wait = unsafe { WaitForSingleObject(self.process, INFINITE) };
            if wait == WAIT_FAILED {
                return Err(NukaError::from_win32(
                    "WaitForSingleObject(watchdog termination)",
                    &windows::core::Error::from_thread(),
                ));
            }
        }
        Ok(())
    }
}

impl Drop for WatchdogHandle {
    fn drop(&mut self) {
        // SAFETY: `self.process` é fechado exatamente uma vez, aqui.
        unsafe {
            let _ = CloseHandle(self.process);
        }
    }
}

/// Nome sob o qual a cópia do executável usada pelo watchdog é gravada,
/// ao lado do `NukaBoost.exe` principal (ver [`watchdog_exe_path`]).
const WATCHDOG_COPY_NAME: &str = "NukaBoostWatchdog.exe";

/// Caminho do executável a lançar para o watchdog: uma cópia do próprio
/// `NukaBoost.exe` sob outro nome, sempre que possível.
///
/// Isso existe para que matar o processo principal pelo nome — pelo
/// Gerenciador de Tarefas, por `taskkill /IM NukaBoost.exe`, ou por um
/// agente automatizado varrendo processos por nome de imagem — não derrube
/// o watchdog junto. Nesse caso o watchdog continua vivo sob outro nome,
/// nota a morte do processo principal e restaura o plano de energia na
/// hora, sem esperar o próximo logon.
///
/// A cópia é refeita a cada ativação (nunca fica desatualizada depois de
/// uma atualização do aplicativo) e é best-effort: se falhar por qualquer
/// motivo (por exemplo, um watchdog de uma sessão anterior ainda travando o
/// arquivo), cai de volta ao próprio executável — o comportamento de hoje,
/// que já funciona — em vez de impedir a ativação.
fn watchdog_exe_path(main_exe: &std::path::Path) -> std::path::PathBuf {
    let copy_path = main_exe.with_file_name(WATCHDOG_COPY_NAME);
    match std::fs::copy(main_exe, &copy_path) {
        Ok(_) => copy_path,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to create a separately named watchdog copy; falling back to the main executable"
            );
            main_exe.to_path_buf()
        }
    }
}

/// Lança `<watchdog-exe> --watchdog <main_pid> <session_id>` como processo
/// filho, sem janela de console. Ver [`watchdog_exe_path`] para por que o
/// executável lançado normalmente não é o próprio `NukaBoost.exe`.
pub fn spawn(main_pid: u32, session_id: Uuid) -> NukaResult<WatchdogHandle> {
    let main_exe = std::env::current_exe().map_err(|e| NukaError::io("<current_exe>", e))?;
    let exe = watchdog_exe_path(&main_exe);
    let command_line = format!("\"{}\" --watchdog {main_pid} {session_id}", exe.display());
    let mut command_line_wide = to_wide_null(&command_line);

    let startup_info = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process_info = PROCESS_INFORMATION::default();

    // SAFETY: `command_line_wide` vive até o fim desta chamada síncrona;
    // `startup_info`/`process_info` são structs válidas na pilha.
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(command_line_wide.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &startup_info,
            &mut process_info,
        )
    }
    .map_err(|e| NukaError::from_win32("CreateProcessW(--watchdog)", &e))?;

    // SAFETY: `hThread` foi retornado por `CreateProcessW` acima; não é
    // usado, então é fechado imediatamente para não vazar o handle.
    unsafe {
        let _ = CloseHandle(process_info.hThread);
    }

    Ok(WatchdogHandle {
        process: process_info.hProcess,
    })
}

/// Corpo do modo `--watchdog <pid> <session-id>`.
///
/// Espera o processo principal terminar e, se ele morreu inesperadamente
/// (o journal da mesma sessão ainda existir), restaura o plano de energia
/// original. Se o processo principal se desarmou normalmente (journal já
/// limpo, ou pertencente a uma sessão mais nova), não faz nada.
pub fn run(main_pid: u32, session_id: Uuid) -> NukaResult<()> {
    let Some(handle) = open_process_to_wait(main_pid, session_id)? else {
        // PID já não corresponde ao processo que criou esta sessão (já
        // encerrou, ou o PID foi reciclado) — nada a observar.
        return recovery::perform_recovery(Some(session_id)).map(|_| ());
    };

    // SAFETY: `handle` é válido; espera indefinidamente o processo terminar.
    unsafe {
        WaitForSingleObject(handle, INFINITE);
    }
    // SAFETY: `handle` foi aberto por `open_process_to_wait` e ainda não
    // foi fechado.
    unsafe {
        let _ = CloseHandle(handle);
    }

    recovery::perform_recovery(Some(session_id)).map(|_| ())
}

/// Abre o processo principal para espera, validando primeiro (via o
/// journal) que o PID ainda corresponde ao mesmo processo que criou esta
/// sessão — a defesa contra reutilização de PID da seção 9.
fn open_process_to_wait(main_pid: u32, session_id: Uuid) -> NukaResult<Option<HANDLE>> {
    let Some(journal) = RecoveryJournal::load()? else {
        return Ok(None);
    };
    if journal.session_id != session_id || journal.main_pid != main_pid {
        return Ok(None);
    }
    if !journal.main_process_still_running() {
        return Ok(None);
    }

    // SAFETY: `main_pid` é validado acima; `OpenProcess` falha com segurança
    // se o processo já não existir.
    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            main_pid,
        )
    }
    .ok();

    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_exe_path_copies_next_to_the_main_exe() {
        let dir = tempfile::tempdir().unwrap();
        let main_exe = dir.path().join("Main.exe");
        std::fs::write(&main_exe, b"stub").unwrap();

        let watchdog_path = watchdog_exe_path(&main_exe);

        assert_eq!(watchdog_path, dir.path().join(WATCHDOG_COPY_NAME));
        assert_eq!(std::fs::read(&watchdog_path).unwrap(), b"stub");
    }

    #[test]
    fn watchdog_exe_path_falls_back_when_copying_is_impossible() {
        // Um diretório-pai inexistente faz `fs::copy` falhar de forma
        // previsível sem precisar simular um arquivo em uso.
        let missing_main_exe = std::path::Path::new("Z:\\definitely\\missing\\Main.exe");

        let watchdog_path = watchdog_exe_path(missing_main_exe);

        assert_eq!(watchdog_path, missing_main_exe);
    }
}

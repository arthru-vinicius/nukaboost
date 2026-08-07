//! Inicialização sob demanda de `NukaBoost.exe` (seção 11 do plano):
//!
//! > Se `start` for executado sem processo principal: 1. Iniciar
//! > `NukaBoost.exe`. 2. Esperar o Named Pipe. 3. Solicitar ativação.
//! > 4. Falhar se o aviso de segurança ainda não estiver confirmado.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nukaboost_core::error::{NukaError, NukaResult};
use nukaboost_core::ipc::protocol::{Command, Outcome};

use crate::client;

const WAIT_FOR_PIPE_TIMEOUT: Duration = Duration::from_secs(10);
const WAIT_FOR_PIPE_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Garante que o processo principal esteja em execução — iniciando-o se
/// necessário e aguardando o Named Pipe responder — e então envia `command`.
pub fn ensure_running_and_send(command: Command) -> NukaResult<Outcome> {
    match client::send(command.clone()) {
        Err(NukaError::ProcessNotRunning) => {}
        other => return other,
    }

    launch_main_process()?;
    wait_for_pipe()?;
    client::send(command)
}

fn launch_main_process() -> NukaResult<()> {
    let ctl_path = std::env::current_exe().map_err(|e| NukaError::io("<current_exe>", e))?;
    let main_exe = sibling_exe_path(&ctl_path, "NukaBoost.exe")?;

    std::process::Command::new(main_exe)
        .spawn()
        .map(|_child| ())
        .map_err(|e| NukaError::io("NukaBoost.exe", e))
}

fn sibling_exe_path(ctl_path: &Path, name: &str) -> NukaResult<PathBuf> {
    let dir = ctl_path
        .parent()
        .ok_or_else(|| NukaError::Other("nukaboostctl.exe has no parent directory".into()))?;
    Ok(dir.join(name))
}

fn wait_for_pipe() -> NukaResult<()> {
    let deadline = Instant::now() + WAIT_FOR_PIPE_TIMEOUT;
    loop {
        match client::send(Command::Status) {
            Ok(_) => return Ok(()),
            Err(NukaError::ProcessNotRunning) if Instant::now() < deadline => {
                std::thread::sleep(WAIT_FOR_PIPE_POLL_INTERVAL);
            }
            Err(NukaError::ProcessNotRunning) => {
                return Err(NukaError::Other(
                    "timed out waiting for NukaBoost to start".into(),
                ));
            }
            // O pipe já responde, mesmo que este "ping" específico tenha
            // retornado outro erro de negócio — o objetivo aqui é só
            // confirmar que o processo está de pé.
            Err(_) => return Ok(()),
        }
    }
}

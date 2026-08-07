//! Cliente do Named Pipe do NukaBoost (seção 11 do plano).
//!
//! Deliberadamente implementado só com `std::fs` — o Windows expõe Named
//! Pipes como parte do namespace de arquivos, então um cliente não precisa
//! da crate `windows` para se conectar (só o **servidor**, em
//! `apps/nukaboost`, precisa de `CreateNamedPipeW` para controlar o
//! descritor de segurança e o modo de mensagem).

use std::fs::OpenOptions;
use std::io;
use std::thread::sleep;
use std::time::Duration;

use nukaboost_core::error::{NukaError, NukaResult};
use nukaboost_core::ipc::protocol::{Command, Outcome};
use nukaboost_core::ipc::{self, Envelope};

/// Código de erro Win32 `ERROR_PIPE_BUSY`: todas as instâncias do pipe
/// estão ocupadas no momento — vale a pena tentar de novo em instantes,
/// diferente de "o pipe não existe" (processo não está rodando).
const ERROR_PIPE_BUSY: i32 = 231;

/// Número de tentativas antes de desistir quando o pipe está ocupado.
const BUSY_RETRY_ATTEMPTS: u32 = 10;
const BUSY_RETRY_DELAY: Duration = Duration::from_millis(50);

/// Envia `command` ao processo principal e retorna a resposta.
///
/// Retorna [`NukaError::ProcessNotRunning`] se o pipe não existir — o
/// processo principal não está em execução. O chamador decide se deve
/// tentar iniciá-lo (ver `launch::ensure_running`).
pub fn send(command: Command) -> NukaResult<Outcome> {
    let pipe_path = ipc::pipe_path()?;

    let mut last_error = None;
    for attempt in 0..=BUSY_RETRY_ATTEMPTS {
        match OpenOptions::new().read(true).write(true).open(&pipe_path) {
            Ok(mut file) => {
                let request = Envelope::new(command);
                ipc::write_message(&mut file, &request)?;
                let response: Envelope<Outcome> = ipc::read_message(&mut file)?;
                ipc::check_protocol_version(response.protocol_version)?;
                if response.request_id != request.request_id {
                    return Err(NukaError::Other(
                        "IPC response request_id does not match the request".into(),
                    ));
                }
                return Ok(response.body);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(NukaError::ProcessNotRunning);
            }
            Err(e)
                if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempt < BUSY_RETRY_ATTEMPTS =>
            {
                last_error = Some(e);
                sleep(BUSY_RETRY_DELAY);
            }
            Err(e) => return Err(NukaError::io(&pipe_path, e)),
        }
    }

    Err(NukaError::io(
        &pipe_path,
        last_error.unwrap_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "pipe ocupado")),
    ))
}

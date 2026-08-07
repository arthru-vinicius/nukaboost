//! Thread dedicada de `SetThreadExecutionState` (seção 6, "Solicitações
//! redundantes de execução") — a terceira via, independente do plano
//! temporário e das Power Requests, para sinalizar atividade ao Windows.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

use crate::error::{NukaError, NukaResult};

/// Controla uma thread dedicada que mantém `ES_CONTINUOUS | ES_SYSTEM_REQUIRED`
/// em vigor enquanto o NukaBoost estiver ativo.
///
/// `SetThreadExecutionState` só afeta a thread que a chamou — se ela
/// terminasse, o estado voltaria ao padrão do sistema. Por isso a chamada
/// vive em uma thread própria e dedicada, mantida viva por um canal que
/// bloqueia até o sinal de parada. Nunca é definido `ES_DISPLAY_REQUIRED`
/// nem `ES_AWAYMODE_REQUIRED` — o NukaBoost nunca mantém a tela ligada.
pub struct ExecutionStateThread {
    stop_tx: mpsc::Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl ExecutionStateThread {
    /// Inicia a thread, que imediatamente aplica
    /// `ES_CONTINUOUS | ES_SYSTEM_REQUIRED` e permanece bloqueada até que
    /// [`ExecutionStateThread::stop`] (ou `Drop`) seja chamado.
    pub fn start() -> NukaResult<Self> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<NukaResult<()>>(1);

        let handle = thread::Builder::new()
            .name("nukaboost-execution-state".into())
            .spawn(move || {
                // SAFETY: `SetThreadExecutionState` não tem pré-condições
                // além de ser chamada por uma thread viva, o que é garantido
                // pelo próprio corpo desta closure.
                let previous =
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
                if previous.0 == 0 {
                    let error = windows::core::Error::from_thread();
                    let _ = ready_tx.send(Err(NukaError::from_win32(
                        "SetThreadExecutionState(ES_SYSTEM_REQUIRED)",
                        &error,
                    )));
                    return;
                }
                let _ = ready_tx.send(Ok(()));
                // Bloqueia aqui: a thread precisa continuar viva durante
                // todo o tempo em que o estado acima deve valer.
                let _ = stop_rx.recv();
                // SAFETY: mesma justificativa acima; restaura o
                // comportamento padrão do sistema antes de encerrar.
                let restored = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
                if restored.0 == 0 {
                    tracing::error!("SetThreadExecutionState(ES_CONTINUOUS) failed during cleanup");
                }
            })
            .map_err(|e| {
                NukaError::Other(format!("failed to start execution-state thread: {e}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = handle.join();
                return Err(error);
            }
            Err(error) => {
                let _ = handle.join();
                return Err(NukaError::Other(format!(
                    "execution-state thread exited before initialization: {error}"
                )));
            }
        }

        Ok(Self {
            stop_tx,
            handle: Some(handle),
        })
    }

    /// Verdadeiro se a thread ainda está viva (não terminou de forma
    /// inesperada). Usado pelo monitoramento periódico (seção 10).
    pub fn is_alive(&self) -> bool {
        self.handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }

    /// Sinaliza a thread para restaurar `ES_CONTINUOUS` e aguarda sua
    /// conclusão. Corresponde ao passo 7 da desativação (seção 8): "Encerrar
    /// a thread de execution state". Também é chamado automaticamente por
    /// `Drop`, mas é exposto para permitir aguardar a conclusão de forma
    /// determinística durante a sequência de desativação.
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ExecutionStateThread {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn starts_and_stops_cleanly() {
        let thread = ExecutionStateThread::start().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(thread.is_alive());
        thread.stop();
    }
}

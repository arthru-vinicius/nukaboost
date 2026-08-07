//! Monitoramento periódico durante o estado ativo (seção 10 do plano).
//!
//! Foca no que é efetivamente reaplicável quando algo muda por fora (Group
//! Policy, outro aplicativo, o próprio usuário mexendo no Painel de
//! Controle): as oito configurações do plano temporário. A validade dos
//! handles de Power Request e a vivacidade da thread de execution state não
//! têm um caminho de "reaplicar" — se algum dia se mostrarem instáveis na
//! prática, a resposta correta é recriar a sessão inteira, não remendar uma
//! peça isolada; por ora, sua presença contínua no `ActiveSession` já é a
//! garantia (`Drop` os limpa, e uma queda do processo aciona o watchdog).

use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use windows::core::GUID;

use crate::orchestrator::{Orchestrator, MONITOR_INTERVAL};

/// Inicia a thread de monitoramento do plano temporário `scheme`. Retorna
/// um canal para sinalizar parada e o `JoinHandle` correspondente — ambos
/// guardados no `ActiveSession` e usados na desativação (seção 8, passo 3:
/// "Parar o monitor" antes de restaurar o plano original).
pub fn spawn(orchestrator: Arc<Orchestrator>, scheme: GUID) -> (mpsc::Sender<()>, JoinHandle<()>) {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let handle = thread::Builder::new()
        .name("nukaboost-monitor".into())
        .spawn(move || loop {
            match stop_rx.recv_timeout(MONITOR_INTERVAL) {
                Ok(()) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            if !orchestrator.monitor_protections_healthy(scheme) {
                if orchestrator.state() != nukaboost_core::state::State::Active {
                    break;
                }
                // A desativação aguarda o monitor terminar. Dispará-la em
                // outra thread evita que o monitor tente dar `join` em si
                // mesmo e entre em deadlock.
                let orchestrator = Arc::clone(&orchestrator);
                if let Err(error) = thread::Builder::new()
                    .name("nukaboost-monitor-recovery".into())
                    .spawn(move || orchestrator.handle_protection_failure())
                {
                    tracing::error!(%error, "failed to start monitor recovery worker");
                }
                break;
            }
        })
        .expect("failed to start the monitor thread");

    (stop_tx, handle)
}

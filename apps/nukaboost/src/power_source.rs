//! Leitura do estado de energia do sistema (fonte, bateria, Economia de
//! Bateria) para `status --json` (seção 11) e para o monitor (seção 10).

use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use nukaboost_core::ipc::protocol::PowerSource;

/// Lê a fonte de energia atual, a porcentagem de bateria (se disponível) e
/// se a Economia de Bateria está ativa. Em caso de falha da API, assume AC
/// sem bateria — um padrão conservador que nunca bloqueia indevidamente.
pub fn read() -> (PowerSource, Option<u8>, bool) {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` é um ponteiro de saída válido na pilha.
    if unsafe { GetSystemPowerStatus(&mut status) }.is_err() {
        return (PowerSource::Ac, None, false);
    }

    // `ACLineStatus`: 0 = bateria, 1 = tomada, 255 = desconhecido.
    let source = if status.ACLineStatus == 1 {
        PowerSource::Ac
    } else {
        PowerSource::Battery
    };
    // `BatteryLifePercent`: 0–100, ou 255 se desconhecido/sem bateria.
    let percent = (status.BatteryLifePercent <= 100).then_some(status.BatteryLifePercent);
    // `SystemStatusFlag`: 0 = Economia de Bateria desligada, 1 = ligada.
    let battery_saver = status.SystemStatusFlag != 0;

    (source, percent, battery_saver)
}

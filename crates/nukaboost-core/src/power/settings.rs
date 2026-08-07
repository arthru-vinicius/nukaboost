//! Leitura e escrita das quatro configurações do plano temporário, com
//! confirmação obrigatória por releitura.
//!
//! A tabela da seção 6 do plano exige aplicar e verificar cada configuração
//! tanto em AC (tomada) quanto em DC (bateria); o passo 6 da transação de
//! ativação (seção 7) exige "ler todos os valores novamente e confirmar"
//! antes de prosseguir. As duas exigências se encontram em
//! [`apply_and_confirm`].

use windows::core::GUID;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Power::{
    PowerReadACValueIndex, PowerReadDCValueIndex, PowerWriteACValueIndex, PowerWriteDCValueIndex,
};

use super::{check, guids};
use crate::error::{NukaError, NukaResult};

/// Valor desejado para `LIDACTION`: `0` — não fazer nada ao fechar a tampa.
pub const LIDACTION_VALUE: u32 = 0;
/// Valor desejado para `STANDBYIDLE`: `0` — nunca suspender por inatividade.
pub const STANDBYIDLE_VALUE: u32 = 0;
/// Valor desejado para `SYSTEMREQUIRED`: `1` — aceitar solicitações de apps.
pub const SYSTEMREQUIRED_VALUE: u32 = 1;
/// Valor desejado para `BATACTIONLOW`: `0` — não suspender em bateria baixa
/// (a ação de bateria **crítica** não é tocada — ver seção 2 do plano).
pub const BATACTIONLOW_VALUE: u32 = 0;

/// Lê o valor de uma configuração de energia no lado AC (conectado à tomada).
pub fn read_ac_value(scheme: GUID, subgroup: GUID, setting: GUID) -> NukaResult<u32> {
    let mut value = 0u32;
    // SAFETY: os três GUIDs de entrada vivem durante toda a chamada
    // síncrona; `value` é um ponteiro de saída válido de um `u32` na pilha.
    let code = unsafe {
        PowerReadACValueIndex(
            None,
            Some(&scheme),
            Some(&subgroup),
            Some(&setting),
            &mut value,
        )
    };
    check(code, "PowerReadACValueIndex")?;
    Ok(value)
}

/// Lê o valor de uma configuração de energia no lado DC (bateria).
pub fn read_dc_value(scheme: GUID, subgroup: GUID, setting: GUID) -> NukaResult<u32> {
    let mut value = 0u32;
    // SAFETY: mesmo contrato de `read_ac_value`.
    let code = unsafe {
        PowerReadDCValueIndex(
            None,
            Some(&scheme),
            Some(&subgroup),
            Some(&setting),
            &mut value,
        )
    };
    // A assinatura gerada para `PowerReadDCValueIndex` retorna `u32` bruto
    // em vez de `WIN32_ERROR` — peculiaridade dos metadados desta função
    // específica — mas o valor continua sendo um código de erro Win32 padrão.
    check(WIN32_ERROR(code), "PowerReadDCValueIndex")?;
    Ok(value)
}

/// Escreve o valor de uma configuração de energia no lado AC.
pub fn write_ac_value(scheme: GUID, subgroup: GUID, setting: GUID, value: u32) -> NukaResult<()> {
    // SAFETY: mesmo contrato de `read_ac_value`.
    let code =
        unsafe { PowerWriteACValueIndex(None, &scheme, Some(&subgroup), Some(&setting), value) };
    check(code, "PowerWriteACValueIndex")
}

/// Escreve o valor de uma configuração de energia no lado DC.
pub fn write_dc_value(scheme: GUID, subgroup: GUID, setting: GUID, value: u32) -> NukaResult<()> {
    // SAFETY: mesmo contrato de `read_ac_value`.
    let code =
        unsafe { PowerWriteDCValueIndex(None, &scheme, Some(&subgroup), Some(&setting), value) };
    check(WIN32_ERROR(code), "PowerWriteDCValueIndex")
}

/// Escreve `value` em AC e DC e confirma imediatamente por releitura,
/// retornando [`NukaError::ProtectionNotConfirmed`] se qualquer um dos dois
/// lados não refletir o valor esperado.
fn write_and_confirm(
    scheme: GUID,
    subgroup: GUID,
    setting: GUID,
    value: u32,
    name_ac: &'static str,
    name_dc: &'static str,
) -> NukaResult<()> {
    write_ac_value(scheme, subgroup, setting, value)?;
    write_dc_value(scheme, subgroup, setting, value)?;

    let confirmed_ac = read_ac_value(scheme, subgroup, setting)?;
    if confirmed_ac != value {
        return Err(NukaError::ProtectionNotConfirmed {
            setting: name_ac,
            expected: value,
            actual: confirmed_ac,
        });
    }

    let confirmed_dc = read_dc_value(scheme, subgroup, setting)?;
    if confirmed_dc != value {
        return Err(NukaError::ProtectionNotConfirmed {
            setting: name_dc,
            expected: value,
            actual: confirmed_dc,
        });
    }

    Ok(())
}

/// Aplica as quatro configurações da tabela da seção 6 (em AC e DC) sobre o
/// plano informado — que deve ser o plano **temporário**, nunca o original
/// do usuário — e confirma cada uma por releitura antes de prosseguir.
pub fn apply_and_confirm(scheme: GUID) -> NukaResult<()> {
    write_and_confirm(
        scheme,
        guids::SUB_BUTTONS,
        guids::LIDACTION,
        LIDACTION_VALUE,
        "LIDACTION/AC",
        "LIDACTION/DC",
    )?;
    write_and_confirm(
        scheme,
        guids::SUB_SLEEP,
        guids::STANDBYIDLE,
        STANDBYIDLE_VALUE,
        "STANDBYIDLE/AC",
        "STANDBYIDLE/DC",
    )?;
    write_and_confirm(
        scheme,
        guids::SUB_SLEEP,
        guids::SYSTEMREQUIRED,
        SYSTEMREQUIRED_VALUE,
        "SYSTEMREQUIRED/AC",
        "SYSTEMREQUIRED/DC",
    )?;
    write_and_confirm(
        scheme,
        guids::SUB_BATTERY,
        guids::BATACTIONLOW,
        BATACTIONLOW_VALUE,
        "BATACTIONLOW/AC",
        "BATACTIONLOW/DC",
    )?;
    Ok(())
}

/// Fotografia do estado observado das quatro proteções do plano temporário,
/// usada tanto pelo monitoramento periódico (seção 10) quanto para montar o
/// objeto `protections` de `nukaboostctl status --json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemeProtectionStatus {
    pub lid_action_ac: bool,
    pub lid_action_dc: bool,
    pub idle_sleep_ac: bool,
    pub idle_sleep_dc: bool,
    pub system_required_ac: bool,
    pub system_required_dc: bool,
    pub low_battery_action_ac: bool,
    pub low_battery_action_dc: bool,
}

impl SchemeProtectionStatus {
    /// Verdadeiro somente se todas as oito verificações passaram. Usado
    /// pelo monitor (seção 10) para decidir se uma reaplicação é necessária.
    pub const fn all_ok(&self) -> bool {
        self.lid_action_ac
            && self.lid_action_dc
            && self.idle_sleep_ac
            && self.idle_sleep_dc
            && self.system_required_ac
            && self.system_required_dc
            && self.low_battery_action_ac
            && self.low_battery_action_dc
    }
}

/// Lê as oito configurações (quatro pares AC/DC) do plano informado e as
/// compara com os valores esperados, sem gravar nada.
pub fn read_status(scheme: GUID) -> NukaResult<SchemeProtectionStatus> {
    Ok(SchemeProtectionStatus {
        lid_action_ac: read_ac_value(scheme, guids::SUB_BUTTONS, guids::LIDACTION)?
            == LIDACTION_VALUE,
        lid_action_dc: read_dc_value(scheme, guids::SUB_BUTTONS, guids::LIDACTION)?
            == LIDACTION_VALUE,
        idle_sleep_ac: read_ac_value(scheme, guids::SUB_SLEEP, guids::STANDBYIDLE)?
            == STANDBYIDLE_VALUE,
        idle_sleep_dc: read_dc_value(scheme, guids::SUB_SLEEP, guids::STANDBYIDLE)?
            == STANDBYIDLE_VALUE,
        system_required_ac: read_ac_value(scheme, guids::SUB_SLEEP, guids::SYSTEMREQUIRED)?
            == SYSTEMREQUIRED_VALUE,
        system_required_dc: read_dc_value(scheme, guids::SUB_SLEEP, guids::SYSTEMREQUIRED)?
            == SYSTEMREQUIRED_VALUE,
        low_battery_action_ac: read_ac_value(scheme, guids::SUB_BATTERY, guids::BATACTIONLOW)?
            == BATACTIONLOW_VALUE,
        low_battery_action_dc: read_dc_value(scheme, guids::SUB_BATTERY, guids::BATACTIONLOW)?
            == BATACTIONLOW_VALUE,
    })
}

//! Camada de energia do NukaBoost: plano temporário, configurações,
//! Power Requests e a thread de `SetThreadExecutionState` (seção 6 do plano).
//!
//! Este módulo é intencionalmente o único lugar do crate que faz chamadas
//! `unsafe` a `powrprof.dll` e `kernel32.dll` para gerenciamento de energia.
//! Cada submódulo expõe apenas funções seguras, cada uma documentando em um
//! comentário `// SAFETY:` por que a chamada Win32 subjacente é válida.

pub mod execution_state;
pub mod guids;
pub mod request;
pub mod scheme;
pub mod settings;

pub use execution_state::ExecutionStateThread;
pub use request::PowerRequestGuard;
pub use scheme::PowerScheme;
pub use settings::{apply_and_confirm, read_status, SchemeProtectionStatus};

use windows::core::GUID;
use windows::Win32::Foundation::{ERROR_ACCESS_DISABLED_BY_POLICY, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Power::{
    PowerSettingAccessCheck, ACCESS_ACTIVE_SCHEME, ACCESS_AC_POWER_SETTING_INDEX,
    ACCESS_CREATE_SCHEME, ACCESS_DC_POWER_SETTING_INDEX, ACCESS_SCHEME,
};

use crate::error::{NukaError, NukaResult};

/// Converte um `WIN32_ERROR` retornado diretamente (sem passar por
/// `GetLastError`) em [`NukaResult`], anotando a função de origem.
///
/// As funções de `powrprof.dll` seguem essa convenção — diferente da
/// maioria das APIs Win32 empacotadas pela crate `windows`, que já traduzem
/// o resultado para `windows_core::Result`.
pub(crate) fn check(code: WIN32_ERROR, function: &'static str) -> NukaResult<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(NukaError::from_win32_code(function, code))
    }
}

/// Verifica, antes de tentar qualquer alteração, se nenhuma política
/// administrativa (Group Policy/MDM) bloqueia as operações ou configurações
/// de energia usadas pelo NukaBoost.
///
/// Corresponde ao passo 2 da transação de ativação (seção 7): "Consultar
/// `PowerSettingAccessCheck`". Retornar cedo aqui evita duplicar um plano,
/// aplicar configurações e só então descobrir que a política nega a
/// gravação — preferimos falhar antes de qualquer efeito colateral.
///
/// `ACCESS_SCHEME` exige o GUID do plano específico. Passar `None` produz
/// `ERROR_INVALID_PARAMETER` em versões atuais do Windows; além disso, a
/// função só deve ser traduzida como política administrativa quando retornar
/// explicitamente `ERROR_ACCESS_DISABLED_BY_POLICY`.
pub fn check_policy_allows_scheme_changes(active_scheme: GUID) -> NukaResult<()> {
    check_policy_access(
        ACCESS_CREATE_SCHEME,
        None,
        "PowerSettingAccessCheck(ACCESS_CREATE_SCHEME)",
    )?;
    check_policy_access(
        ACCESS_ACTIVE_SCHEME,
        None,
        "PowerSettingAccessCheck(ACCESS_ACTIVE_SCHEME)",
    )?;
    check_policy_access(
        ACCESS_SCHEME,
        Some(&active_scheme),
        "PowerSettingAccessCheck(ACCESS_SCHEME)",
    )?;

    for (setting, ac_check, dc_check) in [
        (
            guids::LIDACTION,
            "PowerSettingAccessCheck(LIDACTION/AC)",
            "PowerSettingAccessCheck(LIDACTION/DC)",
        ),
        (
            guids::STANDBYIDLE,
            "PowerSettingAccessCheck(STANDBYIDLE/AC)",
            "PowerSettingAccessCheck(STANDBYIDLE/DC)",
        ),
        (
            guids::SYSTEMREQUIRED,
            "PowerSettingAccessCheck(SYSTEMREQUIRED/AC)",
            "PowerSettingAccessCheck(SYSTEMREQUIRED/DC)",
        ),
        (
            guids::BATACTIONLOW,
            "PowerSettingAccessCheck(BATACTIONLOW/AC)",
            "PowerSettingAccessCheck(BATACTIONLOW/DC)",
        ),
    ] {
        check_policy_access(ACCESS_AC_POWER_SETTING_INDEX, Some(&setting), ac_check)?;
        check_policy_access(ACCESS_DC_POWER_SETTING_INDEX, Some(&setting), dc_check)?;
    }

    Ok(())
}

fn check_policy_access(
    accessor: windows::Win32::System::Power::POWER_DATA_ACCESSOR,
    guid: Option<&GUID>,
    function: &'static str,
) -> NukaResult<()> {
    // SAFETY: quando presente, o GUID vive por toda a chamada síncrona; a
    // função apenas consulta políticas e não retém o ponteiro fornecido.
    let code = unsafe { PowerSettingAccessCheck(accessor, guid.map(std::ptr::from_ref)) };
    interpret_policy_check(code, function)
}

fn interpret_policy_check(code: WIN32_ERROR, function: &'static str) -> NukaResult<()> {
    match code {
        ERROR_SUCCESS => Ok(()),
        ERROR_ACCESS_DISABLED_BY_POLICY => Err(NukaError::PolicyDenied),
        other => Err(NukaError::from_win32_code(function, other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::ERROR_INVALID_PARAMETER;

    #[test]
    fn only_the_explicit_policy_error_is_reported_as_policy_denied() {
        assert!(matches!(
            interpret_policy_check(ERROR_ACCESS_DISABLED_BY_POLICY, "test"),
            Err(NukaError::PolicyDenied)
        ));

        assert!(matches!(
            interpret_policy_check(ERROR_INVALID_PARAMETER, "test"),
            Err(NukaError::Win32 { code: 87, .. })
        ));
    }
}

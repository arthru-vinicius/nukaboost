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

use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Power::{PowerSettingAccessCheck, ACCESS_SCHEME};

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
/// administrativa (Group Policy/MDM) bloqueia a modificação de planos de
/// energia para o usuário atual.
///
/// Corresponde ao passo 2 da transação de ativação (seção 7): "Consultar
/// `PowerSettingAccessCheck`". Retornar cedo aqui evita duplicar um plano,
/// aplicar configurações e só então descobrir que a política nega a
/// gravação — preferimos falhar antes de qualquer efeito colateral.
pub fn check_policy_allows_scheme_changes() -> NukaResult<()> {
    // SAFETY: `ACCESS_SCHEME` não requer um GUID de entrada (passamos
    // `None`); a chamada é síncrona e não retém nenhum ponteiro.
    let code = unsafe { PowerSettingAccessCheck(ACCESS_SCHEME, None) };
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(NukaError::PolicyDenied)
    }
}

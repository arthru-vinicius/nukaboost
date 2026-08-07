//! Power Requests redundantes: `PowerRequestSystemRequired` e
//! `PowerRequestExecutionRequired` (seção 6, "Solicitações redundantes de
//! execução").
//!
//! O plano temporário já remove os gatilhos de suspensão, mas uma Power
//! Request informa formalmente ao Windows que há trabalho ativo em
//! andamento — uma segunda via oficial e independente. O NukaBoost nunca
//! cria uma request `PowerRequestDisplayRequired`: manter a tela acesa não
//! é um objetivo do produto (seção 1: "Permitir que a tela apague
//! normalmente em qualquer situação").

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, GetHandleInformation, HANDLE};
use windows::Win32::System::Power::{
    PowerClearRequest, PowerCreateRequest, PowerRequestExecutionRequired,
    PowerRequestSystemRequired, PowerSetRequest, POWER_REQUEST_TYPE,
};
use windows::Win32::System::Threading::{
    POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
};

use crate::error::NukaResult;

/// Versão exigida de `REASON_CONTEXT`. É sempre `0` — constante fixa dos
/// cabeçalhos do SDK do Windows (`POWER_REQUEST_CONTEXT_VERSION`), não um
/// valor de versão do NukaBoost.
const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;

/// Alça RAII de uma Power Request Win32.
///
/// Ao ser descartada, a request é limpa com `PowerClearRequest` e o handle
/// é fechado automaticamente — nenhum caminho de erro do chamador precisa
/// lembrar de limpar manualmente.
pub struct PowerRequestGuard {
    handle: Option<HANDLE>,
    request_type: POWER_REQUEST_TYPE,
    active: bool,
}

// SAFETY: um `HANDLE` de Power Request é um identificador de kernel opaco;
// o Win32 permite chamar `PowerSetRequest`/`PowerClearRequest`/`CloseHandle`
// a partir de qualquer thread, não apenas da que o criou.
unsafe impl Send for PowerRequestGuard {}

impl PowerRequestGuard {
    /// Cria e ativa uma request `PowerRequestSystemRequired`: informa ao
    /// Windows que um processo depende do sistema permanecer acordado.
    pub fn system_required(reason: &str) -> NukaResult<Self> {
        Self::create(PowerRequestSystemRequired, reason)
    }

    /// Cria e ativa uma request `PowerRequestExecutionRequired`: ajuda a
    /// manter o próprio processo executável (não suspenso/congelado).
    pub fn execution_required(reason: &str) -> NukaResult<Self> {
        Self::create(PowerRequestExecutionRequired, reason)
    }

    fn create(request_type: POWER_REQUEST_TYPE, reason: &str) -> NukaResult<Self> {
        // O buffer UTF-16 precisa sobreviver até o fim de `PowerCreateRequest`,
        // que copia a string internamente antes de retornar.
        let mut reason_wide: Vec<u16> = reason.encode_utf16().chain(std::iter::once(0)).collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason_wide.as_mut_ptr()),
            },
        };

        // SAFETY: `context` é válido durante a chamada e seu campo
        // `SimpleReasonString` aponta para `reason_wide`, que permanece viva
        // até o fim deste bloco (a chamada é síncrona).
        let handle = unsafe { PowerCreateRequest(&context) }
            .map_err(|e| crate::error::NukaError::from_win32("PowerCreateRequest", &e))?;

        // SAFETY: `handle` acabou de ser criado com sucesso por
        // `PowerCreateRequest` e ainda não foi fechado.
        if let Err(e) = unsafe { PowerSetRequest(handle, request_type) } {
            // SAFETY: `handle` é válido; fechamos para não vazar o recurso
            // já que a request nunca chegou a ser efetivamente ativada.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(crate::error::NukaError::from_win32("PowerSetRequest", &e));
        }

        Ok(Self {
            handle: Some(handle),
            request_type,
            active: true,
        })
    }

    /// Verifica se o handle de kernel ainda é válido. Essa confirmação é
    /// usada pelo monitor e pelo relatório de status em vez de inferir
    /// sucesso apenas pela presença da estrutura Rust.
    pub fn is_valid(&self) -> bool {
        let Some(handle) = self.handle else {
            return false;
        };
        let mut flags = 0u32;
        // SAFETY: `handle` pertence a este guard; `flags` é um ponteiro de
        // saída válido na pilha.
        unsafe { GetHandleInformation(handle, &mut flags) }.is_ok()
    }

    /// Limpa explicitamente a request e fecha o handle, propagando falhas
    /// para que a desativação não apague o journal prematuramente.
    pub fn clear(mut self) -> NukaResult<()> {
        self.clear_inner()
    }

    fn clear_inner(&mut self) -> NukaResult<()> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let clear_result = if self.active {
            // SAFETY: o handle é válido e a request foi ativada com este
            // mesmo tipo em `create`.
            unsafe { PowerClearRequest(handle, self.request_type) }
                .map_err(|e| crate::error::NukaError::from_win32("PowerClearRequest", &e))
        } else {
            Ok(())
        };
        self.active = false;

        // SAFETY: este é o único fechamento do handle.
        let close_result = unsafe { CloseHandle(handle) }
            .map_err(|e| crate::error::NukaError::from_win32("CloseHandle(PowerRequest)", &e));

        clear_result.and(close_result)
    }
}

impl Drop for PowerRequestGuard {
    fn drop(&mut self) {
        let _ = self.clear_inner();
    }
}

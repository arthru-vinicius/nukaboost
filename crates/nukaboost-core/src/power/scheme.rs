//! Duplicação, ativação e exclusão de planos de energia (seção 6 do plano).
//!
//! O NukaBoost nunca modifica o plano de energia original do usuário: ele
//! consulta o plano ativo, cria uma cópia chamada `NukaBoost Temporary`,
//! altera apenas essa cópia e a ativa. Ao desativar, o plano original é
//! reativado e a cópia temporária é excluída. Isso usa exclusivamente as
//! APIs oficiais documentadas em
//! [Managing Power Schemes](https://learn.microsoft.com/en-us/windows/win32/power/managing-power-schemes).

use windows::core::GUID;
use windows::Win32::Foundation::{LocalFree, ERROR_FILE_NOT_FOUND, HLOCAL};
use windows::Win32::System::Power::{
    PowerDeleteScheme, PowerDuplicateScheme, PowerGetActiveScheme, PowerSetActiveScheme,
    PowerWriteFriendlyName,
};

use super::check;
use crate::error::NukaResult;

/// Nome dado ao plano de energia temporário criado pelo NukaBoost, exibido
/// no Painel de Controle enquanto ele existir.
pub const TEMPORARY_SCHEME_NAME: &str = "NukaBoost Temporary";

/// Referência a um plano de energia do Windows, identificado por GUID.
///
/// Este tipo não distingue "o plano original" de "o plano temporário" — essa
/// distinção é responsabilidade de quem orquestra a transação de ativação
/// (ver `apps/nukaboost`), que guarda ambos os GUIDs no journal de recuperação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerScheme(GUID);

impl PowerScheme {
    /// Envolve um GUID de plano já conhecido (por exemplo, lido de volta do
    /// journal de recuperação) sem consultar o Windows.
    pub const fn from_guid(guid: GUID) -> Self {
        Self(guid)
    }

    /// GUID bruto deste plano de energia.
    pub const fn guid(&self) -> GUID {
        self.0
    }

    /// Consulta o plano de energia atualmente ativo no sistema
    /// (`PowerGetActiveScheme`).
    pub fn active() -> NukaResult<Self> {
        let mut raw: *mut GUID = std::ptr::null_mut();
        // SAFETY: `raw` é um ponteiro de saída válido para receber um bloco
        // alocado pelo Windows via `LocalAlloc`; nós o liberamos com
        // `LocalFree` logo abaixo, conforme exigido pela documentação da API.
        let code = unsafe { PowerGetActiveScheme(None, &mut raw) };
        check(code, "PowerGetActiveScheme")?;
        Ok(Self(take_guid(raw)))
    }

    /// Cria uma cópia independente deste plano com um novo GUID
    /// (`PowerDuplicateScheme`), deixando o plano original intocado.
    pub fn duplicate(&self) -> NukaResult<Self> {
        let mut raw: *mut GUID = std::ptr::null_mut();
        // SAFETY: mesmo contrato de `active`; `self.0` permanece válido
        // durante toda a chamada, que é síncrona.
        let code = unsafe { PowerDuplicateScheme(None, &self.0, &mut raw) };
        check(code, "PowerDuplicateScheme")?;
        let duplicate = Self(take_guid(raw));
        if let Err(error) = duplicate.set_friendly_name(TEMPORARY_SCHEME_NAME) {
            return match duplicate.delete() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(crate::error::NukaError::Other(format!(
                    "failed to name duplicated power scheme: {error}; cleanup also failed: {cleanup}"
                ))),
            };
        }
        Ok(duplicate)
    }

    /// Torna este plano o plano de energia ativo do sistema
    /// (`PowerSetActiveScheme`).
    pub fn activate(&self) -> NukaResult<()> {
        // SAFETY: `self.0` é um GUID válido e vive durante toda a chamada.
        let code = unsafe { PowerSetActiveScheme(None, Some(&self.0)) };
        check(code, "PowerSetActiveScheme")
    }

    /// Exclui este plano de energia permanentemente (`PowerDeleteScheme`).
    ///
    /// Deve ser chamado apenas sobre o plano temporário do NukaBoost, nunca
    /// sobre o plano original do usuário.
    pub fn delete(&self) -> NukaResult<()> {
        // SAFETY: `self.0` é um GUID válido e vive durante toda a chamada.
        let code = unsafe { PowerDeleteScheme(None, &self.0) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(code, "PowerDeleteScheme")
    }

    /// Define o nome legível exibido pelo Painel de Controle para este
    /// plano. O buffer esperado por `PowerWriteFriendlyName` é UTF-16.
    pub fn set_friendly_name(&self, name: &str) -> NukaResult<()> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` permanece vivo durante a chamada síncrona; a
        // conversão para bytes preserva exatamente o buffer UTF-16.
        let bytes = unsafe {
            std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * size_of::<u16>())
        };
        // SAFETY: o GUID do plano e o buffer permanecem válidos durante a
        // chamada; `None` nos dois GUIDs indica o próprio esquema.
        let code = unsafe { PowerWriteFriendlyName(None, &self.0, None, None, bytes) };
        check(code, "PowerWriteFriendlyName")
    }
}

/// Lê o GUID apontado por um ponteiro alocado pelo Windows via `LocalAlloc`
/// e libera o bloco em seguida, conforme o contrato de `PowerGetActiveScheme`
/// e `PowerDuplicateScheme`.
fn take_guid(raw: *mut GUID) -> GUID {
    assert!(
        !raw.is_null(),
        "Win32 reported success but returned a null pointer"
    );
    // SAFETY: em caso de sucesso (verificado antes de chamar esta função),
    // `raw` aponta para um `GUID` válido alocado pelo Windows.
    let guid = unsafe { *raw };
    // SAFETY: `raw` foi alocado pelo Windows com `LocalAlloc` e ainda não
    // foi liberado; é seguro passá-lo para `LocalFree` exatamente uma vez.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(raw.cast())));
    }
    guid
}

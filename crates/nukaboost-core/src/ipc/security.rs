//! Descritor de segurança do Named Pipe (seção 11 do plano): acesso restrito
//! ao usuário atual e, opcionalmente, aos administradores locais — nunca a
//! logons de rede.
//!
//! Named Pipes têm seu próprio controle de acesso e **não** devem usar o
//! descritor padrão do sistema, que pode ser permissivo demais. Ver
//! [Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights).

use windows::core::{w, BOOL};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

use crate::error::{NukaError, NukaResult};

/// SDDL que concede acesso total (`GA`) apenas ao dono do processo (usuário
/// atual, `OW`) e ao grupo de Administradores locais (`BA`).
///
/// O prefixo `D:P` marca o DACL como *protegido* (não herda ACEs de um
/// contêiner pai); qualquer identidade não listada — incluindo `Everyone` e
/// logons de rede — é implicitamente negada.
const PIPE_SDDL: windows::core::PCWSTR = w!("D:P(A;;GA;;;OW)(A;;GA;;;BA)");

/// Alça RAII de um `SECURITY_DESCRIPTOR` construído a partir de [`PIPE_SDDL`],
/// pronta para ser referenciada por um `SECURITY_ATTRIBUTES` passado a
/// `CreateNamedPipeW`.
pub struct PipeSecurityDescriptor {
    descriptor: PSECURITY_DESCRIPTOR,
}

// SAFETY: o descritor é apenas um bloco de memória opaco alocado pelo
// Windows; não há afinidade de thread associada a ele.
unsafe impl Send for PipeSecurityDescriptor {}

impl PipeSecurityDescriptor {
    /// Constrói o descritor restrito descrito em [`PIPE_SDDL`].
    pub fn build() -> NukaResult<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        // SAFETY: `PIPE_SDDL` é uma string constante terminada em nulo
        // válida durante todo o processo; `descriptor` é um ponteiro de
        // saída válido na pilha.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PIPE_SDDL,
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|e| {
            NukaError::from_win32("ConvertStringSecurityDescriptorToSecurityDescriptorW", &e)
        })?;

        Ok(Self { descriptor })
    }

    /// Monta um `SECURITY_ATTRIBUTES` que referencia este descritor.
    ///
    /// O valor retornado empresta `self`: não deve ser usado depois que
    /// este [`PipeSecurityDescriptor`] for descartado.
    pub fn as_security_attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor.0,
            bInheritHandle: BOOL(0),
        }
    }
}

impl Drop for PipeSecurityDescriptor {
    fn drop(&mut self) {
        if !self.descriptor.is_invalid() {
            // SAFETY: `self.descriptor` foi alocado por
            // `ConvertStringSecurityDescriptorToSecurityDescriptorW` (que
            // usa `LocalAlloc` internamente) e ainda não foi liberado.
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.descriptor.0)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_valid_descriptor() {
        let descriptor = PipeSecurityDescriptor::build().unwrap();
        let attrs = descriptor.as_security_attributes();
        assert!(!attrs.lpSecurityDescriptor.is_null());
        assert_eq!(
            attrs.nLength as usize,
            std::mem::size_of::<SECURITY_ATTRIBUTES>()
        );
    }
}

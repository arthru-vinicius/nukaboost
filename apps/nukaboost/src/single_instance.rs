//! Instância única do processo principal (seção 3 do plano: "Criar uma
//! única instância do processo").
//!
//! Usa o truque clássico do Win32: `CreateMutexW` cria o objeto se ele não
//! existir, ou abre o existente — e `GetLastError` diz qual dos dois
//! aconteceu. Não usamos o mutex para exclusão mútua entre threads (nunca
//! chamamos `WaitForSingleObject`/`ReleaseMutex` nele); ele só precisa
//! existir durante toda a vida do processo para que a segunda instância
//! detecte a primeira.

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

/// Nome do mutex nomeado, restrito à sessão de logon atual (prefixo
/// `Local\`) — suficiente para o caso de uso e evita exigir o privilégio
/// necessário para objetos `Global\`.
const MUTEX_NAME: windows::core::PCWSTR = w!(r"Local\NukaBoost-SingleInstance");

/// Alça RAII que garante instância única enquanto viva.
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl SingleInstanceGuard {
    /// Tenta adquirir a instância única. Retorna `Ok(None)` se **já houver**
    /// outra instância em execução (caso em que o chamador deve encerrar
    /// sem criar nenhum ícone ou janela), ou `Ok(Some(guard))` se esta for
    /// a primeira.
    pub fn acquire() -> windows::core::Result<Option<Self>> {
        // SAFETY: `MUTEX_NAME` é uma string constante válida; não
        // requisitamos posse inicial (`binitialowner: false`) porque nunca
        // usamos este mutex para exclusão mútua real.
        let handle = unsafe { CreateMutexW(None, false, MUTEX_NAME) }?;

        if windows::core::Error::from_thread().code()
            == windows::core::HRESULT::from_win32(ERROR_ALREADY_EXISTS.0)
        {
            // SAFETY: `handle` ainda é válido mesmo neste caminho — o
            // Windows abriu o mutex existente — mas não precisamos dele.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Ok(None);
        }

        Ok(Some(Self { handle }))
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // SAFETY: `self.handle` foi criado por `CreateMutexW` em `acquire`
        // e ainda não foi fechado.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

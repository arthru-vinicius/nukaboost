//! Entrada de recuperação `RunOnce` (seção 9 do plano), deliberadamente
//! separada da entrada normal de Startup.
//!
//! Registrada apenas enquanto uma sessão está ativa; dispara
//! `NukaBoost.exe --recover-only <session-id>` no próximo logon caso o
//! Windows tenha sido desligado ou reiniciado abruptamente antes de o
//! NukaBoost se desarmar normalmente. `RunOnce` é removida pelo próprio
//! Windows antes de executar o comando, então não precisamos limpá-la no
//! caminho de recuperação — apenas no caminho de desativação normal
//! (seção 8, passo 11).
//!
//! O nome do valor é prefixado com `!` deliberadamente: por padrão o
//! Windows apaga uma entrada `RunOnce` **antes** de rodar o comando, então
//! uma falha transitória em `--recover-only` (por exemplo, o plano original
//! ter sido bloqueado por política nesse exato boot) perderia a única
//! tentativa de recuperação para sempre — exatamente o cenário relatado de
//! tampa desconfigurada que sobrevive a reinicializações mesmo sem o
//! NukaBoost em execução. O prefixo `!` adia a exclusão até depois de o
//! comando rodar, então o Windows só remove a entrada quando
//! `--recover-only` sai com sucesso (código 0); enquanto ele continuar
//! falhando, a recuperação é tentada de novo em todo logon subsequente.
//! Comportamento documentado em
//! <https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys>.

use uuid::Uuid;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::error::{NukaError, NukaResult};

const RUNONCE_SUBKEY: windows::core::PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\RunOnce");
/// Prefixo `!` = adiar a exclusão até depois da execução (ver doc do módulo).
const RUNONCE_VALUE_NAME: windows::core::PCWSTR = w!("!NukaBoostRecovery");

/// Registra a recuperação de emergência para a sessão `session_id`.
pub fn register(session_id: Uuid) -> NukaResult<()> {
    let exe = std::env::current_exe().map_err(|e| NukaError::io("<current_exe>", e))?;
    let command = format!("\"{}\" --recover-only {session_id}", exe.display());
    let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: reinterpreta o buffer UTF-16 (sem padding, little-endian em
    // todas as plataformas Windows suportadas) como bytes crus, exatamente
    // o layout que `REG_SZ` espera; `wide` permanece vivo durante o uso de
    // `bytes` logo abaixo.
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };

    let key = create_key()?;
    // SAFETY: `key` foi aberta com sucesso por `open_key`; `bytes` vive
    // durante esta chamada síncrona.
    let code = unsafe { RegSetValueExW(key, RUNONCE_VALUE_NAME, None, REG_SZ, Some(bytes)) };
    // SAFETY: `key` ainda não foi fechada.
    unsafe {
        let _ = RegCloseKey(key);
    }
    check(code, "RegSetValueExW")
}

/// Remove a entrada de recuperação (seção 8, passo 11: "Remover
/// recuperação de próximo logon"). Idempotente — não é erro chamar quando
/// a entrada já não existe (por exemplo, porque o Windows já a consumiu).
pub fn unregister() -> NukaResult<()> {
    let key = match open_key() {
        Ok(key) => key,
        Err(NukaError::Win32 { code, .. }) if code == ERROR_FILE_NOT_FOUND.0 => return Ok(()),
        Err(error) => return Err(error),
    };
    // SAFETY: `key` é válida durante esta chamada síncrona.
    let code = unsafe { RegDeleteValueW(key, RUNONCE_VALUE_NAME) };
    // SAFETY: `key` ainda não foi fechada.
    unsafe {
        let _ = RegCloseKey(key);
    }
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(code, "RegDeleteValueW")
}

fn create_key() -> NukaResult<HKEY> {
    let mut key = HKEY(std::ptr::null_mut());
    // SAFETY: cria a chave padrão somente se ela não existir; nenhum handle
    // ou descritor de segurança é herdado.
    let code = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUNONCE_SUBKEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    check(code, "RegCreateKeyExW")?;
    Ok(key)
}

fn open_key() -> NukaResult<HKEY> {
    let mut key = HKEY(std::ptr::null_mut());
    // SAFETY: `RUNONCE_SUBKEY` é uma string constante válida; `key` é um
    // ponteiro de saída válido na pilha.
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUNONCE_SUBKEY,
            None,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    check(code, "RegOpenKeyExW")?;
    Ok(key)
}

fn check(code: WIN32_ERROR, function: &'static str) -> NukaResult<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(NukaError::from_win32_code(function, code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trava a regressão mais fácil de cometer aqui: perder o prefixo `!`
    /// de novo faz uma falha silenciosa em `--recover-only` nunca mais ser
    /// tentada (ver doc do módulo).
    #[test]
    fn value_name_keeps_the_deferred_deletion_prefix() {
        let decoded = unsafe { RUNONCE_VALUE_NAME.to_string() }.unwrap();
        assert!(decoded.starts_with('!'));
        assert_eq!(decoded, "!NukaBoostRecovery");
    }
}

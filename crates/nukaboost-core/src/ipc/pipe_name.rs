//! Nome do Named Pipe local, único por usuário (seção 11 do plano):
//! `\\.\pipe\NukaBoost-{user-sid}`.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::error::{NukaError, NukaResult};

/// Monta o caminho do Named Pipe do NukaBoost para o usuário atual.
pub fn pipe_path() -> NukaResult<String> {
    Ok(format!(
        r"\\.\pipe\NukaBoost-{}",
        current_user_sid_string()?
    ))
}

/// Obtém a representação textual (`S-1-5-...`) do SID do usuário que
/// executa o processo atual, usada para isolar o pipe de outros usuários da
/// mesma máquina.
fn current_user_sid_string() -> NukaResult<String> {
    // SAFETY: `GetCurrentProcess` retorna um pseudo-handle válido que não
    // precisa (e não deve) ser fechado.
    let process = unsafe { GetCurrentProcess() };

    let mut token = HANDLE::default();
    // SAFETY: `process` é um pseudo-handle válido; `token` é um ponteiro de
    // saída válido para um `HANDLE` na pilha.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
        .map_err(|e| NukaError::from_win32("OpenProcessToken", &e))?;

    let result = read_sid_from_token(token);

    // SAFETY: `token` foi aberto com sucesso por `OpenProcessToken` acima e
    // ainda não foi fechado.
    unsafe {
        let _ = CloseHandle(token);
    }

    result
}

fn read_sid_from_token(token: HANDLE) -> NukaResult<String> {
    // Padrão clássico de duas chamadas: a primeira, com buffer nulo, apenas
    // reporta o tamanho necessário em `needed` (e sempre retorna erro).
    let mut needed = 0u32;
    // SAFETY: buffer nulo é explicitamente permitido pela API quando o
    // objetivo é apenas descobrir o tamanho necessário.
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut needed) };
    if needed == 0 {
        return Err(NukaError::Other(
            "GetTokenInformation did not report a buffer size".into(),
        ));
    }

    let mut buffer = vec![0u8; needed as usize];
    // SAFETY: `buffer` tem exatamente `needed` bytes, o tamanho reportado
    // pela chamada de sondagem acima; `token` continua válido.
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
    }
    .map_err(|e| NukaError::from_win32("GetTokenInformation", &e))?;

    // SAFETY: `buffer` acabou de ser preenchido por `GetTokenInformation`
    // com uma estrutura `TOKEN_USER` válida de pelo menos `needed` bytes.
    let token_user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };

    let mut sid_string = PWSTR::null();
    // SAFETY: `token_user.User.Sid` aponta para dentro de `buffer`, que
    // permanece vivo durante esta chamada síncrona.
    unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string) }
        .map_err(|e| NukaError::from_win32("ConvertSidToStringSidW", &e))?;

    // SAFETY: `sid_string` foi preenchido com sucesso pela chamada acima e
    // aponta para uma string terminada em nulo.
    let text = unsafe { sid_string.to_string() }
        .map_err(|e| NukaError::Other(format!("returned SID is not valid UTF-16: {e}")))?;

    // SAFETY: `sid_string` foi alocado pelo Windows via `LocalAlloc` por
    // `ConvertSidToStringSidW` e ainda não foi liberado.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sid_string.0.cast())));
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_path_is_well_formed() {
        let path = pipe_path().expect("must be able to read the current user's SID in CI/dev");
        assert!(path.starts_with(r"\\.\pipe\NukaBoost-S-1-"));
    }
}

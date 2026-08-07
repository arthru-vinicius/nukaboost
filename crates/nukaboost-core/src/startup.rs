//! Inicialização com o Windows via atalho em `FOLDERID_Startup` (seção 14
//! do plano).
//!
//! Implementado como manipulação direta de arquivo — **sem** depender do
//! processo principal estar em execução nem do Named Pipe — para que
//! `nukaboostctl startup enable|disable` funcione mesmo com o NukaBoost
//! fechado, que é o caso mais comum ao configurar essa opção. Por isso
//! [`Command`](crate::ipc::protocol::Command) não tem variantes de startup:
//! esta é a única fonte da verdade.

use std::path::PathBuf;

use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::S_FALSE;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Startup, IShellLinkW, SHGetKnownFolderPath, ShellLink, KF_FLAG_DEFAULT,
};

use crate::error::{NukaError, NukaResult};
use crate::fs_atomic::remove_if_exists;

/// Nome do arquivo de atalho criado em `FOLDERID_Startup`.
const SHORTCUT_FILE_NAME: &str = "NukaBoost.lnk";

/// Argumento passado ao executável pelo atalho de inicialização (seção 14,
/// regras de `--startup`: recupera, inicia inativo, nunca ativa por causa
/// de leases antigas, não exibe janela principal).
const STARTUP_ARG: &str = "--startup";

/// Caminho completo do atalho de inicialização, mesmo que ele ainda não exista.
pub fn shortcut_path() -> NukaResult<PathBuf> {
    // SAFETY: `FOLDERID_Startup` é uma pasta conhecida padrão do Windows;
    // `KF_FLAG_DEFAULT` não exige nenhum recurso adicional; `None` usa o
    // token do processo atual.
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None) }
        .map_err(|e| NukaError::from_win32("SHGetKnownFolderPath", &e))?;

    // SAFETY: `raw` foi preenchido com sucesso pela chamada acima e aponta
    // para uma string terminada em nulo.
    let text_result = unsafe { raw.to_string() };
    // SAFETY: `raw` foi alocado pelo Windows via `CoTaskMemAlloc` — deve
    // ser liberado com `CoTaskMemFree` (não `LocalFree`), e apenas uma vez.
    unsafe {
        CoTaskMemFree(Some(raw.0 as *const _));
    }

    let text = text_result
        .map_err(|e| NukaError::Other(format!("Startup path is not valid UTF-16: {e}")))?;
    Ok(PathBuf::from(text).join(SHORTCUT_FILE_NAME))
}

/// Verdadeiro se o atalho de inicialização já existir.
pub fn is_enabled() -> NukaResult<bool> {
    Ok(shortcut_path()?.exists())
}

/// Cria (ou substitui) o atalho de inicialização, apontando para
/// `NukaBoost.exe --startup` no mesmo diretório do executável atual.
pub fn enable() -> NukaResult<()> {
    let current_exe = std::env::current_exe().map_err(|e| NukaError::io("<current_exe>", e))?;
    let exe_path = if current_exe.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .eq_ignore_ascii_case("nukaboostctl.exe")
    }) {
        current_exe.with_file_name("NukaBoost.exe")
    } else {
        current_exe
    };
    if !exe_path.is_file() {
        return Err(NukaError::Other(format!(
            "NukaBoost.exe was not found next to the control utility: {}",
            exe_path.display()
        )));
    }
    let link_path = shortcut_path()?;

    let _com = ComGuard::initialize()?;

    // SAFETY: `ShellLink` é o CLSID documentado do objeto `IShellLinkW` do
    // shell; `CLSCTX_INPROC_SERVER` é o contexto usual para esse objeto.
    let shell_link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| NukaError::from_win32("CoCreateInstance(ShellLink)", &e))?;

    let exe_path_wide = HSTRING::from(exe_path.to_string_lossy().as_ref());
    // SAFETY: chamadas COM síncronas; cada argumento vive até o fim da
    // respectiva chamada.
    unsafe { shell_link.SetPath(&exe_path_wide) }
        .map_err(|e| NukaError::from_win32("IShellLinkW::SetPath", &e))?;
    unsafe { shell_link.SetArguments(&HSTRING::from(STARTUP_ARG)) }
        .map_err(|e| NukaError::from_win32("IShellLinkW::SetArguments", &e))?;
    if let Some(dir) = exe_path.parent() {
        unsafe { shell_link.SetWorkingDirectory(&HSTRING::from(dir.to_string_lossy().as_ref())) }
            .map_err(|e| NukaError::from_win32("IShellLinkW::SetWorkingDirectory", &e))?;
    }
    unsafe { shell_link.SetDescription(&HSTRING::from("Start NukaBoost with Windows")) }
        .map_err(|e| NukaError::from_win32("IShellLinkW::SetDescription", &e))?;

    // SAFETY: `cast` verifica o IID via `QueryInterface`; seguro mesmo que
    // a interface não seja suportada (retorna `Err`, não UB).
    let persist_file: IPersistFile = shell_link
        .cast()
        .map_err(|e| NukaError::from_win32("IShellLinkW::cast::<IPersistFile>", &e))?;

    if let Some(parent) = link_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| NukaError::io(parent, e))?;
    }
    let link_path_wide = HSTRING::from(link_path.to_string_lossy().as_ref());
    // SAFETY: `link_path_wide` vive até o fim desta chamada síncrona.
    unsafe { persist_file.Save(&link_path_wide, true) }
        .map_err(|e| NukaError::from_win32("IPersistFile::Save", &e))?;

    Ok(())
}

/// Remove o atalho de inicialização, se existir. Idempotente.
pub fn disable() -> NukaResult<()> {
    remove_if_exists(&shortcut_path()?)
}

/// Alça RAII mínima para `CoInitializeEx`/`CoUninitialize` nesta thread.
///
/// Cada chamada a [`enable`] inicializa e desinicializa COM independentemente
/// — não há custo relevante em fazer isso a cada chamada, e evita manter
/// estado global de COM vivo por toda a vida do processo (`nukaboostctl` é
/// de curta duração; `apps/nukaboost` só precisa de COM neste caminho).
struct ComGuard;

impl ComGuard {
    fn initialize() -> NukaResult<Self> {
        // SAFETY: `CoInitializeEx` é seguro para chamar; o HRESULT
        // retornado é verificado manualmente porque esta API não usa o
        // mecanismo de `GetLastError`.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // `S_FALSE` (COM já inicializado nesta thread, por exemplo por
        // outra biblioteca) também conta como sucesso.
        if hr.is_ok() || hr == S_FALSE {
            Ok(Self)
        } else {
            Err(NukaError::Other(format!("CoInitializeEx failed: {hr:?}")))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: só executado após uma inicialização bem-sucedida
        // correspondente nesta mesma thread, em `ComGuard::initialize`.
        unsafe { CoUninitialize() };
    }
}

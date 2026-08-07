//! Caminhos de arquivos usados pelo NukaBoost no perfil do usuário atual.
//!
//! Todos os artefatos de estado (configuração, journal de recuperação e
//! logs) vivem sob `%LocalAppData%\NukaBoost`, conforme a seção 9 do plano
//! do produto. Usamos a variável de ambiente `LOCALAPPDATA` — que o Windows
//! mantém sincronizada com a pasta conhecida `FOLDERID_LocalAppData` mesmo
//! sob redirecionamento de pasta — em vez de uma chamada COM a
//! `SHGetKnownFolderPath`, para manter este módulo livre de `unsafe`.

use std::path::PathBuf;

use crate::error::{NukaError, NukaResult};

/// Retorna `%LocalAppData%` do usuário atual.
pub fn local_app_data_dir() -> NukaResult<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| NukaError::Other("LOCALAPPDATA environment variable is not set".into()))
}

/// Diretório raiz de dados do NukaBoost: `%LocalAppData%\NukaBoost`.
pub fn app_data_dir() -> NukaResult<PathBuf> {
    Ok(local_app_data_dir()?.join("NukaBoost"))
}

/// Garante que o diretório de dados exista, criando-o se necessário, e o retorna.
pub fn ensure_app_data_dir() -> NukaResult<PathBuf> {
    let dir = app_data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| NukaError::io(&dir, e))?;
    Ok(dir)
}

/// Caminho do arquivo de configuração persistida (`config.json`).
pub fn config_file() -> NukaResult<PathBuf> {
    Ok(app_data_dir()?.join("config.json"))
}

/// Caminho do journal de recuperação (`recovery.json`).
pub fn recovery_journal_file() -> NukaResult<PathBuf> {
    Ok(app_data_dir()?.join("recovery.json"))
}

/// Diretório de logs rotacionados (`logs\`).
pub fn logs_dir() -> NukaResult<PathBuf> {
    Ok(app_data_dir()?.join("logs"))
}

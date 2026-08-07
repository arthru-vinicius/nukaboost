//! Escrita atômica de arquivos JSON de estado.
//!
//! O padrão usado — gravar em um arquivo temporário no mesmo diretório,
//! sincronizar (`fsync`) e então renomear sobre o destino final — é exigido
//! explicitamente pela seção 9 do plano para o journal de recuperação, e é
//! reaproveitado aqui também para a configuração, pelo mesmo motivo: um
//! processo morto no meio da escrita nunca deve deixar um arquivo truncado
//! ou corrompido para trás.
//!
//! No Windows, `std::fs::rename` mapeia para `MoveFileExW` com
//! `MOVEFILE_REPLACE_EXISTING`, portanto a renomeação final já é atômica em
//! relação a um destino pré-existente no mesmo volume.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{NukaError, NukaResult};

/// Serializa `value` como JSON legível e o grava atomicamente em `path`.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> NukaResult<()> {
    let dir = path.parent().ok_or_else(|| {
        NukaError::Other(format!("path has no parent directory: {}", path.display()))
    })?;
    fs::create_dir_all(dir).map_err(|e| NukaError::io(dir, e))?;

    let tmp_path = sibling_tmp_path(path);
    {
        let mut file = File::create(&tmp_path).map_err(|e| NukaError::io(&tmp_path, e))?;
        let json = serde_json::to_vec_pretty(value)?;
        file.write_all(&json)
            .map_err(|e| NukaError::io(&tmp_path, e))?;
        file.sync_all().map_err(|e| NukaError::io(&tmp_path, e))?;
    }
    fs::rename(&tmp_path, path).map_err(|e| NukaError::io(path, e))?;
    Ok(())
}

/// Lê e desserializa um arquivo JSON, retornando `Ok(None)` se ele não existir.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> NukaResult<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(NukaError::io(path, e)),
    }
}

/// Remove um arquivo de estado, ignorando silenciosamente o caso em que ele
/// já não existe (idempotência é desejável ao limpar journal/config).
pub fn remove_if_exists(path: &Path) -> NukaResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(NukaError::io(path, e)),
    }
}

/// Deriva o caminho do arquivo temporário irmão usado durante a escrita,
/// preservando o nome original e apenas acrescentando a extensão `.tmp`.
fn sibling_tmp_path(path: &Path) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sample {
        value: u32,
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.json");

        write_json_atomic(&path, &Sample { value: 42 }).unwrap();
        let loaded: Option<Sample> = read_json(&path).unwrap();

        assert_eq!(loaded, Some(Sample { value: 42 }));
        assert!(
            !sibling_tmp_path(&path).exists(),
            ".tmp file must be removed by the rename"
        );
    }

    #[test]
    fn read_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");

        let loaded: Option<Sample> = read_json(&path).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn write_overwrites_existing_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.json");

        write_json_atomic(&path, &Sample { value: 1 }).unwrap();
        write_json_atomic(&path, &Sample { value: 2 }).unwrap();

        let loaded: Option<Sample> = read_json(&path).unwrap();
        assert_eq!(loaded, Some(Sample { value: 2 }));
    }

    #[test]
    fn remove_if_exists_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.json");

        remove_if_exists(&path).unwrap();
        write_json_atomic(&path, &Sample { value: 1 }).unwrap();
        remove_if_exists(&path).unwrap();
        remove_if_exists(&path).unwrap();

        assert!(!path.exists());
    }
}

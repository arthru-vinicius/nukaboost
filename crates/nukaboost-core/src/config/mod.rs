//! Configuração persistida do usuário.
//!
//! Ao contrário do journal de recuperação (`recovery`), que descreve uma
//! sessão ativa em andamento, [`AppConfig`] guarda apenas preferências que
//! devem sobreviver a reinicializações do Windows: idioma da interface e se
//! o aviso de segurança inicial já foi reconhecido. Nunca inclui o estado
//! `Active`/`Inactive` — a seção 3 do plano exige que o NukaBoost sempre
//! inicie inativo, mesmo pelo Startup.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::NukaResult;
use crate::fs_atomic::{read_json, write_json_atomic};
use crate::i18n::Language;
use crate::paths;

/// Versão do esquema de [`AppConfig`], incrementada sempre que um campo é
/// adicionado ou remove de forma incompatível. Permite que versões futuras
/// migrem arquivos antigos em vez de rejeitá-los.
pub const CONFIG_SCHEMA_VERSION: u32 = 2;

/// Preferências do usuário persistidas entre execuções do NukaBoost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Versão do esquema deste arquivo, para permitir migrações futuras.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Idioma de interface escolhido pelo usuário (padrão: inglês).
    #[serde(default)]
    pub language: Language,

    /// Verdadeiro depois que o usuário confirma o aviso de segurança com a
    /// caixa "Do not show this warning again" marcada. Enquanto for falso,
    /// o aviso volta a ser exibido a cada inicialização e nenhuma ativação
    /// é permitida antes da confirmação (ver [`crate::error::NukaError::SafetyAckRequired`]).
    #[serde(default)]
    pub safety_ack_confirmed: bool,
}

fn default_schema_version() -> u32 {
    CONFIG_SCHEMA_VERSION
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            language: Language::default(),
            safety_ack_confirmed: false,
        }
    }
}

impl AppConfig {
    /// Carrega a configuração de `%LocalAppData%\NukaBoost\config.json`.
    ///
    /// Se o arquivo não existir (primeira execução), retorna
    /// [`AppConfig::default`] sem criar nada em disco — a gravação só
    /// acontece explicitamente via [`AppConfig::save`].
    pub fn load() -> NukaResult<Self> {
        let path = paths::config_file()?;
        Self::load_from(&path)
    }

    /// Persiste a configuração atual em `%LocalAppData%\NukaBoost\config.json`.
    pub fn save(&self) -> NukaResult<()> {
        let path = paths::config_file()?;
        self.save_to(&path)
    }

    /// Variante testável de [`AppConfig::load`] que aceita um caminho explícito.
    pub fn load_from(path: &Path) -> NukaResult<Self> {
        let mut config = read_json::<Self>(path)?.unwrap_or_default();
        if config.schema_version > CONFIG_SCHEMA_VERSION {
            return Err(crate::error::NukaError::Other(format!(
                "config schema {} is newer than supported schema {}",
                config.schema_version, CONFIG_SCHEMA_VERSION
            )));
        }
        config.schema_version = CONFIG_SCHEMA_VERSION;
        Ok(config)
    }

    /// Variante testável de [`AppConfig::save`] que aceita um caminho explícito.
    pub fn save_to(&self, path: &Path) -> NukaResult<()> {
        write_json_atomic(path, self)
    }

    /// Caminho absoluto do arquivo de configuração no perfil do usuário atual.
    pub fn default_path() -> NukaResult<PathBuf> {
        paths::config_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_starts_unacknowledged_and_english() {
        let config = AppConfig::default();
        assert_eq!(config.language, Language::En);
        assert!(!config.safety_ack_confirmed);
    }

    #[test]
    fn missing_file_loads_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded, AppConfig::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = AppConfig {
            language: Language::Pt,
            safety_ack_confirmed: true,
            ..AppConfig::default()
        };
        config.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn rejects_a_future_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"schema_version":999,"language":"en","safety_ack_confirmed":false}"#,
        )
        .unwrap();

        assert!(AppConfig::load_from(&path).is_err());
    }
}

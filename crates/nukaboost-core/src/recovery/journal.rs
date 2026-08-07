//! Journal de recuperação: `%LocalAppData%\NukaBoost\recovery.json`.
//!
//! Descreve uma sessão ativa em andamento — ao contrário de
//! [`crate::config::AppConfig`], que guarda preferências permanentes, este
//! arquivo só existe enquanto o NukaBoost está `Activating`, `Active` ou
//! `Deactivating`, e é apagado assim que o processo volta a `Inactive` de
//! forma limpa. Se ele sobreviver a uma queda de energia ou `taskkill /F`,
//! sua presença no próximo logon é o sinal de que uma restauração pendente
//! precisa ser executada (`--recover-only`).

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::NukaResult;
use crate::fs_atomic::{read_json, remove_if_exists, write_json_atomic};
use crate::paths;
use crate::power::guids::{format_guid, parse_guid, InvalidGuid};

/// Versão do esquema do journal, para permitir migrações futuras.
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

/// Fase da transação em que a sessão registrada pelo journal se encontra.
///
/// Não inclui `Inactive`/`Error` da máquina de estados geral: o journal
/// simplesmente não existe fora de uma sessão ativa (ver módulo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalPhase {
    /// Transação de ativação em andamento; ainda não confirmada.
    Activating,
    /// Sessão totalmente ativa e confirmada.
    Active,
    /// Transação de desativação em andamento.
    Deactivating,
}

/// Conteúdo persistido do journal de recuperação.
///
/// Os campos reproduzem o "conteúdo mínimo" da seção 9 do plano, acrescidos
/// de `main_pid_created_at_utc`, necessário para a defesa contra
/// reutilização de PID descrita na mesma seção
/// (ver [`crate::recovery::is_same_process`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryJournal {
    /// Versão do esquema deste arquivo.
    pub schema_version: u32,
    /// Identificador único desta sessão, também passado ao watchdog.
    pub session_id: Uuid,
    /// PID do processo principal (`NukaBoost.exe`) responsável pela sessão.
    pub main_pid: u32,
    /// Timestamp de criação de `main_pid`, usado para detectar reutilização
    /// de PID — ver [`crate::recovery::is_same_process`].
    pub main_pid_created_at_utc: String,
    /// Fase atual da transação.
    pub phase: JournalPhase,
    /// GUID do plano de energia original do usuário, a restaurar.
    pub original_scheme_guid: String,
    /// GUID do plano de energia temporário `NukaBoost Temporary`, a excluir
    /// ao final.
    pub temporary_scheme_guid: String,
    /// Timestamp de criação do próprio journal.
    pub created_at_utc: String,
}

impl RecoveryJournal {
    /// Monta um novo journal para a sessão atual, capturando o timestamp de
    /// criação do processo corrente automaticamente.
    pub fn new(
        session_id: Uuid,
        main_pid: u32,
        phase: JournalPhase,
        original_scheme_guid: windows::core::GUID,
        temporary_scheme_guid: windows::core::GUID,
    ) -> NukaResult<Self> {
        Ok(Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            session_id,
            main_pid,
            main_pid_created_at_utc: super::current_process_created_at_utc()?,
            phase,
            original_scheme_guid: format_guid(original_scheme_guid),
            temporary_scheme_guid: format_guid(temporary_scheme_guid),
            created_at_utc: now_iso8601()?,
        })
    }

    /// GUID decodificado do plano de energia original.
    pub fn original_scheme(&self) -> Result<windows::core::GUID, InvalidGuid> {
        parse_guid(&self.original_scheme_guid)
    }

    /// GUID decodificado do plano de energia temporário.
    pub fn temporary_scheme(&self) -> Result<windows::core::GUID, InvalidGuid> {
        parse_guid(&self.temporary_scheme_guid)
    }

    /// Verdadeiro se o PID registrado ainda corresponde ao mesmo processo
    /// que criou este journal (ver [`crate::recovery::is_same_process`]).
    pub fn main_process_still_running(&self) -> bool {
        super::is_same_process(self.main_pid, &self.main_pid_created_at_utc)
    }

    /// Carrega o journal de `%LocalAppData%\NukaBoost\recovery.json`, ou
    /// `None` se nenhuma sessão estiver pendente.
    pub fn load() -> NukaResult<Option<Self>> {
        Self::load_from(&paths::recovery_journal_file()?)
    }

    /// Grava este journal atomicamente. Deve ser chamado **antes** de
    /// ativar o plano temporário (passo 7 da seção 7: "Escrever e
    /// sincronizar o journal de recuperação").
    pub fn save(&self) -> NukaResult<()> {
        self.save_to(&paths::recovery_journal_file()?)
    }

    /// Remove o journal, marcando a sessão como encerrada de forma limpa
    /// (passo 12 da seção 8: "Apagar o journal").
    pub fn clear() -> NukaResult<()> {
        remove_if_exists(&paths::recovery_journal_file()?)
    }

    /// Variante testável de [`RecoveryJournal::load`].
    pub fn load_from(path: &Path) -> NukaResult<Option<Self>> {
        let journal: Option<Self> = read_json(path)?;
        if let Some(journal) = &journal {
            if journal.schema_version != JOURNAL_SCHEMA_VERSION {
                return Err(crate::error::NukaError::Other(format!(
                    "unsupported recovery journal schema {}; expected {}",
                    journal.schema_version, JOURNAL_SCHEMA_VERSION
                )));
            }
        }
        Ok(journal)
    }

    /// Variante testável de [`RecoveryJournal::save`].
    pub fn save_to(&self, path: &Path) -> NukaResult<()> {
        write_json_atomic(path, self)
    }

    /// Atualiza a fase de uma sessão existente sem aceitar a troca
    /// acidental do journal de uma sessão mais nova.
    pub fn update_phase(session_id: Uuid, phase: JournalPhase) -> NukaResult<()> {
        let mut journal = Self::load()?.ok_or_else(|| {
            crate::error::NukaError::Other(
                "cannot update recovery phase because the journal is missing".into(),
            )
        })?;
        if journal.session_id != session_id {
            return Err(crate::error::NukaError::Other(format!(
                "recovery journal belongs to session {}, not {session_id}",
                journal.session_id
            )));
        }
        journal.phase = phase;
        journal.save()
    }
}

fn now_iso8601() -> NukaResult<String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| crate::error::NukaError::Other(format!("failed to format timestamp: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::GUID;

    fn sample_journal() -> RecoveryJournal {
        RecoveryJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            session_id: Uuid::new_v4(),
            main_pid: std::process::id(),
            main_pid_created_at_utc: "2026-08-06T12:00:00Z".to_string(),
            phase: JournalPhase::Active,
            original_scheme_guid: format_guid(GUID::from_u128(0x1)),
            temporary_scheme_guid: format_guid(GUID::from_u128(0x2)),
            created_at_utc: "2026-08-06T12:00:01Z".to_string(),
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.json");

        let journal = sample_journal();
        journal.save_to(&path).unwrap();

        let loaded = RecoveryJournal::load_from(&path).unwrap();
        assert_eq!(loaded, Some(journal));
    }

    #[test]
    fn missing_journal_loads_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.json");

        assert_eq!(RecoveryJournal::load_from(&path).unwrap(), None);
    }

    #[test]
    fn rejects_an_unknown_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.json");
        let mut journal = sample_journal();
        journal.schema_version = JOURNAL_SCHEMA_VERSION + 1;
        write_json_atomic(&path, &journal).unwrap();

        assert!(RecoveryJournal::load_from(&path).is_err());
    }

    #[test]
    fn scheme_guids_decode_back() {
        let journal = sample_journal();
        assert_eq!(journal.original_scheme().unwrap(), GUID::from_u128(0x1));
        assert_eq!(journal.temporary_scheme().unwrap(), GUID::from_u128(0x2));
    }

    #[test]
    fn new_captures_current_process_identity() {
        let journal = RecoveryJournal::new(
            Uuid::new_v4(),
            std::process::id(),
            JournalPhase::Activating,
            GUID::from_u128(0x1),
            GUID::from_u128(0x2),
        )
        .unwrap();

        assert_eq!(journal.main_pid, std::process::id());
        assert!(journal.main_process_still_running());
    }
}

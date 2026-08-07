//! Protocolo de mensagens trocadas pelo Named Pipe (seção 11 do plano).
//!
//! Cada mensagem é um envelope JSON com `protocol_version` e `request_id`,
//! delimitado por tamanho na camada de transporte (ver
//! [`crate::ipc::framing`]). As chaves e valores deste protocolo — inclusive
//! os de [`StatusReport`] — nunca são localizados, mesmo quando a interface
//! gráfica estiver em português.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::i18n::Language;
use crate::power::SchemeProtectionStatus;
use crate::state::State;

/// Versão atual do protocolo IPC. Um cliente com versão diferente da do
/// servidor deve receber [`crate::error::NukaError::ProtocolMismatch`] em
/// vez de uma resposta malformada silenciosa.
pub const PROTOCOL_VERSION: u32 = 2;

/// Tamanho máximo de uma mensagem IPC, em bytes (seção 11: "Tamanho máximo
/// pequeno, por exemplo 64 KB"). Protege o servidor contra um cliente mal
/// comportado tentando enviar um payload arbitrariamente grande.
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024;

/// Envelope comum a toda mensagem trocada pelo pipe, seja requisição ou
/// resposta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Versão do protocolo usada pelo remetente.
    pub protocol_version: u32,
    /// Identificador único da requisição, ecoado na resposta correspondente
    /// para permitir correlação em clientes concorrentes.
    pub request_id: Uuid,
    /// Corpo específico da mensagem (um [`Command`] ou um [`Outcome`]).
    #[serde(flatten)]
    pub body: T,
}

impl<T> Envelope<T> {
    /// Envolve `body` em um novo envelope com a versão atual do protocolo e
    /// um `request_id` recém-gerado.
    pub fn new(body: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            body,
        }
    }

    /// Envolve `body` reaproveitando o `request_id` de uma requisição
    /// existente — usado pelo servidor ao montar a resposta.
    pub fn reply_to<U>(request: &Envelope<U>, body: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id,
            body,
        }
    }
}

/// Requisição enviada por `nukaboostctl` (ou qualquer outro cliente) ao
/// processo principal.
///
/// Corresponde diretamente aos comandos da seção 11 do plano que exigem o
/// processo principal em execução (`status`, `start`, `stop`, `toggle`,
/// `language`, `exit`) e aos comandos de leases da seção 12 (`acquire`,
/// `release`). Dois casos ficam de fora deliberadamente:
///
/// - `nukaboostctl startup enable|disable|status` não passa pelo pipe: é
///   pura manipulação de arquivo (o atalho em `FOLDERID_Startup`), então
///   `nukaboostctl` a executa diretamente via
///   [`nukaboost_core::startup`](crate::startup) — funciona mesmo com o
///   NukaBoost fechado, que é o caso mais comum ao configurar o Startup.
/// - `nukaboostctl run --` não tem representação própria aqui: o cliente o
///   implementa como `AcquireLease` seguido da execução local do processo
///   filho e, ao final, `ReleaseLease`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    /// `nukaboostctl status`
    Status,
    /// `nukaboostctl start` — ativação manual (equivalente ao clique
    /// esquerdo no ícone).
    Start,
    /// `nukaboostctl stop` / `stop --force`. `force` cancela todas as
    /// leases mesmo que outras existam; reservado para uso humano
    /// explícito, nunca para uso automático por uma skill de agente.
    Stop {
        #[serde(default)]
        force: bool,
    },
    /// `nukaboostctl toggle`
    Toggle,
    /// `nukaboostctl language en|pt`
    SetLanguage { language: Language },
    /// `nukaboostctl exit` — desativa, restaura tudo e encerra o processo.
    Exit,
    /// `nukaboostctl acquire --reason --owner --ttl`
    AcquireLease {
        reason: String,
        owner: String,
        ttl_seconds: u64,
    },
    /// `nukaboostctl release <lease-id>`
    ReleaseLease { lease_id: Uuid },
}

/// Resposta enviada pelo processo principal para uma [`Command`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Outcome {
    /// Resposta a `Status`.
    Status(Box<StatusReport>),
    /// Confirmação genérica de sucesso sem dados adicionais (`Start`,
    /// `Stop`, `Toggle`, `SetLanguage`, `Exit`).
    Ack,
    /// Resposta a `AcquireLease`.
    LeaseAcquired { lease_id: Uuid },
    /// Resposta a `ReleaseLease`.
    LeaseReleased,
    /// Falha ao processar o comando.
    Error { code: ErrorCode, message: String },
}

/// Código de erro estável, pensado para ser inspecionado por scripts e
/// agentes (não apenas exibido a humanos). `SafetyAckRequired` é o valor
/// explicitamente exigido pela seção 4 do plano quando o CLI tenta ativar
/// antes da confirmação do aviso de segurança.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    SafetyAckRequired,
    PolicyDenied,
    InvalidTransition,
    ProtectionNotConfirmed,
    LeaseNotFound,
    ProcessNotRunning,
    Internal,
}

impl From<&crate::error::NukaError> for ErrorCode {
    fn from(error: &crate::error::NukaError) -> Self {
        use crate::error::NukaError::*;
        match error {
            SafetyAckRequired => Self::SafetyAckRequired,
            PolicyDenied => Self::PolicyDenied,
            InvalidTransition { .. } => Self::InvalidTransition,
            ProtectionNotConfirmed { .. } => Self::ProtectionNotConfirmed,
            LeaseNotFound(_) => Self::LeaseNotFound,
            ProcessNotRunning => Self::ProcessNotRunning,
            Win32 { .. }
            | Io { .. }
            | Serialization(_)
            | MessageTooLarge { .. }
            | ProtocolMismatch { .. }
            | Other(_) => Self::Internal,
        }
    }
}

/// Fonte de energia atual do sistema, relatada em `power_source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerSource {
    /// Conectado à tomada (AC).
    Ac,
    /// Executando na bateria (DC).
    Battery,
}

/// Estado individual de cada proteção obrigatória, exibido em
/// `nukaboostctl status --json` sob a chave `protections`.
///
/// Espelha exatamente o objeto `protections` da seção 11 do plano — a
/// ordem e os nomes dos campos abaixo são o formato público estável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectionsReport {
    pub temporary_scheme_active: bool,
    pub lid_action_ac: bool,
    pub lid_action_dc: bool,
    pub idle_sleep_ac: bool,
    pub idle_sleep_dc: bool,
    pub system_required_ac: bool,
    pub system_required_dc: bool,
    pub low_battery_action: bool,
    pub system_power_request: bool,
    pub execution_power_request: bool,
    pub thread_execution_state: bool,
    pub display_request_absent: bool,
}

impl ProtectionsReport {
    /// Monta o relatório a partir do estado lido do plano temporário
    /// ([`SchemeProtectionStatus`]) e do estado das demais proteções
    /// redundantes, mantidas fora do módulo `power` pelo orquestrador de
    /// `apps/nukaboost`.
    pub fn new(
        temporary_scheme_active: bool,
        scheme: SchemeProtectionStatus,
        system_power_request: bool,
        execution_power_request: bool,
        thread_execution_state: bool,
    ) -> Self {
        Self {
            temporary_scheme_active,
            lid_action_ac: scheme.lid_action_ac,
            lid_action_dc: scheme.lid_action_dc,
            idle_sleep_ac: scheme.idle_sleep_ac,
            idle_sleep_dc: scheme.idle_sleep_dc,
            system_required_ac: scheme.system_required_ac,
            system_required_dc: scheme.system_required_dc,
            low_battery_action: scheme.low_battery_action_ac && scheme.low_battery_action_dc,
            system_power_request,
            execution_power_request,
            thread_execution_state,
            // Sempre `true` por construção: o NukaBoost nunca cria uma
            // `PowerRequestDisplayRequired` nem usa `ES_DISPLAY_REQUIRED`
            // (seção 1: "Permitir que a tela apague normalmente").
            display_request_absent: true,
        }
    }

    /// Verdadeiro somente se todas as proteções obrigatórias estiverem em
    /// vigor. O ícone da bandeja nunca deve mostrar "Active" quando este
    /// método retorna `false`.
    pub const fn all_ok(&self) -> bool {
        self.temporary_scheme_active
            && self.lid_action_ac
            && self.lid_action_dc
            && self.idle_sleep_ac
            && self.idle_sleep_dc
            && self.system_required_ac
            && self.system_required_dc
            && self.low_battery_action
            && self.system_power_request
            && self.execution_power_request
            && self.thread_execution_state
            && self.display_request_absent
    }
}

/// Corpo completo de `nukaboostctl status --json`, também usado como a
/// carga de [`Outcome::Status`].
///
/// Reproduz exatamente a forma do JSON de exemplo da seção 11 do plano.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub protocol_version: u32,
    pub app_version: String,
    pub process_running: bool,
    pub state: State,
    pub language: Language,
    pub manual_hold: bool,
    pub leases: usize,
    pub power_source: PowerSource,
    pub battery_percent: Option<u8>,
    pub battery_saver: bool,
    pub protections: ProtectionsReport,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_protections(all_ok: bool) -> ProtectionsReport {
        ProtectionsReport {
            temporary_scheme_active: all_ok,
            lid_action_ac: all_ok,
            lid_action_dc: all_ok,
            idle_sleep_ac: all_ok,
            idle_sleep_dc: all_ok,
            system_required_ac: all_ok,
            system_required_dc: all_ok,
            low_battery_action: all_ok,
            system_power_request: all_ok,
            execution_power_request: all_ok,
            thread_execution_state: all_ok,
            display_request_absent: true,
        }
    }

    #[test]
    fn envelope_reply_preserves_request_id() {
        let request = Envelope::new(Command::Status);
        let response = Envelope::reply_to(&request, Outcome::Ack);
        assert_eq!(request.request_id, response.request_id);
    }

    #[test]
    fn command_json_uses_snake_case_tag() {
        let json = serde_json::to_string(&Command::Stop { force: true }).unwrap();
        assert!(json.contains("\"command\":\"stop\""));
        assert!(json.contains("\"force\":true"));
    }

    #[test]
    fn safety_ack_required_serializes_to_documented_value() {
        let json = serde_json::to_string(&ErrorCode::SafetyAckRequired).unwrap();
        assert_eq!(json, "\"safety_ack_required\"");
    }

    #[test]
    fn protections_all_ok_requires_every_field() {
        assert!(sample_protections(true).all_ok());

        let mut degraded = sample_protections(true);
        degraded.idle_sleep_dc = false;
        assert!(!degraded.all_ok());
    }

    #[test]
    fn status_report_round_trips_through_json() {
        let report = StatusReport {
            protocol_version: PROTOCOL_VERSION,
            app_version: "1.0.0".to_string(),
            process_running: true,
            state: State::Active,
            language: Language::En,
            manual_hold: false,
            leases: 1,
            power_source: PowerSource::Battery,
            battery_percent: Some(42),
            battery_saver: true,
            protections: sample_protections(true),
            last_error: None,
        };

        let json = serde_json::to_string(&report).unwrap();
        let parsed: StatusReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.state, State::Active);
        assert_eq!(parsed.power_source, PowerSource::Battery);
        assert_eq!(parsed.battery_percent, Some(42));
    }
}

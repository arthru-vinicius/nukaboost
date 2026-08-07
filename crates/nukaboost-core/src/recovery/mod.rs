//! Recuperação contra falhas (seção 9 do plano): journal atômico e
//! identificação robusta de processos para o watchdog.
//!
//! O watchdog (implementado em `apps/nukaboost`, iniciado como
//! `NukaBoost.exe --watchdog <pid> <session-id>`) usa as funções deste
//! módulo para decidir com segurança se o processo principal ainda está
//! vivo — nunca confiando apenas no PID, que o Windows pode reciclar.

pub mod journal;
pub mod runonce;

pub use journal::{JournalPhase, RecoveryJournal};

use uuid::Uuid;
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::error::{NukaError, NukaResult};
use crate::power::scheme::PowerScheme;

/// Diferença, em intervalos de 100 ns, entre a época do Windows
/// (1601-01-01) e a época Unix (1970-01-01). Constante bem conhecida usada
/// para converter `FILETIME` em segundos desde a época Unix.
const FILETIME_EPOCH_DIFF_100NS: i64 = 116_444_736_000_000_000;

/// Converte um `FILETIME` (intervalos de 100 ns desde 1601-01-01) para uma
/// string ISO 8601/RFC 3339 em UTC, como `"2026-08-06T12:00:00Z"`.
fn filetime_to_iso8601(filetime: FILETIME) -> NukaResult<String> {
    let ticks_100ns = ((filetime.dwHighDateTime as i64) << 32) | (filetime.dwLowDateTime as i64);
    // Preserve os 100 ns do FILETIME. Truncar para segundos permitiria que
    // um PID reciclado muito rapidamente parecesse ser o mesmo processo.
    let unix_nanoseconds = (ticks_100ns as i128 - FILETIME_EPOCH_DIFF_100NS as i128) * 100;

    let datetime = time::OffsetDateTime::from_unix_timestamp_nanos(unix_nanoseconds)
        .map_err(|e| NukaError::Other(format!("invalid process timestamp: {e}")))?;
    datetime
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| NukaError::Other(format!("failed to format timestamp: {e}")))
}

/// Timestamp de criação (ISO 8601/UTC) do processo atual, gravado no journal
/// ao iniciar uma sessão ativa.
pub fn current_process_created_at_utc() -> NukaResult<String> {
    // SAFETY: `GetCurrentProcess` retorna um pseudo-handle válido que não
    // precisa (e não deve) ser fechado.
    let handle = unsafe { GetCurrentProcess() };
    read_creation_time(handle)
}

/// Timestamp de criação (ISO 8601/UTC) de um processo arbitrário pelo PID,
/// ou `None` se o processo não existir/não puder ser aberto (por exemplo,
/// já encerrado ou pertencente a outro usuário).
pub fn process_created_at_utc(pid: u32) -> Option<String> {
    // SAFETY: `OpenProcess` valida o PID internamente; nenhuma outra
    // pré-condição é necessária.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let result = read_creation_time(handle).ok();
    // SAFETY: `handle` foi aberto com sucesso por `OpenProcess` logo acima
    // e ainda não foi fechado.
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

fn read_creation_time(handle: windows::Win32::Foundation::HANDLE) -> NukaResult<String> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    // SAFETY: os quatro ponteiros de saída apontam para `FILETIME` válidos
    // na pilha; `handle` é válido durante toda a chamada síncrona.
    unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) }
        .map_err(|e| NukaError::from_win32("GetProcessTimes", &e))?;

    filetime_to_iso8601(creation)
}

/// Verdadeiro se o processo `pid` ainda existir **e** seu horário de
/// criação bater com `expected_created_at_utc`.
///
/// Esta é a defesa contra reutilização de PID exigida pela seção 9: um PID
/// sozinho não é suficiente para identificar "o mesmo processo principal
/// que escreveu o journal", porque o Windows recicla PIDs livremente depois
/// que um processo termina.
pub fn is_same_process(pid: u32, expected_created_at_utc: &str) -> bool {
    match process_created_at_utc(pid) {
        Some(actual) => actual == expected_created_at_utc,
        None => false,
    }
}

/// Resultado de uma tentativa de recuperação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    /// Não havia journal pendente, ou ele pertencia a uma sessão diferente
    /// (mais nova) da que o chamador esperava — nada a fazer.
    NothingPending,
    /// O plano original foi restaurado com sucesso e os artefatos da sessão
    /// (`RunOnce`, journal) foram limpos.
    Recovered,
}

/// Restaura o plano de energia original e limpa os artefatos de uma sessão
/// interrompida abruptamente.
///
/// Compartilhada entre o watchdog (quando o processo principal morre
/// inesperadamente, seção 9) e o modo `--recover-only` (quando o próprio
/// Windows reinicia antes de qualquer um dos dois rodar). Se
/// `expected_session_id` for informado e não bater com o `session_id` do
/// journal, a recuperação é ignorada — o journal pertence a uma sessão mais
/// nova que já substituiu a que este chamador estava observando.
///
/// Em caso de falha ao restaurar, o journal **não** é apagado (seção 9:
/// "Manter logs da recuperação"; seção 15: "Não apagar o recovery
/// journal"), para que uma tentativa futura ainda tenha a informação
/// necessária para tentar de novo.
pub fn perform_recovery(expected_session_id: Option<Uuid>) -> NukaResult<RecoveryOutcome> {
    let Some(journal) = RecoveryJournal::load()? else {
        return Ok(RecoveryOutcome::NothingPending);
    };
    if let Some(expected) = expected_session_id {
        if journal.session_id != expected {
            return Ok(RecoveryOutcome::NothingPending);
        }
    }

    // Uma segunda inicialização normal ou uma entrada RunOnce atrasada não
    // pode desmontar a sessão de um processo principal ainda saudável.
    if journal.main_process_still_running() {
        return Ok(RecoveryOutcome::NothingPending);
    }

    perform_recovery_from_journal(journal)
}

/// Repete a recuperação durante o shutdown do próprio processo que criou
/// o journal. É a única exceção segura à regra de não recuperar sessões
/// vivas e permite uma segunda tentativa antes de encerrar após rollback.
pub fn perform_recovery_owned_by_current_process() -> NukaResult<RecoveryOutcome> {
    let Some(journal) = RecoveryJournal::load()? else {
        return Ok(RecoveryOutcome::NothingPending);
    };
    if journal.main_pid != std::process::id() || !journal.main_process_still_running() {
        return Err(NukaError::Other(
            "pending recovery journal is not owned by the current process".into(),
        ));
    }
    perform_recovery_from_journal(journal)
}

fn perform_recovery_from_journal(journal: RecoveryJournal) -> NukaResult<RecoveryOutcome> {
    let original = journal
        .original_scheme()
        .map(PowerScheme::from_guid)
        .map_err(|e| NukaError::Other(format!("invalid original scheme GUID in journal: {e}")))?;
    original.activate()?;

    let active_now = PowerScheme::active()?;
    if active_now.guid() != original.guid() {
        return Err(NukaError::ProtectionNotConfirmed {
            setting: "original_scheme_restored_by_recovery",
            expected: 1,
            actual: 0,
        });
    }

    let temporary = journal
        .temporary_scheme()
        .map(PowerScheme::from_guid)
        .map_err(|e| NukaError::Other(format!("invalid temporary scheme GUID in journal: {e}")))?;
    temporary.delete()?;

    runonce::unregister()?;
    RecoveryJournal::clear()?;

    Ok(RecoveryOutcome::Recovered)
}

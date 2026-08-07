//! Orquestrador central: liga `power` + `state` + `recovery` + a bandeja,
//! implementando a transação de ativação (seção 7) e de desativação
//! (seção 8) do plano. É o único lugar de `apps/nukaboost` que decide
//! *quando* ativar/desativar — a bandeja e o servidor IPC apenas chamam
//! seus métodos.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;

use nukaboost_core::config::AppConfig;
use nukaboost_core::error::{NukaError, NukaResult};
use nukaboost_core::i18n::Language;
use nukaboost_core::ipc::protocol::{ProtectionsReport, StatusReport};
use nukaboost_core::power::execution_state::ExecutionStateThread;
use nukaboost_core::power::request::PowerRequestGuard;
use nukaboost_core::power::scheme::PowerScheme;
use nukaboost_core::power::{self, settings};
use nukaboost_core::recovery::{runonce, JournalPhase, RecoveryJournal};
use nukaboost_core::state::{is_effectively_active, LeaseManager, State, StateMachine};

use windows::Win32::Foundation::HWND;

use nukaboost_win32::tray::{TrayIcon, TrayViewState, TrayVisualState};

use crate::power_source;
use crate::watchdog;

/// Versão exibida em `About` e em `status --json`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Intervalo do monitoramento periódico (seção 10: "por exemplo a cada
/// cinco segundos").
pub const MONITOR_INTERVAL: Duration = Duration::from_secs(5);

/// Tudo que existe apenas enquanto o NukaBoost está `Active`, na ordem
/// exata em que deve ser desfeito ao desativar.
struct ActiveSession {
    session_id: Uuid,
    original_scheme: PowerScheme,
    temporary_scheme: PowerScheme,
    system_request: Option<PowerRequestGuard>,
    execution_request: Option<PowerRequestGuard>,
    execution_state_thread: Option<ExecutionStateThread>,
    watchdog: watchdog::WatchdogHandle,
    monitor_stop: Option<std::sync::mpsc::Sender<()>>,
    monitor_handle: Option<std::thread::JoinHandle<()>>,
}

/// Orquestrador central, compartilhado (via `Arc`) entre a thread de UI, o
/// servidor IPC e o monitor periódico.
pub struct Orchestrator {
    state: StateMachine,
    leases: Mutex<LeaseManager>,
    config: Mutex<AppConfig>,
    session: Mutex<Option<ActiveSession>>,
    manual_hold: AtomicBool,
    safety_acknowledged_this_run: AtomicBool,
    tray_icon: Mutex<TrayIcon>,
    tray_view: Arc<Mutex<TrayViewState>>,
    last_error: Mutex<Option<String>>,
    hwnd: HWND,
}

// SAFETY: `HWND` é um identificador de kernel opaco; seguro entre threads.
unsafe impl Send for Orchestrator {}
unsafe impl Sync for Orchestrator {}

impl Orchestrator {
    /// Cria o orquestrador, carregando a configuração persistida e
    /// preparando (mas não exibindo) o ícone da bandeja. O estado inicial é
    /// sempre [`State::Inactive`] (seção 3).
    pub fn new(hwnd: HWND, tray_view: Arc<Mutex<TrayViewState>>) -> NukaResult<Arc<Self>> {
        let config = AppConfig::load()?;

        let safety_acknowledged = config.safety_ack_confirmed;
        let orchestrator = Arc::new(Self {
            state: StateMachine::new(),
            leases: Mutex::new(LeaseManager::new()),
            tray_icon: Mutex::new(TrayIcon::new(hwnd)),
            config: Mutex::new(config),
            session: Mutex::new(None),
            manual_hold: AtomicBool::new(false),
            safety_acknowledged_this_run: AtomicBool::new(safety_acknowledged),
            tray_view,
            last_error: Mutex::new(None),
            hwnd,
        });

        // Expiração de lease é uma mudança de intenção, não apenas uma
        // estatística. A thread usa `Weak`, portanto não prolonga a vida do
        // processo nem cria um ciclo de `Arc`.
        let weak = Arc::downgrade(&orchestrator);
        std::thread::Builder::new()
            .name("nukaboost-lease-housekeeping".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let Some(orchestrator) = weak.upgrade() else {
                    break;
                };
                let expired = orchestrator
                    .leases
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .reap_expired();
                if expired > 0 {
                    tracing::info!(expired, "expired leases were reaped");
                    if let Err(error) = orchestrator.reconcile() {
                        tracing::error!(%error, "failed to reconcile after lease expiration");
                    }
                }
            })
            .map_err(|e| NukaError::Other(format!("failed to start lease housekeeping: {e}")))?;

        Ok(orchestrator)
    }

    /// Janela oculta de mensagens, usada como proprietária dos diálogos.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Idioma atual da interface.
    pub fn language(&self) -> Language {
        self.config
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .language
    }

    /// Troca o idioma, persiste a preferência e atualiza a bandeja.
    pub fn set_language(&self, language: Language) -> NukaResult<()> {
        {
            let mut config = self.config.lock().unwrap_or_else(|p| p.into_inner());
            config.language = language;
            config.save()?;
        }
        self.refresh_tray();
        Ok(())
    }

    /// Verdadeiro se o aviso de segurança já foi confirmado anteriormente.
    pub fn is_safety_ack_confirmed(&self) -> bool {
        self.safety_acknowledged_this_run.load(Ordering::SeqCst)
    }

    /// Registra a confirmação do aviso de segurança (seção 4).
    pub fn confirm_safety_ack(&self, acknowledged: bool, dont_show_again: bool) -> NukaResult<()> {
        if !acknowledged {
            return Ok(());
        }
        self.safety_acknowledged_this_run
            .store(true, Ordering::SeqCst);
        if !dont_show_again {
            return Ok(());
        }
        let mut config = self.config.lock().unwrap_or_else(|p| p.into_inner());
        config.safety_ack_confirmed = true;
        config.save()
    }

    /// Estado observável atual.
    pub fn state(&self) -> State {
        self.state.current()
    }

    /// Número de leases ainda válidas.
    pub fn active_lease_count(&self) -> usize {
        self.leases
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .active_count()
    }

    // ------------------------------------------------------------------
    // Ativação manual / leases (seções 3 e 12)
    // ------------------------------------------------------------------

    /// `Start`/`Iniciar` — clique esquerdo, item de menu ou `nukaboostctl start`.
    pub fn manual_start(self: &Arc<Self>) -> NukaResult<()> {
        if !self.is_safety_ack_confirmed() {
            return Err(NukaError::SafetyAckRequired);
        }
        self.manual_hold.store(true, Ordering::SeqCst);
        if let Err(error) = self.ensure_active() {
            self.manual_hold.store(false, Ordering::SeqCst);
            return Err(error);
        }
        Ok(())
    }

    /// `Stop`/`Parar` — clique manual tem prioridade humana: cancela **todas**
    /// as leases quando `force` é verdadeiro (seção 12: "`stop --force`
    /// existe para emergência, mas não deve ser usado pela skill").
    pub fn manual_stop(self: &Arc<Self>, force: bool) -> NukaResult<()> {
        self.manual_hold.store(false, Ordering::SeqCst);
        if force {
            self.leases
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .release_all();
        }
        self.reconcile()
    }

    /// Parada originada diretamente pela interface. A intenção humana tem
    /// prioridade e sempre revoga leases automáticas.
    pub fn human_stop(self: &Arc<Self>) -> NukaResult<()> {
        self.manual_stop(true)
    }

    /// `Toggle`/alterna entre ativo e inativo.
    pub fn toggle(self: &Arc<Self>) -> NukaResult<()> {
        if matches!(self.state(), State::Active | State::Error) {
            self.human_stop()
        } else {
            self.manual_start()
        }
    }

    /// Concede uma lease e garante que o NukaBoost esteja ativo.
    pub fn acquire_lease(
        self: &Arc<Self>,
        reason: String,
        owner: String,
        ttl: Duration,
    ) -> NukaResult<Uuid> {
        if !self.is_safety_ack_confirmed() {
            return Err(NukaError::SafetyAckRequired);
        }
        let id = self
            .leases
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .acquire(reason, owner, ttl)?;
        if let Err(error) = self.ensure_active() {
            let _ = self
                .leases
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .release(id);
            return Err(error);
        }
        Ok(id)
    }

    /// Libera uma lease específica e desativa se não houver mais motivo
    /// para continuar ativo (seção 12: fórmula `manual_hold OR leases > 0`).
    pub fn release_lease(self: &Arc<Self>, id: Uuid) -> NukaResult<()> {
        self.leases
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .release(id)?;
        self.reconcile()
    }

    /// Garante que o estado efetivo (seção 12) esteja refletido: ativa se
    /// deveria estar ativo e não está, desativa se não deveria e está.
    fn reconcile(self: &Arc<Self>) -> NukaResult<()> {
        let manual_hold = self.manual_hold.load(Ordering::SeqCst);
        let leases = self.active_lease_count();
        let should_be_active = is_effectively_active(manual_hold, leases);

        let has_session = self
            .session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some();
        match (should_be_active, self.state(), has_session) {
            (true, State::Inactive, _) | (true, State::Error, false) => self.activate(),
            (false, State::Active, _) | (false, State::Error, true) => self.deactivate(),
            _ => Ok(()),
        }
    }

    fn ensure_active(self: &Arc<Self>) -> NukaResult<()> {
        match self.state() {
            State::Active => Ok(()),
            State::Inactive => self.activate(),
            State::Error
                if self
                    .session
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_none() =>
            {
                self.activate()
            }
            State::Error => Err(NukaError::Other(
                "a previous session still requires recovery before activation".into(),
            )),
            State::Activating | State::Deactivating => Err(NukaError::InvalidTransition {
                from: "transition in progress".to_string(),
                to: "active".to_string(),
            }),
        }
    }

    /// Desativa incondicionalmente e libera todos os recursos — usado por
    /// `Exit`/`Fechar` e pela desinstalação (seção 3: "`Exit/Fechar` primeiro
    /// desativa e restaura tudo, depois encerra").
    pub fn shutdown(self: &Arc<Self>) -> NukaResult<()> {
        self.manual_hold.store(false, Ordering::SeqCst);
        self.leases
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .release_all();
        // Uma transição concorrente curta (IPC/monitor) não pode permitir
        // que o processo feche antes do rollback. Aguarda de forma limitada.
        for _ in 0..100 {
            if !matches!(self.state(), State::Activating | State::Deactivating) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        match self.state() {
            State::Active | State::Error => self.deactivate()?,
            State::Activating | State::Deactivating => {
                return Err(NukaError::Other(
                    "timed out waiting for the current power transition".into(),
                ));
            }
            State::Inactive => {}
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Transação de ativação (seção 7)
    // ------------------------------------------------------------------

    fn activate(self: &Arc<Self>) -> NukaResult<()> {
        self.resolve_pending_recovery_before_activation()?;
        tracing::info!("starting activation transaction");
        // Passo 1: mutex interno de mudança de estado.
        let guard = self.state.begin_transition(State::Activating)?;
        self.refresh_tray();

        match self.try_activate() {
            Ok(session) => {
                *self.session.lock().unwrap_or_else(|p| p.into_inner()) = Some(session);
                *self.last_error.lock().unwrap_or_else(|p| p.into_inner()) = None;
                guard.commit(State::Active)?;
                self.refresh_tray();
                self.notify_activated();
                tracing::info!("activation transaction committed");
                // A intenção pode ter mudado enquanto chamadas Win32
                // estavam em andamento (por exemplo, uma lease liberada).
                // Reconciliar após o commit fecha essa janela de corrida.
                let should_remain_active = is_effectively_active(
                    self.manual_hold.load(Ordering::SeqCst),
                    self.active_lease_count(),
                );
                if !should_remain_active {
                    self.deactivate()?;
                }
                Ok(())
            }
            Err(err) => {
                tracing::error!(error = %err, "activation transaction failed");
                *self.last_error.lock().unwrap_or_else(|p| p.into_inner()) = Some(err.to_string());
                // O guard cai em `Error` automaticamente ao ser descartado
                // sem `commit`, mas fazemos explícito para maior clareza.
                let _ = guard.commit(State::Error);
                self.refresh_tray();
                Err(err)
            }
        }
    }

    /// Nunca sobrescreve um journal de uma ativação anterior. Quando ele é
    /// desta própria instância, tenta concluir o rollback antes de começar
    /// uma nova sessão; journals de outro processo permanecem intocados.
    fn resolve_pending_recovery_before_activation(&self) -> NukaResult<()> {
        let Some(journal) = RecoveryJournal::load()? else {
            return Ok(());
        };
        if journal.main_pid == std::process::id() && journal.main_process_still_running() {
            nukaboost_core::recovery::perform_recovery_owned_by_current_process()?;
        } else {
            nukaboost_core::recovery::perform_recovery(Some(journal.session_id))?;
        }
        if RecoveryJournal::load()?.is_some() {
            return Err(NukaError::Other(
                "a previous recovery journal is still pending; refusing to overwrite it".into(),
            ));
        }
        Ok(())
    }

    /// Executa os passos 2 a 14 da transação de ativação. Qualquer falha
    /// aciona o rollback e propaga o erro; `activate` decide o estado final.
    fn try_activate(self: &Arc<Self>) -> NukaResult<ActiveSession> {
        // Passo 2 e 3: ler o plano original e consultar as políticas sobre
        // esse GUID e sobre cada configuração que será alterada.
        let original_scheme = PowerScheme::active()?;
        power::check_policy_allows_scheme_changes(original_scheme.guid())?;

        // Passo 4: duplicar o plano.
        let temporary_scheme = original_scheme.duplicate()?;

        // Passos 5 e 6: configurar e confirmar por releitura.
        if let Err(e) = settings::apply_and_confirm(temporary_scheme.guid()) {
            return match temporary_scheme.delete() {
                Ok(()) => Err(e),
                Err(cleanup) => Err(NukaError::Other(format!(
                    "power-plan configuration failed: {e}; temporary scheme cleanup also failed: {cleanup}"
                ))),
            };
        }

        let session_id = Uuid::new_v4();

        // Passo 7: escrever e sincronizar o journal de recuperação.
        if let Err(e) = RecoveryJournal::new(
            session_id,
            std::process::id(),
            JournalPhase::Activating,
            original_scheme.guid(),
            temporary_scheme.guid(),
        )
        .and_then(|j| j.save())
        {
            return match temporary_scheme.delete() {
                Ok(()) => Err(e),
                Err(cleanup) => Err(NukaError::Other(format!(
                    "recovery journal creation failed: {e}; temporary scheme cleanup also failed: {cleanup}"
                ))),
            };
        }

        // Passo 8: registrar recuperação no próximo logon.
        if let Err(e) = runonce::register(session_id) {
            return match temporary_scheme
                .delete()
                .and_then(|()| runonce::unregister())
                .and_then(|()| RecoveryJournal::clear())
            {
                Ok(()) => Err(e),
                Err(cleanup) => Err(NukaError::Other(format!(
                    "RunOnce registration failed: {e}; cleanup remains pending: {cleanup}"
                ))),
            };
        }

        // Passo 9: iniciar o watchdog.
        let watchdog_handle = match watchdog::spawn(std::process::id(), session_id) {
            Ok(handle) => handle,
            Err(e) => {
                return match temporary_scheme
                    .delete()
                    .and_then(|()| runonce::unregister())
                    .and_then(|()| RecoveryJournal::clear())
                {
                    Ok(()) => Err(e),
                    Err(cleanup) => Err(NukaError::Other(format!(
                        "watchdog startup failed: {e}; cleanup remains pending: {cleanup}"
                    ))),
                };
            }
        };

        // A partir daqui, qualquer falha aciona o rollback completo
        // (watchdog, RunOnce, journal e plano temporário).
        let result = self.finish_activation(&temporary_scheme, session_id);
        match result {
            Ok((system_request, execution_request, execution_state_thread)) => {
                if let Err(error) = RecoveryJournal::update_phase(session_id, JournalPhase::Active)
                {
                    drop(execution_state_thread);
                    drop(execution_request);
                    drop(system_request);
                    return Err(self.rollback_failed_activation(
                        &original_scheme,
                        &temporary_scheme,
                        &watchdog_handle,
                        error,
                    ));
                }
                let (monitor_stop, monitor_handle) =
                    crate::monitor::spawn(Arc::clone(self), temporary_scheme.guid());
                Ok(ActiveSession {
                    session_id,
                    original_scheme,
                    temporary_scheme,
                    system_request: Some(system_request),
                    execution_request: Some(execution_request),
                    execution_state_thread: Some(execution_state_thread),
                    watchdog: watchdog_handle,
                    monitor_stop: Some(monitor_stop),
                    monitor_handle: Some(monitor_handle),
                })
            }
            Err(e) => Err(self.rollback_failed_activation(
                &original_scheme,
                &temporary_scheme,
                &watchdog_handle,
                e,
            )),
        }
    }

    /// Restaura primeiro e só desarma os mecanismos de recuperação depois
    /// que a restauração foi confirmada. Se qualquer passo crítico falhar,
    /// o filho watchdog, o RunOnce e o journal permanecem disponíveis.
    fn rollback_failed_activation(
        &self,
        original_scheme: &PowerScheme,
        temporary_scheme: &PowerScheme,
        watchdog_handle: &watchdog::WatchdogHandle,
        activation_error: NukaError,
    ) -> NukaError {
        let rollback = (|| -> NukaResult<()> {
            original_scheme.activate()?;
            let active = PowerScheme::active()?;
            if active.guid() != original_scheme.guid() {
                return Err(NukaError::ProtectionNotConfirmed {
                    setting: "original_scheme_restored_after_activation_failure",
                    expected: 1,
                    actual: 0,
                });
            }
            temporary_scheme.delete()?;
            watchdog_handle.terminate()?;
            runonce::unregister()?;
            RecoveryJournal::clear()?;
            Ok(())
        })();

        match rollback {
            Ok(()) => activation_error,
            Err(rollback_error) => NukaError::Other(format!(
                "activation failed: {activation_error}; rollback remains pending: {rollback_error}"
            )),
        }
    }

    /// Passos 10 a 13: ativar o plano temporário, confirmar, criar as Power
    /// Requests e iniciar a thread de execution state.
    fn finish_activation(
        &self,
        temporary_scheme: &PowerScheme,
        _session_id: Uuid,
    ) -> NukaResult<(PowerRequestGuard, PowerRequestGuard, ExecutionStateThread)> {
        // Passo 10: ativar o plano temporário.
        temporary_scheme.activate()?;

        // Passo 11: confirmar que ele é realmente o ativo.
        let active_now = PowerScheme::active()?;
        if active_now.guid() != temporary_scheme.guid() {
            return Err(NukaError::ProtectionNotConfirmed {
                setting: "temporary_scheme_active",
                expected: 1,
                actual: 0,
            });
        }

        // Passo 12: as duas Power Requests redundantes.
        let system_request =
            PowerRequestGuard::system_required("NukaBoost keeps the system awake")?;
        let execution_request =
            PowerRequestGuard::execution_required("NukaBoost keeps the process running")?;

        // Passo 13: thread de SetThreadExecutionState.
        let execution_state_thread = ExecutionStateThread::start()?;

        Ok((system_request, execution_request, execution_state_thread))
    }

    // ------------------------------------------------------------------
    // Desativação (seção 8)
    // ------------------------------------------------------------------

    fn deactivate(self: &Arc<Self>) -> NukaResult<()> {
        let guard = self.state.begin_transition(State::Deactivating)?;
        self.refresh_tray();

        let session = self
            .session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        let Some(mut session) = session else {
            // Uma ativação que falhou antes de construir `ActiveSession`
            // ainda pode ter deixado um journal recuperável.
            nukaboost_core::recovery::perform_recovery_owned_by_current_process()?;
            guard.commit(State::Inactive)?;
            self.refresh_tray();
            return Ok(());
        };

        match self.try_deactivate(&mut session) {
            Ok(()) => {
                guard.commit(State::Inactive)?;
                self.refresh_tray();
                self.notify_deactivated();
                tracing::info!("deactivation transaction committed");
                Ok(())
            }
            Err(e) => {
                *self.session.lock().unwrap_or_else(|p| p.into_inner()) = Some(session);
                *self.last_error.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
                let _ = guard.commit(State::Error);
                self.refresh_tray();
                Err(e)
            }
        }
    }

    fn try_deactivate(&self, session: &mut ActiveSession) -> NukaResult<()> {
        tracing::info!(session_id = %session.session_id, "deactivating session");

        RecoveryJournal::update_phase(session.session_id, JournalPhase::Deactivating)?;

        // Passos 2 e 3: as Power Requests continuam válidas durante a
        // restauração (só são descartadas no fim desta função) e o monitor
        // é parado primeiro para não competir com a restauração manual.
        if let Some(stop) = session.monitor_stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = session.monitor_handle.take() {
            let _ = handle.join();
        }

        // Passo 4: reativar o plano original.
        session.original_scheme.activate()?;

        // Passo 5: confirmar por leitura.
        let active_now = PowerScheme::active()?;
        if active_now.guid() != session.original_scheme.guid() {
            return Err(NukaError::ProtectionNotConfirmed {
                setting: "original_scheme_restored",
                expected: 1,
                actual: 0,
            });
        }

        // Passo 6: excluir o plano temporário.
        session.temporary_scheme.delete()?;

        // Passo 7: encerrar a thread de execution state.
        if let Some(thread) = session.execution_state_thread.take() {
            thread.stop();
        }

        // Passos 8 e 9: limpar as Power Requests explicitamente, registrando
        // uma falha em vez de escondê-la no `Drop`.
        if let Some(request) = session.execution_request.take() {
            request.clear()?;
        }
        if let Some(request) = session.system_request.take() {
            request.clear()?;
        }

        // Passo 10: desarmar o watchdog desta sessão.
        session.watchdog.terminate()?;

        // Passo 11: remover recuperação de próximo logon.
        runonce::unregister()?;

        // Passo 12: apagar o journal.
        RecoveryJournal::clear()?;

        Ok(())
    }

    // ------------------------------------------------------------------
    // Bandeja e status
    // ------------------------------------------------------------------

    /// Exibe o ícone pela primeira vez (inicialização) ou o recria depois
    /// que o Explorer reinicia (seção 17: `TaskbarCreated`).
    pub fn show_initial_icon(&self) {
        self.refresh_tray();
    }

    fn refresh_tray(&self) {
        let language = self.language();
        let state = self.state();
        {
            let mut view = self.tray_view.lock().unwrap_or_else(|p| p.into_inner());
            view.active = state == State::Active;
            view.language = language;
        }

        let visual = match state {
            State::Inactive | State::Activating => TrayVisualState::Inactive,
            State::Active | State::Deactivating => TrayVisualState::Active,
            State::Error => TrayVisualState::Error,
        };
        let mut icon = self.tray_icon.lock().unwrap_or_else(|p| p.into_inner());
        let _ = icon.set_state(visual, language);
    }

    fn notify_activated(&self) {
        let strings = self.language().strings();
        let icon = self.tray_icon.lock().unwrap_or_else(|p| p.into_inner());
        let _ = icon.show_balloon(
            strings.notification_activated_title,
            strings.notification_activated_body,
            false,
        );
    }

    fn notify_deactivated(&self) {
        let strings = self.language().strings();
        let icon = self.tray_icon.lock().unwrap_or_else(|p| p.into_inner());
        let _ = icon.show_balloon(
            strings.notification_deactivated_title,
            strings.notification_deactivated_body,
            false,
        );
    }

    pub fn notify_error(&self) {
        let strings = self.language().strings();
        let icon = self.tray_icon.lock().unwrap_or_else(|p| p.into_inner());
        let _ = icon.show_balloon(
            strings.notification_error_title,
            strings.notification_error_body,
            true,
        );
    }

    /// Chamado pelo monitor (seção 10) quando uma proteção não pôde ser
    /// reconfirmada após uma tentativa de reaplicação: desativa com
    /// rollback e cai em `Error`.
    pub fn handle_protection_failure(self: &Arc<Self>) {
        tracing::warn!("monitor could not confirm a protection; deactivating with rollback");
        if let Err(e) = self.deactivate() {
            tracing::error!(error = %e, "failed to deactivate after losing a protection");
        }
        self.notify_error();
    }

    /// Verificação usada pelo monitor. Confirma o plano ativo, todas as
    /// configurações, os handles das Power Requests e a thread dedicada.
    /// Apenas configurações de plano são reaplicadas uma vez; recursos de
    /// kernel inválidos exigem encerrar a sessão inteira.
    pub fn monitor_protections_healthy(&self, scheme: windows::core::GUID) -> bool {
        if self.state() != State::Active {
            return true;
        }
        let session = self.session.lock().unwrap_or_else(|p| p.into_inner());
        let Some(session) = session.as_ref() else {
            return self.state() != State::Active;
        };
        if session.temporary_scheme.guid() != scheme {
            return false;
        }
        let requests_healthy = session
            .system_request
            .as_ref()
            .is_some_and(PowerRequestGuard::is_valid)
            && session
                .execution_request
                .as_ref()
                .is_some_and(PowerRequestGuard::is_valid);
        let thread_healthy = session
            .execution_state_thread
            .as_ref()
            .is_some_and(ExecutionStateThread::is_alive);
        if !requests_healthy || !thread_healthy {
            return false;
        }

        let first_ok = PowerScheme::active().is_ok_and(|active| active.guid() == scheme)
            && settings::read_status(scheme).is_ok_and(|status| status.all_ok());
        if first_ok {
            return true;
        }

        tracing::warn!("monitor detected a diverging power plan; attempting one reapply");
        temporary_reapply_and_confirm(scheme)
    }

    /// Monta o relatório de status completo (seção 11).
    pub fn status_report(&self) -> StatusReport {
        let state = self.state();
        let language = self.language();
        let manual_hold = self.manual_hold.load(Ordering::SeqCst);
        let leases = self.active_lease_count();
        let last_error = self
            .last_error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();

        let session = self.session.lock().unwrap_or_else(|p| p.into_inner());
        let protections = if let Some(session) = session.as_ref() {
            let scheme_status = settings::read_status(session.temporary_scheme.guid()).unwrap_or(
                nukaboost_core::power::SchemeProtectionStatus {
                    lid_action_ac: false,
                    lid_action_dc: false,
                    idle_sleep_ac: false,
                    idle_sleep_dc: false,
                    system_required_ac: false,
                    system_required_dc: false,
                    low_battery_action_ac: false,
                    low_battery_action_dc: false,
                },
            );
            let temporary_active = PowerScheme::active()
                .map(|active| active.guid() == session.temporary_scheme.guid())
                .unwrap_or(false);
            ProtectionsReport::new(
                temporary_active,
                scheme_status,
                session
                    .system_request
                    .as_ref()
                    .is_some_and(PowerRequestGuard::is_valid),
                session
                    .execution_request
                    .as_ref()
                    .is_some_and(PowerRequestGuard::is_valid),
                session
                    .execution_state_thread
                    .as_ref()
                    .map(|t| t.is_alive())
                    .unwrap_or(false),
            )
        } else {
            ProtectionsReport::new(
                false,
                nukaboost_core::power::SchemeProtectionStatus {
                    lid_action_ac: false,
                    lid_action_dc: false,
                    idle_sleep_ac: false,
                    idle_sleep_dc: false,
                    system_required_ac: false,
                    system_required_dc: false,
                    low_battery_action_ac: false,
                    low_battery_action_dc: false,
                },
                false,
                false,
                false,
            )
        };
        drop(session);

        let (power_source, battery_percent, battery_saver) = power_source::read();

        StatusReport {
            app_version: APP_VERSION.to_string(),
            process_running: true,
            state,
            language,
            manual_hold,
            leases,
            power_source,
            battery_percent,
            battery_saver,
            protections,
            last_error,
        }
    }
}

fn temporary_reapply_and_confirm(scheme: windows::core::GUID) -> bool {
    let power_scheme = PowerScheme::from_guid(scheme);
    if power_scheme.activate().is_err() || settings::apply_and_confirm(scheme).is_err() {
        return false;
    }
    PowerScheme::active().is_ok_and(|active| active.guid() == scheme)
        && settings::read_status(scheme).is_ok_and(|status| status.all_ok())
}

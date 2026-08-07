// Subsistema Windows: sem console (seção 5 do plano — `NukaBoost.exe` é o
// binário de bandeja, `nukaboostctl.exe` é o de console).
#![windows_subsystem = "windows"]

//! `NukaBoost.exe` — ponto de entrada.
//!
//! Despacha para um dos três modos de execução:
//!
//! - Sem argumentos, ou `--startup`: modo normal (bandeja). O argumento
//!   `--startup` não muda o comportamento em relação ao lançamento normal
//!   neste crate — todas as regras da seção 14 ("fazer recuperação
//!   primeiro", "iniciar inativo", "nunca carregar estado ativo anterior",
//!   "exibir aviso de segurança se não suprimido") já são universais no
//!   modo normal — mas o argumento é aceito e preservado para que o
//!   atalho de Startup e o Gerenciador de Tarefas identifiquem claramente
//!   a origem do processo.
//! - `--watchdog <pid> <session-id>`: processo watchdog (seção 9).
//! - `--recover-only <session-id>`: recuperação silenciosa após reinício
//!   abrupto do Windows, sem criar ícone (seção 9).

mod ipc_server;
mod monitor;
mod orchestrator;
mod power_source;
mod single_instance;
mod watchdog;

use std::sync::{Arc, Mutex};

use uuid::Uuid;

use nukaboost_core::error::NukaError;
use nukaboost_core::i18n::Language;
use nukaboost_win32::dialogs::{self, AboutInfo};
use nukaboost_win32::tray::{TrayEvent, TrayViewState, TrayWindow};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

use orchestrator::Orchestrator;

fn main() {
    let _log_guard = init_tracing();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let exit_code = match args.first().map(String::as_str) {
        Some("--watchdog") => run_watchdog_mode(&args[1..]),
        Some("--recover-only") => run_recover_only_mode(&args[1..]),
        Some("--startup") | None => run_normal_mode(),
        Some(other) => {
            tracing::error!(arg = other, "unknown command-line argument");
            2
        }
    };

    std::process::exit(exit_code);
}

/// Inicializa logs em arquivo rotacionado diariamente em
/// `%LocalAppData%\NukaBoost\logs\` (seção 17). O guard retornado precisa
/// permanecer vivo por toda a duração do processo — descartá-lo cedo demais
/// interrompe silenciosamente a gravação assíncrona dos logs.
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let dir = nukaboost_core::paths::logs_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    cleanup_old_logs(&dir);

    let appender = tracing_appender::rolling::daily(&dir, "nukaboost.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(filter)
        .try_init();

    Some(guard)
}

/// Mantém no máximo duas semanas de logs e limita o total histórico a
/// 20 MiB. A limpeza é best-effort e nunca impede a recuperação de energia.
fn cleanup_old_logs(dir: &std::path::Path) {
    const MAX_FILES: usize = 14;
    const MAX_BYTES: u64 = 20 * 1024 * 1024;

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            if !name.starts_with("nukaboost.log") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((path, modified, metadata.len()))
        })
        .collect();
    files.sort_by_key(|(_, modified, _)| *modified);
    let mut total: u64 = files.iter().map(|(_, _, size)| size).sum();
    while files.len() >= MAX_FILES || total > MAX_BYTES {
        let (path, _, size) = files.remove(0);
        if std::fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

fn run_watchdog_mode(args: &[String]) -> i32 {
    let (Some(pid_arg), Some(session_arg)) = (args.first(), args.get(1)) else {
        tracing::error!("--watchdog requires <pid> <session-id>");
        return 2;
    };
    let (Ok(pid), Ok(session_id)) = (pid_arg.parse::<u32>(), Uuid::parse_str(session_arg)) else {
        tracing::error!("--watchdog: invalid arguments");
        return 2;
    };

    match watchdog::run(pid, session_id) {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!(error = %e, "watchdog: recovery failed");
            1
        }
    }
}

fn run_recover_only_mode(args: &[String]) -> i32 {
    let Some(session_arg) = args.first() else {
        tracing::error!("--recover-only requires <session-id>");
        return 2;
    };
    let Ok(session_id) = Uuid::parse_str(session_arg) else {
        tracing::error!("--recover-only: invalid session-id");
        return 2;
    };

    match nukaboost_core::recovery::perform_recovery(Some(session_id)) {
        Ok(_) => 0,
        Err(e) => {
            tracing::error!(error = %e, "--recover-only: recovery failed");
            1
        }
    }
}

fn run_normal_mode() -> i32 {
    // A exclusão mútua vem antes da recuperação: uma segunda inicialização
    // nunca pode restaurar por engano a sessão saudável da primeira.
    let instance_guard = match single_instance::SingleInstanceGuard::acquire() {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            tracing::info!("another NukaBoost instance is already running; exiting");
            return 0;
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to check for a single instance");
            return 1;
        }
    };

    // Recuperação de uma sessão anterior incompleta, agora sob o mutex de
    // instância. Uma falha é bloqueante: continuar poderia reportar estado
    // inativo enquanto um plano temporário ainda está aplicado.
    if let Err(e) = nukaboost_core::recovery::perform_recovery(None) {
        tracing::error!(error = %e, "failed to recover a pending session on startup");
        return 1;
    }

    let tray_view = Arc::new(Mutex::new(TrayViewState {
        active: false,
        language: Language::default(),
    }));

    let (event_tx, event_rx) = std::sync::mpsc::channel::<TrayEvent>();
    let window = match TrayWindow::create(event_tx, Arc::clone(&tray_view)) {
        Ok(window) => window,
        Err(e) => {
            tracing::error!(error = %e, "failed to create the hidden tray window");
            return 1;
        }
    };

    let orchestrator = match Orchestrator::new(window.hwnd(), Arc::clone(&tray_view)) {
        Ok(orchestrator) => orchestrator,
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize the orchestrator");
            return 1;
        }
    };

    // Seção 3, passos 3–4: sempre inicia `Inactive` (garantido pelo
    // `StateMachine::new()` dentro de `Orchestrator::new`) e mostra o
    // ícone inativo.
    {
        let mut view = tray_view.lock().unwrap_or_else(|p| p.into_inner());
        view.language = orchestrator.language();
    }
    orchestrator.show_initial_icon();

    // Servidor IPC em thread própria (seção 11).
    {
        let orchestrator = Arc::clone(&orchestrator);
        std::thread::Builder::new()
            .name("nukaboost-ipc".into())
            .spawn(move || ipc_server::run(orchestrator))
            .expect("failed to start the IPC server thread");
    }

    // Seção 3, passo 6 / seção 4: aviso de segurança, a menos que já tenha
    // sido confirmado com "não mostrar novamente".
    if !orchestrator.is_safety_ack_confirmed() {
        match dialogs::show_safety_warning(window.hwnd(), orchestrator.language()) {
            Ok(result) => {
                if let Err(e) =
                    orchestrator.confirm_safety_ack(result.acknowledged, result.dont_show_again)
                {
                    tracing::error!(error = %e, "failed to persist the safety acknowledgement");
                }
            }
            Err(e) => tracing::error!(error = %e, "failed to show the safety warning"),
        }
    }

    // Eventos da bandeja (cliques, menu) em thread própria — o laço de
    // mensagens da janela precisa rodar exclusivamente na thread principal.
    {
        let orchestrator = Arc::clone(&orchestrator);
        std::thread::Builder::new()
            .name("nukaboost-tray-events".into())
            .spawn(move || {
                let hwnd = orchestrator.hwnd();
                for event in event_rx {
                    handle_tray_event(event, &orchestrator, hwnd);
                }
            })
            .expect("failed to start the tray events thread");
    }

    TrayWindow::run_message_loop();

    // Ao sair do laço de mensagens (por exemplo, `WM_QUERYENDSESSION` ou
    // `Exit`/`Fechar`), garante que tudo esteja desativado e restaurado
    // antes de o processo encerrar de fato.
    if let Err(e) = orchestrator.shutdown() {
        tracing::error!(error = %e, "failed to restore settings during shutdown");
        drop(instance_guard);
        return 1;
    }

    drop(instance_guard);
    0
}

fn handle_tray_event(event: TrayEvent, orchestrator: &Arc<Orchestrator>, hwnd: HWND) {
    match event {
        TrayEvent::ToggleRequested => {
            if let Err(NukaError::SafetyAckRequired) = orchestrator.toggle() {
                // O aviso ainda não foi confirmado: exibe agora e, se
                // confirmado, tenta ativar de novo.
                if let Ok(result) = dialogs::show_safety_warning(hwnd, orchestrator.language()) {
                    if let Err(e) =
                        orchestrator.confirm_safety_ack(result.acknowledged, result.dont_show_again)
                    {
                        tracing::error!(error = %e, "failed to save safety acknowledgement");
                    }
                    if result.acknowledged {
                        let _ = orchestrator.toggle();
                    }
                }
            }
        }
        TrayEvent::LanguageSelected(language) => {
            if let Err(e) = orchestrator.set_language(language) {
                tracing::error!(error = %e, "failed to persist language selection");
            }
        }
        TrayEvent::AboutRequested => {
            let info = AboutInfo {
                app_version: orchestrator::APP_VERSION,
                state: orchestrator.state(),
                language: orchestrator.language(),
            };
            let _ = dialogs::show_about(hwnd, &info);
        }
        TrayEvent::ExitRequested => {
            if let Err(e) = orchestrator.shutdown() {
                tracing::error!(error = %e, "failed to restore settings on exit");
                dialogs::show_error(
                    hwnd,
                    "NukaBoost",
                    &format!("Could not safely restore the original power settings.\n\n{e}"),
                );
                return;
            }
            // SAFETY: `hwnd` é a janela oculta do próprio processo, válida
            // durante toda a sua vida.
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        TrayEvent::TaskbarRecreated => orchestrator.show_initial_icon(),
        TrayEvent::SessionEnding { completion } => {
            let result = orchestrator.shutdown();
            if let Err(e) = &result {
                tracing::error!(error = %e, "failed to restore settings on session end");
            }
            let _ = completion.send(result.is_ok());
        }
    }
}

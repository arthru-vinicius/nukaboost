//! `nukaboostctl.exe` — CLI de controle e leases do NukaBoost (seções 11 e
//! 12 do plano). Subsistema console (padrão): roda bem em terminal e é a
//! interface usada por agentes automatizados.

mod cli;
mod client;
mod duration;
mod launch;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;

use nukaboost_core::error::NukaError;
use nukaboost_core::ipc::protocol::{
    Command, Outcome, PowerSource, ProtectionsReport, StatusReport,
};
use nukaboost_core::startup;

use cli::{Cli, StartupAction};

fn main() {
    let cli = Cli::parse();

    let exit_code = match run(cli.command) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("nukaboostctl: {e}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(command: cli::Command) -> Result<i32, String> {
    let result = match command {
        cli::Command::Status { json } => cmd_status(json),
        cli::Command::Start => cmd_simple(launch::ensure_running_and_send(Command::Start)),
        cli::Command::Stop { force } => cmd_simple(client::send(Command::Stop { force })),
        cli::Command::Toggle => cmd_simple(launch::ensure_running_and_send(Command::Toggle)),
        cli::Command::Language { language } => cmd_simple(client::send(Command::SetLanguage {
            language: language.into(),
        })),
        cli::Command::Startup { action } => cmd_startup(action),
        cli::Command::Exit => cmd_exit(),
        cli::Command::Acquire {
            reason,
            owner,
            ttl,
            json,
        } => cmd_acquire(reason, owner, ttl, json),
        cli::Command::Release { lease_id } => {
            cmd_simple(client::send(Command::ReleaseLease { lease_id }))
        }
        cli::Command::Run { command } => return cmd_run(command),
    };
    result.map(|()| 0)
}

/// Trata o caso comum de comandos que só precisam confirmar sucesso/erro
/// (`Outcome::Ack` ou `Outcome::Error`).
fn cmd_simple(result: Result<Outcome, NukaError>) -> Result<(), String> {
    match result.map_err(|e| e.to_string())? {
        Outcome::Ack | Outcome::LeaseReleased => Ok(()),
        Outcome::Error { code, message } => Err(format!("{message} ({code:?})")),
        other => Err(format!("unexpected response from NukaBoost: {other:?}")),
    }
}

/// `exit` é idempotente por design: se o NukaBoost já não estiver em
/// execução, não há nada a encerrar — isso conta como sucesso, não como
/// erro. Essa distinção importa para o desinstalador MSI, que roda
/// `nukaboostctl exit` como uma ação de limpeza e trata qualquer código de
/// saída diferente de zero como falha real de restauração (seção 15:
/// "Preferencialmente abortar a remoção antes de deixar o sistema em
/// estado desconhecido").
fn cmd_exit() -> Result<(), String> {
    match client::send(Command::Exit) {
        Err(NukaError::ProcessNotRunning) => {
            let pending =
                nukaboost_core::recovery::RecoveryJournal::load().map_err(|e| e.to_string())?;
            if pending
                .as_ref()
                .is_some_and(|journal| journal.main_process_still_running())
            {
                return Err("a live NukaBoost session owns the recovery journal, but its IPC endpoint is unavailable".into());
            }
            nukaboost_core::recovery::perform_recovery(None)
                .map(|_| ())
                .map_err(|e| format!("pending recovery failed: {e}"))
        }
        other => cmd_simple(other),
    }
}

fn cmd_status(json: bool) -> Result<(), String> {
    let report = match client::send(Command::Status) {
        Ok(Outcome::Status(report)) => *report,
        Ok(Outcome::Error { code, message }) => return Err(format!("{message} ({code:?})")),
        Ok(other) => return Err(format!("unexpected response from NukaBoost: {other:?}")),
        Err(NukaError::ProcessNotRunning) => not_running_report(),
        Err(e) => return Err(e.to_string()),
    };

    if json {
        let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        println!("{text}");
    } else {
        print_status_human(&report);
    }
    Ok(())
}

/// Relatório sintético usado quando o processo principal não está em
/// execução — `status --json` continua retornando um objeto válido, com
/// `process_running: false`, em vez de um erro.
fn not_running_report() -> StatusReport {
    StatusReport {
        protocol_version: nukaboost_core::ipc::PROTOCOL_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        process_running: false,
        state: nukaboost_core::state::State::Inactive,
        language: nukaboost_core::i18n::Language::default(),
        manual_hold: false,
        leases: 0,
        power_source: PowerSource::Ac,
        battery_percent: None,
        battery_saver: false,
        protections: ProtectionsReport {
            temporary_scheme_active: false,
            lid_action_ac: false,
            lid_action_dc: false,
            idle_sleep_ac: false,
            idle_sleep_dc: false,
            system_required_ac: false,
            system_required_dc: false,
            low_battery_action: false,
            system_power_request: false,
            execution_power_request: false,
            thread_execution_state: false,
            display_request_absent: true,
        },
        last_error: None,
    }
}

fn print_status_human(report: &StatusReport) {
    if !report.process_running {
        println!("NukaBoost: not running");
        return;
    }

    println!("NukaBoost: {}", report.state);
    println!("Language: {}", report.language);
    println!("Manual hold: {}", report.manual_hold);
    println!("Active leases: {}", report.leases);
    let source = match report.power_source {
        PowerSource::Ac => "AC",
        PowerSource::Battery => "battery",
    };
    match report.battery_percent {
        Some(pct) => println!(
            "Power source: {source} ({pct}%, battery saver: {})",
            report.battery_saver
        ),
        None => println!("Power source: {source}"),
    }
    println!(
        "Protections: {}",
        if report.protections.all_ok() {
            "OK"
        } else {
            "INCOMPLETE"
        }
    );
    if let Some(error) = &report.last_error {
        println!("Last error: {error}");
    }
}

fn cmd_startup(action: StartupAction) -> Result<(), String> {
    match action {
        StartupAction::Enable => startup::enable().map_err(|e| e.to_string()),
        StartupAction::Disable => startup::disable().map_err(|e| e.to_string()),
        StartupAction::Status => {
            let enabled = startup::is_enabled().map_err(|e| e.to_string())?;
            println!("Startup: {}", if enabled { "enabled" } else { "disabled" });
            Ok(())
        }
    }
}

fn cmd_acquire(reason: String, owner: String, ttl: String, json: bool) -> Result<(), String> {
    let ttl = duration::parse_ttl(&ttl)?;
    let command = Command::AcquireLease {
        reason,
        owner,
        ttl_seconds: ttl.as_secs(),
    };

    match launch::ensure_running_and_send(command).map_err(|e| e.to_string())? {
        Outcome::LeaseAcquired { lease_id } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "lease_id": lease_id.to_string() })
                );
            } else {
                println!("{lease_id}");
            }
            Ok(())
        }
        Outcome::Error { code, message } => Err(format!("{message} ({code:?})")),
        other => Err(format!("unexpected response from NukaBoost: {other:?}")),
    }
}

/// `nukaboostctl run -- <comando>`: adquire uma lease, executa o processo
/// filho, e a libera ao final — inclusive em `Ctrl+C` (seção 12).
fn cmd_run(command_and_args: Vec<String>) -> Result<i32, String> {
    let Some((program, args)) = command_and_args.split_first() else {
        return Err("no command given after '--'".to_string());
    };

    let lease_id = match launch::ensure_running_and_send(Command::AcquireLease {
        reason: command_and_args.join(" "),
        owner: "nukaboostctl run".to_string(),
        ttl_seconds: 4 * 3600, // Teto de segurança de 4h; liberada explicitamente ao final de qualquer forma.
    })
    .map_err(|e| e.to_string())?
    {
        Outcome::LeaseAcquired { lease_id } => lease_id,
        Outcome::Error { code, message } => return Err(format!("{message} ({code:?})")),
        other => return Err(format!("unexpected response from NukaBoost: {other:?}")),
    };

    // A aquisição só é considerada concluída quando o estado e todas as
    // proteções forem confirmados pelo servidor.
    let protection_check = match client::send(Command::Status) {
        Ok(Outcome::Status(report))
            if report.state == nukaboost_core::state::State::Active
                && report.protections.all_ok() =>
        {
            Ok(())
        }
        Ok(Outcome::Status(report)) => Err(format!(
            "NukaBoost did not confirm an active/protected state (state: {})",
            report.state
        )),
        Ok(other) => Err(format!("unexpected status response: {other:?}")),
        Err(error) => Err(format!("could not confirm NukaBoost protections: {error}")),
    };
    if let Err(error) = protection_check {
        let _ = client::send(Command::ReleaseLease { lease_id });
        return Err(error);
    }

    // A partir daqui, a lease precisa ser liberada em qualquer saída —
    // sucesso, erro do processo filho, ou este próprio processo sendo
    // interrompido (Ctrl+C também termina o filho, o que faz `wait()`
    // retornar normalmente).
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&interrupted);
    if let Err(error) = ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst)) {
        let _ = client::send(Command::ReleaseLease { lease_id });
        return Err(format!("failed to install Ctrl+C handler: {error}"));
    }

    let run_result = (|| -> Result<std::process::ExitStatus, String> {
        let mut child = std::process::Command::new(program)
            .args(args)
            .spawn()
            .map_err(|e| format!("failed to run '{program}': {e}"))?;
        loop {
            if interrupted.load(Ordering::SeqCst) {
                let _ = child.kill();
                return child
                    .wait()
                    .map_err(|e| format!("failed to wait for '{program}': {e}"));
            }
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(format!("failed to wait for '{program}': {e}")),
            }
        }
    })();

    match client::send(Command::ReleaseLease { lease_id }) {
        Ok(Outcome::LeaseReleased) => {}
        Ok(Outcome::Error { code, message }) => {
            eprintln!(
                "nukaboostctl: warning: failed to release lease {lease_id}: {message} ({code:?})"
            );
        }
        Ok(other) => {
            eprintln!(
                "nukaboostctl: warning: unexpected release response for {lease_id}: {other:?}"
            );
        }
        Err(error) => {
            eprintln!("nukaboostctl: warning: failed to release lease {lease_id}: {error}");
        }
    }

    match run_result {
        Ok(status) => Ok(status.code().unwrap_or(1)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_release_is_a_successful_simple_outcome() {
        assert!(cmd_simple(Ok(Outcome::LeaseReleased)).is_ok());
    }
}

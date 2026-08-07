//! Servidor Named Pipe (seção 11 do plano): aceita uma conexão por vez de
//! `nukaboostctl` (ou de qualquer outro cliente autorizado) e processa
//! exatamente um comando por conexão — suficiente para o uso do CLI, que
//! abre uma conexão nova a cada invocação.

use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::sync::Arc;
use std::time::Duration;

use windows::core::HSTRING;
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, LPARAM, WPARAM};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

use nukaboost_core::error::NukaError;
use nukaboost_core::ipc::protocol::{Command, ErrorCode, Outcome, MAX_MESSAGE_SIZE};
use nukaboost_core::ipc::{self, Envelope, PipeSecurityDescriptor};

use crate::orchestrator::Orchestrator;

/// Roda o laço do servidor IPC nesta thread indefinidamente. Cada iteração
/// aceita uma conexão, processa um único comando, e fecha.
pub fn run(orchestrator: Arc<Orchestrator>) {
    loop {
        match accept_one() {
            Ok(file) => handle_connection(file, &orchestrator),
            Err(e) => {
                tracing::error!(error = %e, "failed to accept IPC connection; retrying shortly");
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn accept_one() -> windows::core::Result<File> {
    let pipe_path = ipc::pipe_path().map_err(|e| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
    })?;
    let security = PipeSecurityDescriptor::build().map_err(|e| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
    })?;
    let attrs = security.as_security_attributes();
    let pipe_path_wide = HSTRING::from(pipe_path.as_str());

    // SAFETY: `pipe_path_wide` e `attrs` vivem até o fim desta chamada
    // síncrona; um único buffer de mensagem (`MAX_MESSAGE_SIZE`) é usado
    // tanto para entrada quanto para saída.
    let handle = unsafe {
        CreateNamedPipeW(
            &pipe_path_wide,
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            MAX_MESSAGE_SIZE as u32,
            MAX_MESSAGE_SIZE as u32,
            0,
            Some(&attrs as *const _),
        )
    };
    if handle.is_invalid() {
        return Err(windows::core::Error::from_thread());
    }

    // SAFETY: `handle` acabou de ser criado; `None` significa E/S síncrona
    // (sem `OVERLAPPED`), que é como este handle foi aberto.
    if let Err(e) = unsafe { ConnectNamedPipe(handle, None) } {
        // `ERROR_PIPE_CONNECTED`: um cliente já se conectou entre a criação
        // do pipe e esta chamada — não é uma falha real.
        let already_connected =
            e.code() == windows::core::HRESULT::from_win32(ERROR_PIPE_CONNECTED.0);
        if !already_connected {
            // SAFETY: `handle` é válido e ainda não foi transferido a
            // ninguém.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(e);
        }
    }

    // SAFETY: `handle` está conectado e sua posse passa integralmente para
    // o `File`, que o fechará ao ser descartado.
    Ok(unsafe { File::from_raw_handle(handle.0 as _) })
}

fn handle_connection(mut file: File, orchestrator: &Arc<Orchestrator>) {
    let request: Envelope<Command> = match ipc::read_message(&mut file) {
        Ok(request) => request,
        Err(_) => return, // Cliente desconectou sem enviar uma mensagem válida.
    };

    if let Err(mismatch) = ipc::check_protocol_version(request.protocol_version) {
        let response = Envelope::reply_to(
            &request,
            Outcome::Error {
                code: ErrorCode::Internal,
                message: mismatch.to_string(),
            },
        );
        let _ = ipc::write_message(&mut file, &response);
        return;
    }

    let exit_requested = matches!(request.body, Command::Exit);
    let outcome = dispatch(orchestrator, request.body.clone());
    let exit_succeeded = exit_requested && matches!(outcome, Outcome::Ack);
    let response = Envelope::reply_to(&request, outcome);
    let reply_sent = ipc::write_message(&mut file, &response).is_ok();
    drop(file);

    // Só fecha depois de enviar e fechar a resposta. Isso evita que o CLI
    // (e a custom action do MSI) observe um pipe quebrado embora a
    // restauração já tenha sido concluída com sucesso.
    if exit_succeeded && reply_sent {
        // SAFETY: a janela existe enquanto o `Orchestrator` está vivo.
        unsafe {
            let _ = PostMessageW(Some(orchestrator.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

fn dispatch(orchestrator: &Arc<Orchestrator>, command: Command) -> Outcome {
    let result = match command {
        Command::Status => return Outcome::Status(Box::new(orchestrator.status_report())),
        Command::Start => orchestrator.manual_start(),
        Command::Stop { force } => orchestrator.manual_stop(force),
        Command::Toggle => orchestrator.toggle(),
        Command::SetLanguage { language } => orchestrator.set_language(language),
        Command::Exit => orchestrator.shutdown(),
        Command::AcquireLease {
            reason,
            owner,
            ttl_seconds,
        } => {
            return match orchestrator.acquire_lease(reason, owner, Duration::from_secs(ttl_seconds))
            {
                Ok(lease_id) => Outcome::LeaseAcquired { lease_id },
                Err(e) => error_outcome(&e),
            };
        }
        Command::ReleaseLease { lease_id } => {
            return match orchestrator.release_lease(lease_id) {
                Ok(()) => Outcome::LeaseReleased,
                Err(e) => error_outcome(&e),
            };
        }
    };

    match result {
        Ok(()) => Outcome::Ack,
        Err(e) => error_outcome(&e),
    }
}

fn error_outcome(error: &NukaError) -> Outcome {
    Outcome::Error {
        code: ErrorCode::from(error),
        message: error.to_string(),
    }
}

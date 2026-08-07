//! Camada de IPC do NukaBoost (seção 11 do plano): nome do pipe local por
//! usuário, descritor de segurança restrito e o protocolo de mensagens
//! trocado com `nukaboostctl` e outros clientes (por exemplo, a skill de
//! agentes).
//!
//! O transporte físico (`CreateNamedPipeW` do lado do servidor,
//! `std::fs::File` aberto sobre `\\.\pipe\...` do lado do cliente) vive em
//! `apps/nukaboost` e `apps/nukaboostctl`, que dependem apenas das funções
//! de enquadramento ([`write_message`]/[`read_message`]) e dos tipos deste
//! módulo — nenhum dos dois binários precisa conhecer o formato de baixo
//! nível das mensagens.

pub mod pipe_name;
pub mod protocol;
pub mod security;

pub use pipe_name::pipe_path;
pub use protocol::{
    Command, Envelope, ErrorCode, Outcome, PowerSource, ProtectionsReport, StatusReport,
    MAX_MESSAGE_SIZE, PROTOCOL_VERSION,
};
pub use security::PipeSecurityDescriptor;

use std::io::{Read, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{NukaError, NukaResult};

/// Escreve uma mensagem JSON delimitada por tamanho (4 bytes little-endian
/// com o comprimento do payload, seguidos do próprio payload) em `writer`.
///
/// Usado tanto pelo cliente (enviar um [`Envelope<Command>`]) quanto pelo
/// servidor (enviar um [`Envelope<Outcome>`]).
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> NukaResult<()> {
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_MESSAGE_SIZE {
        return Err(NukaError::MessageTooLarge {
            limit: MAX_MESSAGE_SIZE,
        });
    }

    let len = payload.len() as u32;
    writer
        .write_all(&len.to_le_bytes())
        .map_err(|e| NukaError::io("<pipe>", e))?;
    writer
        .write_all(&payload)
        .map_err(|e| NukaError::io("<pipe>", e))?;
    writer.flush().map_err(|e| NukaError::io("<pipe>", e))?;
    Ok(())
}

/// Lê uma mensagem JSON delimitada por tamanho de `reader`.
///
/// Rejeita qualquer prefixo de tamanho maior que [`MAX_MESSAGE_SIZE`]
/// **antes** de alocar o buffer correspondente, para que um cliente mal
/// comportado anunciando um tamanho arbitrariamente grande não force uma
/// alocação de memória arbitrária no servidor.
pub fn read_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> NukaResult<T> {
    let mut len_bytes = [0u8; 4];
    reader
        .read_exact(&mut len_bytes)
        .map_err(|e| NukaError::io("<pipe>", e))?;
    let len = u32::from_le_bytes(len_bytes) as usize;

    if len > MAX_MESSAGE_SIZE {
        return Err(NukaError::MessageTooLarge {
            limit: MAX_MESSAGE_SIZE,
        });
    }

    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|e| NukaError::io("<pipe>", e))?;

    Ok(serde_json::from_slice(&payload)?)
}

/// Confirma que a versão de protocolo anunciada pelo cliente é a mesma do
/// servidor, retornando [`NukaError::ProtocolMismatch`] caso contrário.
pub fn check_protocol_version(received: u32) -> NukaResult<()> {
    if received == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(NukaError::ProtocolMismatch {
            client: received,
            server: PROTOCOL_VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_then_read_round_trips_a_command() {
        let request = Envelope::new(Command::Status);

        let mut buffer = Vec::new();
        write_message(&mut buffer, &request).unwrap();

        let mut cursor = Cursor::new(buffer);
        let decoded: Envelope<Command> = read_message(&mut cursor).unwrap();

        assert_eq!(decoded.request_id, request.request_id);
        assert!(matches!(decoded.body, Command::Status));
    }

    #[test]
    fn read_rejects_oversized_length_prefix() {
        let mut buffer = Vec::new();
        let huge_len = (MAX_MESSAGE_SIZE as u32) + 1;
        buffer.extend_from_slice(&huge_len.to_le_bytes());

        let mut cursor = Cursor::new(buffer);
        let result: NukaResult<Envelope<Command>> = read_message(&mut cursor);

        assert!(matches!(result, Err(NukaError::MessageTooLarge { .. })));
    }

    #[test]
    fn protocol_version_check_matches_current_version() {
        assert!(check_protocol_version(PROTOCOL_VERSION).is_ok());
        assert!(check_protocol_version(PROTOCOL_VERSION + 1).is_err());
    }
}

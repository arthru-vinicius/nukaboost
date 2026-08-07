//! Tipo de erro central do NukaBoost.
//!
//! Todas as operações falíveis do núcleo (`power`, `state`, `recovery`,
//! `ipc`, `config`) retornam [`NukaError`] através do alias [`NukaResult`].
//! Concentrar os erros em um único enum facilita o mapeamento para códigos
//! de saída do CLI e para o campo `last_error` do JSON de status.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Alias de conveniência para `Result<T, NukaError>`.
pub type NukaResult<T> = Result<T, NukaError>;

/// Erros que podem ocorrer em qualquer camada do núcleo do NukaBoost.
///
/// A variante escolhida deve refletir a causa raiz, não o subsistema que a
/// detectou, para que `nukaboostctl` e a skill de agentes consigam decidir
/// automaticamente se vale a pena tentar novamente.
#[derive(Debug, Error)]
pub enum NukaError {
    /// Uma chamada de API do Win32 retornou um código de erro.
    #[error("Win32 call '{function}' failed with code {code:#x}: {message}")]
    Win32 {
        /// Nome da função Win32 que falhou, para fins de diagnóstico em log.
        function: &'static str,
        /// Código de erro bruto retornado pelo Windows.
        code: u32,
        /// Mensagem legível associada ao código de erro.
        message: String,
    },

    /// Uma configuração de energia foi aplicada, mas a releitura posterior
    /// não confirmou o valor esperado — viola a garantia de honestidade do
    /// estado descrita no plano do produto.
    #[error("protection '{setting}' could not be confirmed by readback: expected {expected}, got {actual}")]
    ProtectionNotConfirmed {
        /// Nome lógico da proteção (ex.: "LIDACTION/AC").
        setting: &'static str,
        /// Valor que deveria estar em vigor.
        expected: u32,
        /// Valor efetivamente lido do sistema.
        actual: u32,
    },

    /// A transição de estado solicitada não é permitida a partir do estado
    /// atual (por exemplo, tentar desativar algo que já está inativo).
    #[error("invalid state transition: {from} → {to}")]
    InvalidTransition {
        /// Estado de origem no momento da tentativa.
        from: String,
        /// Estado de destino solicitado.
        to: String,
    },

    /// O aviso de segurança inicial ainda não foi confirmado pelo usuário,
    /// portanto a ativação não pode prosseguir.
    #[error("the safety warning has not been acknowledged by the user yet")]
    SafetyAckRequired,

    /// Uma política administrativa (Group Policy/MDM) impede a alteração
    /// necessária do plano de energia.
    #[error("an administrative policy prevents changing the power plan")]
    PolicyDenied,

    /// A lease informada não existe ou já expirou/foi liberada.
    #[error("lease '{0}' not found")]
    LeaseNotFound(String),

    /// Falha de I/O genérica (arquivos de configuração, journal, logs).
    #[error("I/O failure on '{path}': {source}")]
    Io {
        /// Caminho do arquivo envolvido na operação.
        path: PathBuf,
        /// Erro de E/S original.
        #[source]
        source: io::Error,
    },

    /// Falha ao serializar ou desserializar uma estrutura JSON (config,
    /// journal ou mensagem IPC).
    #[error("failed to (de)serialize JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    /// O processo principal não está em execução, então o pipe nomeado não
    /// pôde ser alcançado.
    #[error("the NukaBoost main process is not running")]
    ProcessNotRunning,

    /// Uma mensagem recebida pelo IPC excedeu o tamanho máximo permitido.
    #[error("IPC message exceeds the maximum allowed size ({limit} bytes)")]
    MessageTooLarge {
        /// Limite configurado, em bytes.
        limit: usize,
    },

    /// A versão do protocolo IPC usada pelo cliente é incompatível com a do
    /// servidor.
    #[error("incompatible IPC protocol version: client={client}, server={server}")]
    ProtocolMismatch {
        /// Versão anunciada pelo cliente.
        client: u32,
        /// Versão suportada pelo servidor.
        server: u32,
    },

    /// Erro de propósito geral com uma mensagem descritiva, usado apenas
    /// quando nenhuma variante mais específica se aplica.
    #[error("{0}")]
    Other(String),
}

impl NukaError {
    /// Constrói um [`NukaError::Win32`] a partir de um [`windows::core::Error`],
    /// anotando qual função Win32 originou a falha.
    pub fn from_win32(function: &'static str, error: &windows::core::Error) -> Self {
        Self::Win32 {
            function,
            code: error.code().0 as u32,
            message: error.message().to_string(),
        }
    }

    /// Constrói um [`NukaError::Win32`] a partir de um código `WIN32_ERROR`
    /// bruto, como os retornados diretamente por funções de `powrprof.dll`
    /// (`PowerGetActiveScheme`, `PowerDuplicateScheme`, etc.), que não usam
    /// o mecanismo de `GetLastError`.
    pub fn from_win32_code(
        function: &'static str,
        code: windows::Win32::Foundation::WIN32_ERROR,
    ) -> Self {
        let hresult = windows::core::HRESULT::from_win32(code.0);
        let message = windows::core::Error::from_hresult(hresult).message();
        Self::Win32 {
            function,
            code: code.0,
            message,
        }
    }

    /// Envolve um [`io::Error`] junto do caminho do arquivo que o causou.
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

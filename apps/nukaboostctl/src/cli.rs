//! Definição da interface de linha de comando (seções 11 e 12 do plano).

use clap::{Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use nukaboost_core::i18n::Language;

/// `nukaboostctl` — controle e leases do NukaBoost a partir do terminal ou
/// de agentes automatizados.
#[derive(Debug, Parser)]
#[command(name = "nukaboostctl", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Exibe o estado atual do NukaBoost.
    Status {
        /// Emite o status como JSON de máquina (nunca localizado).
        #[arg(long)]
        json: bool,
    },
    /// Ativa o NukaBoost (equivalente ao clique esquerdo no ícone).
    Start,
    /// Desativa o NukaBoost.
    Stop {
        /// Cancela todas as leases existentes, mesmo as de outros donos.
        /// Reservado para uso humano em emergência — nunca use em uma
        /// skill de agente (seção 12).
        #[arg(long)]
        force: bool,
    },
    /// Alterna entre ativo e inativo.
    Toggle,
    /// Troca o idioma da interface.
    Language {
        #[arg(value_enum)]
        language: LanguageArg,
    },
    /// Gerencia a inicialização automática com o Windows (seção 14).
    Startup {
        #[command(subcommand)]
        action: StartupAction,
    },
    /// Desativa, restaura tudo e encerra o processo principal.
    Exit,
    /// Concede uma lease com TTL, ativando o NukaBoost se necessário.
    Acquire {
        /// Motivo legível, por exemplo `"cargo build --release"`.
        #[arg(long)]
        reason: String,
        /// Identificador do dono da lease, por exemplo `"codex"`.
        #[arg(long)]
        owner: String,
        /// Tempo de vida da lease, por exemplo `2h`, `30m`, `90s`.
        #[arg(long)]
        ttl: String,
        /// Emite o resultado como JSON de máquina.
        #[arg(long)]
        json: bool,
    },
    /// Libera uma lease previamente concedida.
    Release {
        /// ID da lease retornado por `acquire`.
        lease_id: Uuid,
    },
    /// Adquire uma lease, executa o comando informado e a libera ao final
    /// — mesmo em caso de erro ou `Ctrl+C` (seção 12).
    Run {
        /// Comando a executar, precedido por `--`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StartupAction {
    /// Cria o atalho em `FOLDERID_Startup`.
    Enable,
    /// Remove o atalho de inicialização.
    Disable,
    /// Informa se o atalho de inicialização existe.
    Status,
}

/// Espelha [`Language`] apenas para o parsing do CLI — `nukaboost-core`
/// não depende de `clap`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LanguageArg {
    En,
    Pt,
}

impl From<LanguageArg> for Language {
    fn from(value: LanguageArg) -> Self {
        match value {
            LanguageArg::En => Language::En,
            LanguageArg::Pt => Language::Pt,
        }
    }
}

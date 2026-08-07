//! Definição da interface de linha de comando (seções 11 e 12 do plano).

use clap::{Command as ClapCommand, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use nukaboost_core::i18n::Language;

/// Controle e leases do NukaBoost a partir do terminal ou de agentes
/// automatizados.
#[derive(Debug, Parser)]
#[command(name = "nukaboostctl", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Analisa os argumentos usando o idioma persistido antes que o `clap`
/// renderize help, erros de uso ou a lista de subcomandos.
pub fn parse(language: Language) -> Cli {
    let matches = command_for(language).get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}

fn command_for(language: Language) -> ClapCommand {
    let text = HelpText::for_language(language);

    Cli::command()
        .about(text.root)
        .mut_subcommand("status", |command| {
            command
                .about(text.status)
                .mut_arg("json", |argument| argument.help(text.status_json))
        })
        .mut_subcommand("start", |command| command.about(text.start))
        .mut_subcommand("stop", |command| {
            command
                .about(text.stop)
                .mut_arg("force", |argument| argument.help(text.stop_force))
        })
        .mut_subcommand("toggle", |command| command.about(text.toggle))
        .mut_subcommand("language", |command| {
            command
                .about(text.language)
                .mut_arg("language", |argument| argument.help(text.language_value))
        })
        .mut_subcommand("startup", |command| {
            command
                .about(text.startup)
                .mut_subcommand("enable", |nested| nested.about(text.startup_enable))
                .mut_subcommand("disable", |nested| nested.about(text.startup_disable))
                .mut_subcommand("status", |nested| nested.about(text.startup_status))
        })
        .mut_subcommand("exit", |command| command.about(text.exit))
        .mut_subcommand("acquire", |command| {
            command
                .about(text.acquire)
                .mut_arg("reason", |argument| argument.help(text.acquire_reason))
                .mut_arg("owner", |argument| argument.help(text.acquire_owner))
                .mut_arg("ttl", |argument| argument.help(text.acquire_ttl))
                .mut_arg("json", |argument| argument.help(text.acquire_json))
        })
        .mut_subcommand("release", |command| {
            command
                .about(text.release)
                .mut_arg("lease_id", |argument| argument.help(text.release_id))
        })
        .mut_subcommand("run", |command| {
            command
                .about(text.run)
                .mut_arg("command", |argument| argument.help(text.run_command))
        })
}

struct HelpText {
    root: &'static str,
    status: &'static str,
    status_json: &'static str,
    start: &'static str,
    stop: &'static str,
    stop_force: &'static str,
    toggle: &'static str,
    language: &'static str,
    language_value: &'static str,
    startup: &'static str,
    startup_enable: &'static str,
    startup_disable: &'static str,
    startup_status: &'static str,
    exit: &'static str,
    acquire: &'static str,
    acquire_reason: &'static str,
    acquire_owner: &'static str,
    acquire_ttl: &'static str,
    acquire_json: &'static str,
    release: &'static str,
    release_id: &'static str,
    run: &'static str,
    run_command: &'static str,
}

impl HelpText {
    const fn for_language(language: Language) -> &'static Self {
        match language {
            Language::En => &HELP_EN,
            Language::Pt => &HELP_PT,
        }
    }
}

static HELP_EN: HelpText = HelpText {
    root: "Control NukaBoost and manage leases from the terminal or automation agents.",
    status: "Show the current NukaBoost status.",
    status_json: "Emit the status as stable, non-localized JSON.",
    start: "Activate NukaBoost, equivalent to left-clicking the tray icon.",
    stop: "Deactivate NukaBoost.",
    stop_force: "Cancel every active lease, including leases owned by other processes. Reserved for human emergency use.",
    toggle: "Toggle between active and inactive.",
    language: "Change the interface language.",
    language_value: "Interface language to select.",
    startup: "Manage automatic startup with Windows.",
    startup_enable: "Create the Windows startup shortcut.",
    startup_disable: "Remove the Windows startup shortcut.",
    startup_status: "Show whether automatic startup is enabled.",
    exit: "Deactivate NukaBoost, restore all settings, and exit.",
    acquire: "Acquire a lease with a TTL, activating NukaBoost if necessary.",
    acquire_reason: "Human-readable reason, for example: cargo build --release.",
    acquire_owner: "Lease owner identifier, for example: codex.",
    acquire_ttl: "Lease lifetime, for example: 2h, 30m, or 90s.",
    acquire_json: "Emit the result as stable, non-localized JSON.",
    release: "Release a previously acquired lease.",
    release_id: "Lease ID returned by acquire.",
    run: "Acquire a lease, run a command, and release the lease when it finishes, fails, or is interrupted.",
    run_command: "Command to run, preceded by --.",
};

static HELP_PT: HelpText = HelpText {
    root: "Controle o NukaBoost e gerencie leases pelo terminal ou por agentes de automação.",
    status: "Exibe o estado atual do NukaBoost.",
    status_json: "Emite o estado como JSON estável e não localizado.",
    start: "Ativa o NukaBoost, equivalente ao clique esquerdo no ícone da área de notificação.",
    stop: "Desativa o NukaBoost.",
    stop_force: "Cancela todas as leases ativas, inclusive as pertencentes a outros processos. Reservado para emergências e uso humano.",
    toggle: "Alterna entre ativo e inativo.",
    language: "Troca o idioma da interface.",
    language_value: "Idioma da interface que será selecionado.",
    startup: "Gerencia a inicialização automática com o Windows.",
    startup_enable: "Cria o atalho de inicialização do Windows.",
    startup_disable: "Remove o atalho de inicialização do Windows.",
    startup_status: "Informa se a inicialização automática está ativada.",
    exit: "Desativa o NukaBoost, restaura todas as configurações e encerra.",
    acquire: "Concede uma lease com TTL, ativando o NukaBoost se necessário.",
    acquire_reason: "Motivo legível, por exemplo: cargo build --release.",
    acquire_owner: "Identificador do dono da lease, por exemplo: codex.",
    acquire_ttl: "Tempo de vida da lease, por exemplo: 2h, 30m ou 90s.",
    acquire_json: "Emite o resultado como JSON estável e não localizado.",
    release: "Libera uma lease concedida anteriormente.",
    release_id: "ID da lease retornado por acquire.",
    run: "Adquire uma lease, executa um comando e libera a lease ao terminar, falhar ou ser interrompido.",
    run_command: "Comando que será executado, precedido por --.",
};

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
        /// Reservado para uso humano em emergência. Nunca use em uma skill
        /// de agente (seção 12).
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
    /// Adquire uma lease, executa o comando informado e a libera ao final,
    /// mesmo em caso de erro ou `Ctrl+C` (seção 12).
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

/// Espelha [`Language`] apenas para o parsing do CLI. `nukaboost-core`
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn rendered_help(language: Language, args: &[&str]) -> String {
        let error = command_for(language)
            .try_get_matches_from(args)
            .expect_err("--help must stop parsing with DisplayHelp");
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        error.to_string()
    }

    #[test]
    fn every_help_screen_uses_the_selected_language() {
        let cases: &[(&[&str], &str, &str)] = &[
            (
                &["nukaboostctl", "--help"],
                "Control NukaBoost and manage leases",
                "Controle o NukaBoost e gerencie leases",
            ),
            (
                &["nukaboostctl", "status", "--help"],
                "stable, non-localized JSON",
                "JSON estável e não localizado",
            ),
            (
                &["nukaboostctl", "start", "--help"],
                "equivalent to left-clicking",
                "equivalente ao clique esquerdo",
            ),
            (
                &["nukaboostctl", "stop", "--help"],
                "Reserved for human emergency use",
                "Reservado para emergências e uso humano",
            ),
            (
                &["nukaboostctl", "toggle", "--help"],
                "Toggle between active and inactive",
                "Alterna entre ativo e inativo",
            ),
            (
                &["nukaboostctl", "language", "--help"],
                "Change the interface language",
                "Troca o idioma da interface",
            ),
            (
                &["nukaboostctl", "startup", "--help"],
                "Manage automatic startup with Windows",
                "Gerencia a inicialização automática com o Windows",
            ),
            (
                &["nukaboostctl", "exit", "--help"],
                "restore all settings, and exit",
                "restaura todas as configurações e encerra",
            ),
            (
                &["nukaboostctl", "acquire", "--help"],
                "Lease owner identifier",
                "Identificador do dono da lease",
            ),
            (
                &["nukaboostctl", "release", "--help"],
                "Lease ID returned by acquire",
                "ID da lease retornado por acquire",
            ),
            (
                &["nukaboostctl", "run", "--help"],
                "release the lease when it finishes",
                "libera a lease ao terminar",
            ),
        ];

        for (args, english, portuguese) in cases {
            let english_help = rendered_help(Language::En, args);
            assert!(english_help.contains(english), "{args:?}: {english_help}");
            assert!(
                !english_help.contains(portuguese),
                "{args:?}: {english_help}"
            );

            let portuguese_help = rendered_help(Language::Pt, args);
            assert!(
                portuguese_help.contains(portuguese),
                "{args:?}: {portuguese_help}"
            );
            assert!(
                !portuguese_help.contains(english),
                "{args:?}: {portuguese_help}"
            );
        }
    }

    #[test]
    fn localized_commands_keep_the_same_parser_shape() {
        command_for(Language::En).debug_assert();
        command_for(Language::Pt).debug_assert();
    }
}

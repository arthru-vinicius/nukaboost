//! Internacionalização (i18n) do NukaBoost.
//!
//! O produto suporta apenas dois idiomas de interface — inglês (padrão) e
//! português — conforme decidido no plano do produto. O texto do aviso de
//! segurança é uma exceção: ele é **sempre** bilíngue, independentemente do
//! idioma escolhido, então vive em constantes separadas ([`SAFETY_WARNING_EN`]
//! e [`SAFETY_WARNING_PT`]) em vez de dentro de [`Strings`].
//!
//! As chaves e valores trocados via IPC (`ipc::protocol`) nunca devem ser
//! localizados; apenas texto exibido diretamente ao usuário (bandeja, menu,
//! diálogos, notificações) passa por este módulo.

mod en;
mod pt;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Idioma de interface selecionado pelo usuário.
///
/// O padrão de fábrica é [`Language::En`], conforme exigido pelo plano do
/// produto ("EN padrão"). A preferência é persistida em
/// [`crate::config::AppConfig`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Inglês — idioma padrão da aplicação.
    #[default]
    En,
    /// Português — idioma alternativo selecionável pelo usuário.
    Pt,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::En => "en",
            Self::Pt => "pt",
        })
    }
}

impl FromStr for Language {
    type Err = UnknownLanguage;

    /// Interpreta os códigos aceitos pelo CLI (`nukaboostctl language en|pt`),
    /// sem diferenciar maiúsculas de minúsculas.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "en" => Ok(Self::En),
            "pt" => Ok(Self::Pt),
            other => Err(UnknownLanguage(other.to_string())),
        }
    }
}

impl Language {
    /// Retorna a tabela de strings localizadas para este idioma.
    pub const fn strings(self) -> &'static Strings {
        match self {
            Self::En => &en::STRINGS,
            Self::Pt => &pt::STRINGS,
        }
    }
}

/// Erro retornado quando um código de idioma desconhecido é fornecido, por
/// exemplo via `nukaboostctl language <valor>`.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown language: '{0}' (use 'en' or 'pt')")]
pub struct UnknownLanguage(pub String);

/// Conjunto completo de textos localizados exibidos pela interface gráfica.
///
/// Cada campo corresponde a um único ponto de exibição na bandeja, no menu
/// de contexto ou nos diálogos nativos. Mantemos os campos como
/// `&'static str` para evitar alocações em tempo de execução — a troca de
/// idioma apenas troca qual tabela estática é referenciada.
pub struct Strings {
    // Tooltips do ícone da bandeja (seção 3 do plano).
    pub tray_tooltip_inactive: &'static str,
    pub tray_tooltip_active: &'static str,
    pub tray_tooltip_error: &'static str,

    // Menu de contexto.
    pub menu_start: &'static str,
    pub menu_stop: &'static str,
    pub menu_language: &'static str,
    pub menu_language_en: &'static str,
    pub menu_language_pt: &'static str,
    pub menu_about: &'static str,
    pub menu_exit: &'static str,

    // Janela About.
    pub about_title: &'static str,
    pub about_version_label: &'static str,
    pub about_state_label: &'static str,
    pub about_description: &'static str,
    pub about_never_keeps_screen_on: &'static str,
    pub about_ventilation_warning: &'static str,
    pub about_termination_warning: &'static str,
    pub about_close_button: &'static str,

    // Aviso de segurança (controles; o corpo do texto é sempre bilíngue).
    pub safety_warning_checkbox: &'static str,
    pub safety_warning_ok_button: &'static str,

    // Notificações balão emitidas ao ativar/desativar.
    pub notification_activated_title: &'static str,
    pub notification_activated_body: &'static str,
    pub notification_deactivated_title: &'static str,
    pub notification_deactivated_body: &'static str,
    pub notification_error_title: &'static str,
    pub notification_error_body: &'static str,

    // Nomes de estado exibidos na UI (não confundir com os valores JSON,
    // que nunca são localizados).
    pub state_inactive: &'static str,
    pub state_activating: &'static str,
    pub state_active: &'static str,
    pub state_deactivating: &'static str,
    pub state_error: &'static str,
}

/// Texto do aviso de segurança exibido na inicialização, em inglês.
///
/// Reproduzido literalmente da seção 4 do plano do produto — não deve ser
/// reescrito, apenas exibido.
pub const SAFETY_WARNING_EN: &str = "NukaBoost can keep your laptop running with the lid closed. \
Never place it inside a bag, sleeve, or poorly ventilated space while active. \
Heavy workloads can cause excessive heat, battery drain, or thermal shutdown.";

/// Texto do aviso de segurança exibido na inicialização, em português.
///
/// Reproduzido literalmente da seção 4 do plano do produto — não deve ser
/// reescrito, apenas exibido.
pub const SAFETY_WARNING_PT: &str = "O NukaBoost pode manter o notebook funcionando com a tampa fechada. \
Nunca coloque o notebook dentro de uma mochila, capa ou espaço sem ventilação enquanto ele estiver ativo. \
Cargas intensas podem causar aquecimento excessivo, consumo da bateria ou desligamento térmico.";

/// Aviso sobre encerramento fora do controle do aplicativo (Gerenciador de
/// Tarefas, `taskkill`, um agente automatizado), em inglês.
///
/// Reproduzido literalmente da subseção "Encerramento fora do controle do
/// aplicativo" (seção 4 do plano do produto) — não deve ser reescrito,
/// apenas exibido. Exibido junto de [`SAFETY_WARNING_EN`] no diálogo
/// inicial; uma versão resumida também aparece no diálogo `About`
/// ([`Strings::about_termination_warning`]).
pub const TERMINATION_WARNING_EN: &str = "Force-closing NukaBoost (Task Manager, taskkill, or an automated agent) \
instead of stopping it normally can leave the lid/sleep override in place. A watchdog process usually restores \
your original settings within seconds; if that also fails, NukaBoost self-heals at your next sign-in. \
Prefer 'nukaboostctl stop' or the tray menu to deactivate.";

/// Aviso sobre encerramento fora do controle do aplicativo, em português.
///
/// Reproduzido literalmente da mesma subseção do plano do produto — não
/// deve ser reescrito, apenas exibido. Ver [`TERMINATION_WARNING_EN`].
pub const TERMINATION_WARNING_PT: &str = "Encerrar o NukaBoost à força (Gerenciador de Tarefas, taskkill, ou um \
agente automatizado) em vez de pará-lo normalmente pode deixar a substituição de tampa/suspensão ativa. Um \
processo watchdog costuma restaurar as configurações originais em segundos; se isso também falhar, o NukaBoost \
se autocorrige no próximo login. Prefira 'nukaboostctl stop' ou o menu da bandeja para desativar.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_language_is_english() {
        assert_eq!(Language::default(), Language::En);
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(Language::from_str("EN").unwrap(), Language::En);
        assert_eq!(Language::from_str("pt").unwrap(), Language::Pt);
        assert_eq!(Language::from_str("Pt").unwrap(), Language::Pt);
    }

    #[test]
    fn rejects_unknown_language() {
        assert!(Language::from_str("fr").is_err());
    }

    #[test]
    fn display_round_trips_through_from_str() {
        for lang in [Language::En, Language::Pt] {
            assert_eq!(Language::from_str(&lang.to_string()).unwrap(), lang);
        }
    }
}

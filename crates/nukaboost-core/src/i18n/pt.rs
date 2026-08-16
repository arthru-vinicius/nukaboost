//! Tabela de strings em português. É o idioma alternativo selecionável pelo usuário.

use super::Strings;

/// Instância estática com todos os textos da interface em português.
pub static STRINGS: Strings = Strings {
    tray_tooltip_inactive: "NukaBoost: Desativado",
    tray_tooltip_active: "NukaBoost: Ativo",
    tray_tooltip_error: "NukaBoost: Erro de proteção",

    menu_start: "Iniciar",
    menu_stop: "Parar",
    menu_language: "Idioma",
    menu_language_en: "English (EN)",
    menu_language_pt: "Português (PT)",
    menu_about: "Sobre",
    menu_exit: "Fechar",

    about_title: "Sobre o NukaBoost",
    about_version_label: "Versão",
    about_state_label: "Estado atual",
    about_description: "O NukaBoost mantém seu PC acordado durante tarefas longas, como \
compilações, downloads e renderizações, mesmo com a tampa fechada.",
    about_never_keeps_screen_on: "O NukaBoost nunca mantém a tela ligada.",
    about_ventilation_warning: "Garanta ventilação adequada enquanto estiver ativo para evitar \
aquecimento excessivo.",
    about_termination_warning:
        "Não encerre o NukaBoost à força (Gerenciador de Tarefas, taskkill, \
um agente automatizado): isso pode deixar as configurações de suspensão desativadas. Use \
'nukaboostctl stop' em vez disso.",
    about_close_button: "Fechar",

    safety_warning_checkbox: "Não mostrar este aviso novamente",
    safety_warning_ok_button: "OK",

    notification_activated_title: "NukaBoost ativado",
    notification_activated_body: "A suspensão fica bloqueada enquanto o NukaBoost estiver ativo. \
A tela continuará apagando normalmente.",
    notification_deactivated_title: "NukaBoost desativado",
    notification_deactivated_body: "As configurações originais de energia foram restauradas.",
    notification_error_title: "Erro de proteção do NukaBoost",
    notification_error_body: "Uma proteção não pôde ser confirmada. O NukaBoost foi desativado \
por segurança.",

    state_inactive: "Desativado",
    state_activating: "Ativando",
    state_active: "Ativo",
    state_deactivating: "Desativando",
    state_error: "Erro de proteção",
};

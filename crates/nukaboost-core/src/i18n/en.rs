//! Tabela de strings em inglês. É o idioma padrão da aplicação.

use super::Strings;

/// Instância estática com todos os textos da interface em inglês.
pub static STRINGS: Strings = Strings {
    tray_tooltip_inactive: "NukaBoost: Inactive",
    tray_tooltip_active: "NukaBoost: Active",
    tray_tooltip_error: "NukaBoost: Protection error",

    menu_start: "Start",
    menu_stop: "Stop",
    menu_language: "Language",
    menu_language_en: "English (EN)",
    menu_language_pt: "Português (PT)",
    menu_about: "About",
    menu_exit: "Exit",

    about_title: "About NukaBoost",
    about_version_label: "Version",
    about_state_label: "Current state",
    about_description: "NukaBoost keeps your PC awake for long-running tasks such as builds, \
downloads, and renders, even with the lid closed.",
    about_never_keeps_screen_on: "NukaBoost never keeps the screen on.",
    about_ventilation_warning: "Ensure adequate ventilation while active to avoid overheating.",
    about_close_button: "Close",

    safety_warning_checkbox: "Do not show this warning again",
    safety_warning_ok_button: "OK",

    notification_activated_title: "NukaBoost activated",
    notification_activated_body: "Sleep is suspended while NukaBoost is active. The screen will \
still turn off normally.",
    notification_deactivated_title: "NukaBoost deactivated",
    notification_deactivated_body: "Original power settings have been restored.",
    notification_error_title: "NukaBoost protection error",
    notification_error_body: "A protection could not be confirmed. NukaBoost has been deactivated \
for safety.",

    state_inactive: "Inactive",
    state_activating: "Activating",
    state_active: "Active",
    state_deactivating: "Deactivating",
    state_error: "Protection error",
};

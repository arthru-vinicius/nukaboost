//! Diálogos nativos do NukaBoost: About (seção 3) e aviso de segurança
//! inicial (seção 4). Implementados como recursos `DIALOGEX` em
//! `resources/nukaboost.rc`, carregados via `DialogBoxParamW` — Win32
//! nativo, sem nenhum framework de UI multiplataforma (seção 3: "evitando
//! frameworks grandes apenas para essa janela").

pub mod about;
pub mod warning;

pub use about::{show_about, AboutInfo};
pub use warning::{show_safety_warning, SafetyWarningResult};

use windows::core::HSTRING;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetDlgItem, MessageBoxW, SetWindowTextW, MB_ICONERROR, MB_OK,
};

/// Define o texto de um controle estático/botão do diálogo `dialog`
/// identificado por `control_id`. Falhas em localizar o controle são
/// silenciosamente ignoradas — o pior caso é um rótulo em branco, nunca um
/// crash do diálogo.
pub(crate) fn set_dlg_text(dialog: HWND, control_id: u16, text: &str) {
    // SAFETY: `dialog` é válido durante o processamento das mensagens do
    // diálogo, de onde esta função é sempre chamada.
    let Ok(control) = (unsafe { GetDlgItem(Some(dialog), control_id as i32) }) else {
        return;
    };
    let value = HSTRING::from(text);
    // SAFETY: `control` é válido; `value` vive até o fim desta chamada síncrona.
    unsafe {
        let _ = SetWindowTextW(control, &value);
    }
}

/// Exibe uma falha que exige intervenção sem depender do ícone da bandeja.
pub fn show_error(owner: HWND, title: &str, message: &str) {
    let title = HSTRING::from(title);
    let message = HSTRING::from(message);
    // SAFETY: strings permanecem válidas durante a chamada modal síncrona.
    unsafe {
        let _ = MessageBoxW(Some(owner), &message, &title, MB_OK | MB_ICONERROR);
    }
}

//! Diálogo "About NukaBoost" (seção 3 do plano): ícone, nome, versão,
//! estado atual, descrição localizada, aviso de que a tela nunca é mantida
//! ligada, aviso de ventilação e dica de como fixar o ícone na bandeja.

use windows::core::HSTRING;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    DialogBoxParamW, EndDialog, SetWindowTextW, IDCANCEL, IDOK, WM_CLOSE, WM_COMMAND, WM_INITDIALOG,
};

use nukaboost_core::i18n::{Language, Strings};
use nukaboost_core::state::State;

use crate::dialogs::set_dlg_text;
use crate::resource_ids::{
    IDC_ABOUT_DESC, IDC_ABOUT_PIN_HINT, IDC_ABOUT_SCREEN_NOTE, IDC_ABOUT_STATE, IDC_ABOUT_TITLE,
    IDC_ABOUT_VENTILATION, IDC_ABOUT_VERSION, IDD_ABOUT,
};
use crate::wide::resource_id;

/// Dados exibidos no diálogo About, fornecidos pela aplicação chamadora.
pub struct AboutInfo<'a> {
    pub app_version: &'a str,
    pub state: State,
    pub language: Language,
}

/// Exibe o diálogo About de forma modal, bloqueando a thread atual até o
/// usuário fechar a janela. `owner` deve ser a janela oculta de mensagens
/// do NukaBoost.
pub fn show_about(owner: HWND, info: &AboutInfo<'_>) -> windows::core::Result<()> {
    // SAFETY: `None` pede o handle do módulo executável atual.
    let instance = unsafe { GetModuleHandleW(None) }?;

    // `info` é passado como `dwInitParam` e recuperado em `WM_INITDIALOG`;
    // `info` vive por toda a duração desta chamada (bloqueante), então o
    // ponteiro permanece válido durante todo o ciclo de vida do diálogo.
    // SAFETY: `IDD_ABOUT` corresponde a um recurso `DIALOGEX` embutido pelo
    // `build.rs`; `about_dlgproc` é uma `DLGPROC` válida.
    let result = unsafe {
        DialogBoxParamW(
            Some(instance.into()),
            resource_id(IDD_ABOUT),
            Some(owner),
            Some(about_dlgproc),
            LPARAM(info as *const AboutInfo<'_> as isize),
        )
    };

    if result <= 0 {
        return Err(windows::core::Error::from_thread());
    }
    Ok(())
}

unsafe extern "system" fn about_dlgproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match msg {
        WM_INITDIALOG => {
            // SAFETY: `lparam` é o `dwInitParam` passado em `show_about`,
            // que aponta para um `AboutInfo` válido durante todo o diálogo.
            let info = unsafe { &*(lparam.0 as *const AboutInfo<'_>) };
            populate(hwnd, info);
            1
        }
        WM_COMMAND => {
            let id = (wparam.0 as u32 & 0xFFFF) as i32;
            if id == IDOK.0 || id == IDCANCEL.0 {
                // SAFETY: `hwnd` é o próprio diálogo, sempre válido aqui.
                let _ = unsafe { EndDialog(hwnd, 1) };
            }
            1
        }
        WM_CLOSE => {
            // SAFETY: mesmo contrato acima.
            let _ = unsafe { EndDialog(hwnd, 0) };
            1
        }
        _ => 0,
    }
}

fn populate(hwnd: HWND, info: &AboutInfo<'_>) {
    let strings = info.language.strings();
    let caption = HSTRING::from(strings.about_title);
    // SAFETY: `hwnd` é o próprio diálogo, sempre válido em `WM_INITDIALOG`;
    // `caption` vive até o fim desta chamada síncrona.
    unsafe {
        let _ = SetWindowTextW(hwnd, &caption);
    }
    set_dlg_text(hwnd, IDC_ABOUT_TITLE, "NukaBoost");
    set_dlg_text(
        hwnd,
        IDC_ABOUT_VERSION,
        &format!("{}: {}", strings.about_version_label, info.app_version),
    );
    set_dlg_text(
        hwnd,
        IDC_ABOUT_STATE,
        &format!(
            "{}: {}",
            strings.about_state_label,
            state_label(info.state, strings)
        ),
    );
    set_dlg_text(hwnd, IDC_ABOUT_DESC, strings.about_description);
    set_dlg_text(
        hwnd,
        IDC_ABOUT_SCREEN_NOTE,
        strings.about_never_keeps_screen_on,
    );
    set_dlg_text(
        hwnd,
        IDC_ABOUT_VENTILATION,
        strings.about_ventilation_warning,
    );
    set_dlg_text(hwnd, IDC_ABOUT_PIN_HINT, strings.about_tray_pin_hint);
    set_dlg_text(hwnd, IDOK.0 as u16, strings.about_close_button);
}

fn state_label(state: State, strings: &Strings) -> &str {
    match state {
        State::Inactive => strings.state_inactive,
        State::Activating => strings.state_activating,
        State::Active => strings.state_active,
        State::Deactivating => strings.state_deactivating,
        State::Error => strings.state_error,
    }
}

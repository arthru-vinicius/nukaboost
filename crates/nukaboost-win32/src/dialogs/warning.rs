//! Diálogo do aviso de segurança inicial (seção 4 do plano).
//!
//! O corpo do texto é sempre bilíngue (inglês e português), independente do
//! idioma escolhido pelo usuário — apenas a caixa de seleção e o botão OK
//! seguem o idioma da interface ("O restante da interface segue o idioma
//! escolhido"). Fechar a janela sem clicar em OK **não** conta como
//! confirmação: a ativação continua bloqueada.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{IsDlgButtonChecked, BST_CHECKED};
use windows::Win32::UI::WindowsAndMessaging::{
    DialogBoxParamW, EndDialog, GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, IDCANCEL,
    IDOK, WM_CLOSE, WM_COMMAND, WM_INITDIALOG,
};

use nukaboost_core::i18n::{
    Language, SAFETY_WARNING_EN, SAFETY_WARNING_PT, TERMINATION_WARNING_EN, TERMINATION_WARNING_PT,
};

use crate::dialogs::set_dlg_text;
use crate::resource_ids::{
    IDC_WARNING_CHECKBOX, IDC_WARNING_TEXT_EN, IDC_WARNING_TEXT_PT, IDD_SAFETY_WARNING,
};
use crate::wide::resource_id;

/// Resultado da interação do usuário com o diálogo do aviso de segurança.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyWarningResult {
    /// Verdadeiro somente se o usuário clicou em OK. Fechar pela borda/X
    /// (ou pressionar Esc) resulta em `false` — a seção 4 do plano exige
    /// que "enquanto o aviso não for confirmado, o estado permanece
    /// inativo", então qualquer saída que não seja OK deve ser tratada como
    /// não confirmada.
    pub acknowledged: bool,
    /// Verdadeiro se "Do not show this warning again" estava marcada
    /// quando o usuário clicou em OK. Ignorado quando `acknowledged` é falso.
    pub dont_show_again: bool,
}

struct DialogState {
    language: Language,
    result: SafetyWarningResult,
}

/// Exibe o aviso de segurança de forma modal, bloqueando a thread atual até
/// o usuário confirmar ou fechar a janela.
pub fn show_safety_warning(
    owner: HWND,
    language: Language,
) -> windows::core::Result<SafetyWarningResult> {
    // SAFETY: `None` pede o handle do módulo executável atual.
    let instance = unsafe { GetModuleHandleW(None) }?;

    let mut state = DialogState {
        language,
        result: SafetyWarningResult {
            acknowledged: false,
            dont_show_again: false,
        },
    };

    // SAFETY: `IDD_SAFETY_WARNING` corresponde a um recurso `DIALOGEX`
    // embutido pelo `build.rs`; `&mut state` vive durante toda a chamada
    // bloqueante, cobrindo o tempo de vida do ponteiro passado ao diálogo.
    let dialog_result = unsafe {
        DialogBoxParamW(
            Some(instance.into()),
            resource_id(IDD_SAFETY_WARNING),
            Some(owner),
            Some(warning_dlgproc),
            LPARAM(&mut state as *mut DialogState as isize),
        )
    };

    if dialog_result < 0 {
        return Err(windows::core::Error::from_thread());
    }
    Ok(state.result)
}

unsafe extern "system" fn warning_dlgproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match msg {
        WM_INITDIALOG => {
            // SAFETY: `SetWindowLongPtrW` apenas armazena o valor.
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, lparam.0) };
            // SAFETY: `lparam` é o `dwInitParam` passado em
            // `show_safety_warning`, válido durante todo o diálogo.
            let state = unsafe { &*(lparam.0 as *const DialogState) };
            populate(hwnd, state.language);
            1
        }
        WM_COMMAND => {
            let id = (wparam.0 as u32 & 0xFFFF) as i32;
            if id == IDOK.0 {
                // SAFETY: armazenado em `WM_INITDIALOG`, que sempre precede
                // `WM_COMMAND` para este diálogo.
                let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DialogState;
                if !ptr.is_null() {
                    // SAFETY: `ptr` aponta para o mesmo `DialogState` válido
                    // durante toda a chamada bloqueante de `show_safety_warning`.
                    let state = unsafe { &mut *ptr };
                    // SAFETY: `IDC_WARNING_CHECKBOX` identifica a caixa de
                    // seleção definida no `.rc`; `hwnd` é o diálogo atual.
                    let checked = unsafe { IsDlgButtonChecked(hwnd, IDC_WARNING_CHECKBOX as i32) };
                    state.result = SafetyWarningResult {
                        acknowledged: true,
                        dont_show_again: checked == BST_CHECKED.0,
                    };
                }
                // SAFETY: `hwnd` é o próprio diálogo, sempre válido aqui.
                let _ = unsafe { EndDialog(hwnd, 1) };
            } else if id == IDCANCEL.0 {
                // SAFETY: mesmo contrato acima.
                let _ = unsafe { EndDialog(hwnd, 0) };
            }
            1
        }
        WM_CLOSE => {
            // SAFETY: mesmo contrato acima; o resultado padrão
            // (`acknowledged: false`) já reflete "não confirmado".
            let _ = unsafe { EndDialog(hwnd, 0) };
            1
        }
        _ => 0,
    }
}

fn populate(hwnd: HWND, language: Language) {
    set_dlg_text(
        hwnd,
        IDC_WARNING_TEXT_EN,
        &format!("{SAFETY_WARNING_EN}\r\n\r\n{TERMINATION_WARNING_EN}"),
    );
    set_dlg_text(
        hwnd,
        IDC_WARNING_TEXT_PT,
        &format!("{SAFETY_WARNING_PT}\r\n\r\n{TERMINATION_WARNING_PT}"),
    );

    let strings = language.strings();
    set_dlg_text(hwnd, IDC_WARNING_CHECKBOX, strings.safety_warning_checkbox);
    set_dlg_text(hwnd, IDOK.0 as u16, strings.safety_warning_ok_button);
}

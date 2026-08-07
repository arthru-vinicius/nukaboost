//! Menu de contexto da bandeja (seção 3 do plano): `Start/Stop`, `Language`
//! (submenu EN/PT), `About`, separador, `Exit`.

use windows::core::HSTRING;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow,
    TrackPopupMenu, HMENU, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
};

use nukaboost_core::i18n::Language;

use crate::resource_ids::{
    ID_TRAY_ABOUT, ID_TRAY_EXIT, ID_TRAY_LANG_EN, ID_TRAY_LANG_PT, ID_TRAY_START_STOP,
};

/// Comando escolhido pelo usuário no menu de contexto da bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    /// `Start/Stop` — `Iniciar/Parar`.
    ToggleStartStop,
    /// Seleção de idioma no submenu `Language`/`Idioma`.
    SelectLanguage(Language),
    /// `About`/`Sobre`.
    About,
    /// `Exit`/`Fechar`.
    Exit,
}

impl MenuCommand {
    /// Traduz o ID de comando recebido em `WM_COMMAND` de volta para um
    /// [`MenuCommand`], ou `None` se o ID não pertencer a este menu.
    pub fn from_id(id: u16) -> Option<Self> {
        match id {
            ID_TRAY_START_STOP => Some(Self::ToggleStartStop),
            ID_TRAY_LANG_EN => Some(Self::SelectLanguage(Language::En)),
            ID_TRAY_LANG_PT => Some(Self::SelectLanguage(Language::Pt)),
            ID_TRAY_ABOUT => Some(Self::About),
            ID_TRAY_EXIT => Some(Self::Exit),
            _ => None,
        }
    }
}

/// Monta e exibe o menu de contexto na posição atual do cursor, bloqueando
/// até o usuário escolher um item ou fechar o menu (clicando fora ou com Esc).
///
/// `active` decide o rótulo do primeiro item (`Start`/`Stop`) e `language`
/// decide tanto os textos quanto qual opção do submenu aparece marcada.
pub fn show(
    hwnd: HWND,
    active: bool,
    language: Language,
) -> windows::core::Result<Option<MenuCommand>> {
    let strings = language.strings();

    // SAFETY: chamadas de menu Win32 padrão, sem pré-condições além de
    // executar na thread de UI — garantido pelo chamador (`WndProc`).
    let menu = unsafe { CreatePopupMenu() }?;
    let lang_menu = unsafe { CreatePopupMenu() }?;

    let start_stop_label = if active {
        strings.menu_stop
    } else {
        strings.menu_start
    };
    append_string(menu, ID_TRAY_START_STOP, start_stop_label)?;

    append_checked(
        lang_menu,
        ID_TRAY_LANG_EN,
        strings.menu_language_en,
        language == Language::En,
    )?;
    append_checked(
        lang_menu,
        ID_TRAY_LANG_PT,
        strings.menu_language_pt,
        language == Language::Pt,
    )?;
    append_submenu(menu, lang_menu, strings.menu_language)?;

    append_string(menu, ID_TRAY_ABOUT, strings.menu_about)?;
    append_separator(menu)?;
    append_string(menu, ID_TRAY_EXIT, strings.menu_exit)?;

    // Exigido pela documentação da Microsoft para menus de bandeja: sem
    // isso, o menu pode não fechar corretamente quando o usuário clica fora
    // dele.
    // SAFETY: `hwnd` é a janela oculta de mensagens do NukaBoost, válida
    // durante toda a vida do processo.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }

    let mut cursor = POINT::default();
    // SAFETY: `cursor` é um ponteiro de saída válido na pilha.
    unsafe { GetCursorPos(&mut cursor) }?;

    let flags = TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN;
    // SAFETY: `menu` é válido e não é usado por nenhuma outra thread; a
    // chamada bloqueia até o usuário decidir.
    let selected = unsafe { TrackPopupMenu(menu, flags, cursor.x, cursor.y, None, hwnd, None) };

    // Workaround documentado pela Microsoft: uma mensagem inócua após
    // `TrackPopupMenu` evita que o menu reapareça em certos cliques fora dele.
    // SAFETY: `hwnd` é válida; `WM_NULL` não carrega dados.
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
    }

    // SAFETY: `menu` e `lang_menu` foram criados por `CreatePopupMenu`
    // acima e cada um é destruído exatamente uma vez aqui.
    unsafe {
        let _ = DestroyMenu(lang_menu);
        let _ = DestroyMenu(menu);
    }

    // Com `TPM_RETURNCMD`, o valor de retorno é o ID do item escolhido
    // (0 se o usuário cancelou o menu sem selecionar nada).
    let id = selected.0 as u16;
    Ok(MenuCommand::from_id(id))
}

fn append_string(menu: HMENU, id: u16, text: &str) -> windows::core::Result<()> {
    let label = HSTRING::from(text);
    // SAFETY: `menu` é válido; `label` vive até o fim desta chamada síncrona.
    unsafe { AppendMenuW(menu, MF_STRING, id as usize, &label) }
}

fn append_checked(menu: HMENU, id: u16, text: &str, checked: bool) -> windows::core::Result<()> {
    let label = HSTRING::from(text);
    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
    // SAFETY: mesmo contrato de `append_string`.
    unsafe { AppendMenuW(menu, flags, id as usize, &label) }
}

fn append_separator(menu: HMENU) -> windows::core::Result<()> {
    // SAFETY: separadores não carregam texto; `PCWSTR::null()` é aceito
    // pela API para este caso.
    unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null()) }
}

fn append_submenu(parent: HMENU, submenu: HMENU, text: &str) -> windows::core::Result<()> {
    let label = HSTRING::from(text);
    // SAFETY: `submenu.0 as usize` é o uso documentado de `uIDNewItem`
    // quando `MF_POPUP` está presente: o Windows o reinterpreta como o
    // handle do submenu, não como um ID de comando.
    unsafe { AppendMenuW(parent, MF_STRING | MF_POPUP, submenu.0 as usize, &label) }
}

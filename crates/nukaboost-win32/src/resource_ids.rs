//! Identificadores numéricos compartilhados entre o código Rust e
//! `resources/nukaboost.rc`.
//!
//! O compilador de recursos (`rc.exe`) não tem acesso a este arquivo — os
//! mesmos números aparecem literalmente em `nukaboost.rc`. **Ao alterar um
//! valor aqui, altere também o `.rc` correspondente.** Os comentários em
//! cada constante indicam a linha esperada no `.rc`.

/// Ícone de estado `Inactive` (cinza). Ver `IDI_INACTIVE` em `nukaboost.rc`.
pub const IDI_INACTIVE: u16 = 101;
/// Ícone de estado `Active` (verde). Ver `IDI_ACTIVE` em `nukaboost.rc`.
pub const IDI_ACTIVE: u16 = 102;
/// Ícone de estado `Error` (vermelho). Ver `IDI_ERROR` em `nukaboost.rc`.
pub const IDI_ERROR: u16 = 103;

/// Diálogo "About NukaBoost". Ver `IDD_ABOUT` em `nukaboost.rc`.
pub const IDD_ABOUT: u16 = 201;
/// Diálogo do aviso de segurança inicial. Ver `IDD_SAFETY_WARNING` em `nukaboost.rc`.
pub const IDD_SAFETY_WARNING: u16 = 202;

/// Controle estático de título no diálogo About.
pub const IDC_ABOUT_TITLE: u16 = 1001;
/// Controle estático de versão no diálogo About.
pub const IDC_ABOUT_VERSION: u16 = 1002;
/// Controle estático de estado atual no diálogo About.
pub const IDC_ABOUT_STATE: u16 = 1003;
/// Controle estático de descrição no diálogo About.
pub const IDC_ABOUT_DESC: u16 = 1004;
/// Controle estático "NukaBoost nunca mantém a tela ligada".
pub const IDC_ABOUT_SCREEN_NOTE: u16 = 1005;
/// Controle estático do aviso de ventilação.
pub const IDC_ABOUT_VENTILATION: u16 = 1006;
/// Controle estático do aviso contra encerramento forçado do processo.
pub const IDC_ABOUT_TERMINATION: u16 = 1007;
/// Controle estático com o texto do aviso em inglês (sempre presente).
pub const IDC_WARNING_TEXT_EN: u16 = 1101;
/// Controle estático com o texto do aviso em português (sempre presente).
pub const IDC_WARNING_TEXT_PT: u16 = 1102;
/// Caixa de seleção "Do not show this warning again".
pub const IDC_WARNING_CHECKBOX: u16 = 1103;

/// Item de menu Start/Stop (rótulo dinâmico conforme o estado atual).
pub const ID_TRAY_START_STOP: u16 = 2001;
/// Item de menu "English (EN)".
pub const ID_TRAY_LANG_EN: u16 = 2002;
/// Item de menu "Português (PT)".
pub const ID_TRAY_LANG_PT: u16 = 2003;
/// Item de menu About/Sobre.
pub const ID_TRAY_ABOUT: u16 = 2004;
/// Item de menu Exit/Fechar.
pub const ID_TRAY_EXIT: u16 = 2005;

/// Identificador (`uID`) fixo do ícone na área de notificações, exigido por
/// `Shell_NotifyIconW`. Combinado com o GUID estável da aplicação (ver
/// [`crate::tray::icon`]) para sobreviver a reinícios do Explorer.
pub const TRAY_ICON_UID: u32 = 1;

/// Mensagem privada de callback do ícone da bandeja
/// (`uCallbackMessage` de `NOTIFYICONDATAW`), acima de `WM_APP` para não
/// colidir com mensagens padrão do sistema.
pub const WM_NUKABOOST_TRAYICON: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;

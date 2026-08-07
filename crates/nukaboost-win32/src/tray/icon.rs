//! Ícone da área de notificações (`Shell_NotifyIconW`) e seus três estados
//! visuais — a seção 3 do plano recomenda um terceiro estado (`Error`) além
//! dos dois ícones originalmente pedidos, para nunca exibir informação
//! falsa sobre proteções incompletas.

use windows::core::GUID;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_GUID, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_ERROR,
    NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW,
    NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{LoadIconW, HICON};

use nukaboost_core::i18n::{Language, Strings};

use crate::resource_ids::{
    IDI_ACTIVE, IDI_ERROR, IDI_INACTIVE, TRAY_ICON_UID, WM_NUKABOOST_TRAYICON,
};
use crate::wide::{copy_to_fixed_buffer, resource_id};

/// GUID estável do ícone do NukaBoost, usado com `NIF_GUID` para que o
/// Windows identifique o mesmo ícone entre reinícios do Explorer (seção 17
/// do plano: "Usar um GUID estável para o ícone via `Shell_NotifyIconW`").
///
/// Gerado uma única vez para este projeto — não deve mudar entre versões,
/// ou o Windows tratará o ícone como um item novo (perdendo a posição que
/// o usuário eventualmente tenha fixado manualmente).
const TRAY_ICON_GUID: GUID = GUID::from_u128(0x6e3a1f2a_2b3c_4d5e_9f10_1a2b3c4d5e6f);

/// Estado visual do ícone da bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayVisualState {
    /// Ícone cinza/apagado — nenhuma proteção em vigor.
    Inactive,
    /// Ícone colorido — todas as proteções confirmadas.
    Active,
    /// Ícone vermelho com exclamação — proteção incompleta ou não confirmada.
    Error,
}

impl TrayVisualState {
    fn icon_resource_id(self) -> u16 {
        match self {
            Self::Inactive => IDI_INACTIVE,
            Self::Active => IDI_ACTIVE,
            Self::Error => IDI_ERROR,
        }
    }

    fn tooltip(self, strings: &Strings) -> &str {
        match self {
            Self::Inactive => strings.tray_tooltip_inactive,
            Self::Active => strings.tray_tooltip_active,
            Self::Error => strings.tray_tooltip_error,
        }
    }
}

/// Alça RAII do ícone na área de notificações.
///
/// Ao ser descartado, o ícone é removido com `NIM_DELETE` — nenhum
/// caminho de encerramento do processo (normal, `Exit`, ou até um `panic`
/// que dispare o `Drop`) deixa um ícone fantasma na bandeja.
pub struct TrayIcon {
    hwnd: HWND,
    added: bool,
}

impl TrayIcon {
    /// Associa o ícone à janela oculta de mensagens `hwnd`. Nenhuma chamada
    /// Win32 é feita ainda — use [`TrayIcon::set_state`] para efetivamente
    /// adicionar o ícone à bandeja.
    pub fn new(hwnd: HWND) -> Self {
        Self { hwnd, added: false }
    }

    /// Carrega um ícone embutido no próprio executável pelo seu ID de
    /// recurso. Ícones carregados assim são geridos pelo sistema e **não**
    /// devem ser destruídos manualmente com `DestroyIcon` — diferente de
    /// ícones criados a partir de arquivos externos.
    fn load_icon(id: u16) -> windows::core::Result<HICON> {
        // SAFETY: `None` pede o handle do próprio módulo executável atual;
        // chamada síncrona sem pré-condições adicionais.
        let instance = unsafe { GetModuleHandleW(None) }?;
        // SAFETY: `resource_id(id)` produz um ponteiro-índice válido no
        // formato exigido pela API (equivalente a `MAKEINTRESOURCEW`); o
        // ícone com esse ID existe no `.rc` embutido pelo `build.rs`.
        unsafe { LoadIconW(Some(instance.into()), resource_id(id)) }
    }

    fn build_data(
        &self,
        state: TrayVisualState,
        language: Language,
    ) -> windows::core::Result<NOTIFYICONDATAW> {
        let icon = Self::load_icon(state.icon_resource_id())?;

        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_UID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP | NIF_GUID,
            uCallbackMessage: WM_NUKABOOST_TRAYICON,
            hIcon: icon,
            guidItem: TRAY_ICON_GUID,
            ..Default::default()
        };
        copy_to_fixed_buffer(&mut data.szTip, state.tooltip(language.strings()));

        Ok(data)
    }

    /// Adiciona o ícone (na primeira chamada) ou atualiza seu ícone/tooltip
    /// (nas chamadas seguintes) para refletir `state` e `language`.
    ///
    /// Na primeira adição, também opta pelo comportamento de versão 4 do
    /// shell (`NIM_SETVERSION`), que entrega cliques e teclado através de
    /// `WM_CONTEXTMENU` em vez das mensagens legadas — o comportamento
    /// esperado pelo `WndProc` deste crate.
    pub fn set_state(
        &mut self,
        state: TrayVisualState,
        language: Language,
    ) -> windows::core::Result<()> {
        let mut data = self.build_data(state, language)?;
        let mut newly_added = !self.added;

        // SAFETY: `data` é uma `NOTIFYICONDATAW` totalmente inicializada,
        // válida durante esta chamada síncrona.
        if self.added
            && unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }
                .ok()
                .is_err()
        {
            // O Explorer pode ter reiniciado sem que a aplicação tenha
            // processado `TaskbarCreated`; nesse caso, recrie em vez de
            // permanecer silenciosamente sem ícone.
            self.added = false;
            newly_added = true;
        }
        if !self.added {
            unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.ok()?;
            self.added = true;
        }

        if newly_added {
            // Escrita em campo de união: sempre segura em Rust (apenas a
            // leitura de um campo de união exige `unsafe`).
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            // SAFETY: mesmo contrato da chamada acima.
            unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) }.ok()?;
        }

        Ok(())
    }

    /// Força nova adição após a mensagem global `TaskbarCreated`.
    pub fn recreate(
        &mut self,
        state: TrayVisualState,
        language: Language,
    ) -> windows::core::Result<()> {
        self.added = false;
        self.set_state(state, language)
    }

    /// Exibe uma notificação balão associada ao ícone (seção 3: "Emitir uma
    /// notificação ao ativar/desativar"), reaproveitando o ícone e GUID já
    /// registrados.
    pub fn show_balloon(
        &self,
        title: &str,
        body: &str,
        is_error: bool,
    ) -> windows::core::Result<()> {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_UID,
            uFlags: NIF_INFO | NIF_GUID,
            guidItem: TRAY_ICON_GUID,
            dwInfoFlags: if is_error { NIIF_ERROR } else { NIIF_INFO },
            ..Default::default()
        };
        copy_to_fixed_buffer(&mut data.szInfoTitle, title);
        copy_to_fixed_buffer(&mut data.szInfo, body);

        // SAFETY: mesmo contrato de `set_state`.
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.ok()
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        if !self.added {
            return;
        }
        let data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_UID,
            uFlags: NIF_GUID,
            guidItem: TRAY_ICON_GUID,
            ..Default::default()
        };
        // SAFETY: `data` identifica o mesmo ícone criado por `set_state`;
        // chamar `NIM_DELETE` mais de uma vez seria inofensivo, mas
        // `self.added` garante que isso não aconteça.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }
}

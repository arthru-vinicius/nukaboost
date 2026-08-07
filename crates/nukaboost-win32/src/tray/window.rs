//! Janela oculta de mensagens que hospeda o ícone da bandeja e recebe seus
//! eventos (seção 3 do plano).
//!
//! O `WndProc` nunca executa lógica de negócio: ele apenas traduz mensagens
//! Win32 em [`TrayEvent`] e os envia por um canal para a thread que
//! realmente orquestra ativação/desativação (`apps/nukaboost`). Isso evita
//! bloquear a fila de mensagens da UI com chamadas potencialmente lentas
//! (energia, IPC). O único estado lido diretamente pelo `WndProc` é
//! [`TrayViewState`] — dois campos simples atrás de um `Mutex`, usados só
//! para desenhar o menu de contexto.

use std::ffi::c_void;
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{NINF_KEY, NIN_SELECT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetWindowLongPtrW, TranslateMessage,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_MESSAGE, MSG, WINDOW_EX_STYLE,
    WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY,
    WM_QUERYENDSESSION, WNDCLASSEXW, WS_OVERLAPPED,
};

use nukaboost_core::i18n::Language;

use crate::resource_ids::WM_NUKABOOST_TRAYICON;
use crate::tray::menu::{self, MenuCommand};

/// Nome da classe de janela, único para não colidir com outros aplicativos.
const WINDOW_CLASS_NAME: windows::core::PCWSTR = w!("NukaBoostTrayWindowClass");

/// Eventos que a janela oculta entrega à aplicação, já traduzidos de
/// mensagens Win32 cruas para conceitos de domínio.
#[derive(Debug)]
pub enum TrayEvent {
    /// Clique esquerdo no ícone — alterna ativo/inativo (seção 3: "Clique
    /// esquerdo alterna entre ativo e inativo").
    ToggleRequested,
    /// Item de idioma escolhido no submenu `Language`/`Idioma`.
    LanguageSelected(Language),
    /// Item `About`/`Sobre` escolhido.
    AboutRequested,
    /// Item `Exit`/`Fechar` escolhido.
    ExitRequested,
    /// O Explorer foi reiniciado (`TaskbarCreated`); o ícone precisa ser
    /// recriado (seção 17: "Se o Explorer reiniciar, recriar o ícone
    /// automaticamente").
    TaskbarRecreated,
    /// `WM_QUERYENDSESSION`/`WM_ENDSESSION`: o Windows está encerrando a
    /// sessão (logoff, desligamento ou reinício) e espera que o processo
    /// restaure suas configurações e se encerre rapidamente.
    SessionEnding { completion: SyncSender<bool> },
}

/// Estado mínimo necessário para desenhar o menu de contexto corretamente:
/// se o rótulo deve ser `Start`/`Stop` e qual idioma marcar no submenu.
///
/// Mantido deliberadamente pequeno e `Copy` — é lido pelo `WndProc` a cada
/// abertura do menu, e nunca deve exigir uma chamada lenta para ser
/// atualizado.
#[derive(Debug, Clone, Copy)]
pub struct TrayViewState {
    pub active: bool,
    pub language: Language,
}

struct WindowState {
    events: Sender<TrayEvent>,
    view: Arc<Mutex<TrayViewState>>,
    taskbar_created_message: u32,
}

/// Janela oculta de mensagens que representa o NukaBoost na área de
/// notificações.
pub struct TrayWindow {
    hwnd: HWND,
}

// SAFETY: `HWND` é um identificador de kernel opaco; chamadas Win32 sobre
// ele (como `PostMessageW`) são seguras a partir de qualquer thread.
unsafe impl Send for TrayWindow {}

impl TrayWindow {
    /// Registra a classe de janela (se ainda não registrada neste processo)
    /// e cria a janela oculta, associando-a ao canal de eventos e ao
    /// estado compartilhado do menu.
    pub fn create(
        events: Sender<TrayEvent>,
        view: Arc<Mutex<TrayViewState>>,
    ) -> windows::core::Result<Self> {
        // SAFETY: `None` pede o handle do módulo executável atual.
        let instance = unsafe { GetModuleHandleW(None) }?;

        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: WINDOW_CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` é uma `WNDCLASSEXW` válida na pilha; registrar a
        // mesma classe duas vezes (por exemplo, em testes) retorna erro,
        // que ignoramos deliberadamente logo abaixo.
        let atom = unsafe { RegisterClassExW(&class) };
        if atom == 0 {
            let err = windows::core::Error::from_thread();
            let already_exists = windows::core::HRESULT::from_win32(
                windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS.0,
            );
            if err.code() != already_exists {
                return Err(err);
            }
        }

        // SAFETY: `RegisterWindowMessageW` apenas registra um identificador
        // de mensagem global no sistema; não tem pré-condições.
        let taskbar_created_message = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

        let state = Box::new(WindowState {
            events,
            view,
            taskbar_created_message,
        });
        let state_ptr = Box::into_raw(state) as *const c_void;

        // SAFETY: todos os parâmetros são válidos; `state_ptr` é recuperado
        // em `WM_NCCREATE` (a primeira mensagem recebida pela janela) e sua
        // posse passa para a própria janela — liberado em `WM_NCDESTROY`.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WINDOW_CLASS_NAME,
                w!("NukaBoost"),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                Some(state_ptr),
            )
        }?;

        Ok(Self { hwnd })
    }

    /// Handle da janela oculta, usado por [`crate::tray::icon::TrayIcon`] e
    /// pelos diálogos (como proprietário/`hwnd` de referência).
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Bombeia a fila de mensagens até `WM_QUIT`. Bloqueia a thread atual —
    /// deve ser chamada pela mesma thread que criou a janela (normalmente a
    /// thread principal do processo).
    pub fn run_message_loop() {
        let mut msg = MSG::default();
        loop {
            // SAFETY: `msg` é um `MSG` válido na pilha; `None` pede
            // mensagens de qualquer janela desta thread (inclusive as sem
            // janela, como as postadas via `PostThreadMessage`).
            let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if result.0 <= 0 {
                break; // 0 = WM_QUIT; -1 = erro.
            }
            // SAFETY: `msg` acabou de ser preenchida por `GetMessageW`.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: para `WM_NCCREATE`, `lparam` aponta para um
        // `CREATESTRUCTW` cujo `lpCreateParams` é o ponteiro que passamos
        // em `CreateWindowExW`.
        let create_struct = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        // SAFETY: `SetWindowLongPtrW` apenas armazena o valor; nenhuma
        // outra pré-condição.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_struct.lpCreateParams as isize);
        }
        // SAFETY: mesmo contrato de `DefWindowProcW` em geral.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    // SAFETY: se `WM_NCCREATE` já foi processada, este ponteiro é o mesmo
    // `Box::into_raw` armazenado acima e permanece válido até `WM_NCDESTROY`.
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if state_ptr.is_null() {
        // SAFETY: nenhum estado associado ainda (mensagens muito cedo, como
        // `WM_GETMINMAXINFO`); comportamento padrão é seguro.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    // SAFETY: `state_ptr` não é nulo e continua válido conforme o
    // comentário acima; usamos uma referência, não tomamos posse.
    let state = unsafe { &*state_ptr };

    match msg {
        WM_NUKABOOST_TRAYICON => {
            let mouse_message = loword(lparam.0 as u32) as u32;
            if mouse_message == WM_LBUTTONUP
                || mouse_message == NIN_SELECT
                || mouse_message == (NIN_SELECT | NINF_KEY)
            {
                let _ = state.events.send(TrayEvent::ToggleRequested);
            } else if mouse_message == WM_CONTEXTMENU {
                show_context_menu(hwnd, state);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            // WM_COMMAND também pode chegar de outras fontes; só tratamos
            // os IDs que reconhecemos.
            let id = loword(wparam.0 as u32);
            if let Some(command) = MenuCommand::from_id(id) {
                dispatch_menu_command(state, command);
            }
            LRESULT(0)
        }
        WM_QUERYENDSESSION => {
            let (completion, result) = std::sync::mpsc::sync_channel(1);
            if state
                .events
                .send(TrayEvent::SessionEnding { completion })
                .is_err()
            {
                return LRESULT(0);
            }
            match result.recv_timeout(Duration::from_secs(20)) {
                Ok(true) => LRESULT(1),
                Ok(false) | Err(_) => LRESULT(0),
            }
        }
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                unsafe { PostQuitMessage(0) };
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: encerra a fila de mensagens desta thread; chamado
            // apenas quando a própria janela está sendo destruída.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // Última mensagem recebida pela janela: retomamos a posse do
            // `Box` alocado em `TrayWindow::create` para que ele seja
            // liberado corretamente, e limpamos o ponteiro armazenado.
            // SAFETY: `state_ptr` foi originalmente produzido por
            // `Box::into_raw` em `TrayWindow::create` e esta é a única vez
            // em que o reconstruímos.
            unsafe {
                drop(Box::from_raw(state_ptr));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            // SAFETY: contrato padrão de `DefWindowProcW`.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        other if other == state.taskbar_created_message => {
            let _ = state.events.send(TrayEvent::TaskbarRecreated);
            LRESULT(0)
        }
        _ => {
            // SAFETY: contrato padrão de `DefWindowProcW`.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

fn show_context_menu(hwnd: HWND, state: &WindowState) {
    let view = *state
        .view
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match menu::show(hwnd, view.active, view.language) {
        Ok(Some(command)) => dispatch_menu_command(state, command),
        Ok(None) => {}
        Err(_) => {
            // A falha em exibir o menu não é uma condição de proteção
            // incompleta; registrá-la é responsabilidade da aplicação, que
            // tem acesso ao logger (`tracing`) — este crate não depende dele.
        }
    }
}

fn dispatch_menu_command(state: &WindowState, command: MenuCommand) {
    let event = match command {
        MenuCommand::ToggleStartStop => TrayEvent::ToggleRequested,
        MenuCommand::SelectLanguage(language) => TrayEvent::LanguageSelected(language),
        MenuCommand::About => TrayEvent::AboutRequested,
        MenuCommand::Exit => TrayEvent::ExitRequested,
    };
    let _ = state.events.send(event);
}

const fn loword(value: u32) -> u16 {
    (value & 0xFFFF) as u16
}

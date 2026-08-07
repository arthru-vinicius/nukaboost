//! Área de notificações do NukaBoost: janela oculta de mensagens, ícone de
//! três estados e menu de contexto (seção 3 do plano).

pub mod icon;
pub mod menu;
pub mod window;

pub use icon::{TrayIcon, TrayVisualState};
pub use menu::MenuCommand;
pub use window::{TrayEvent, TrayViewState, TrayWindow};

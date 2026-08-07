//! # nukaboost-win32
//!
//! Camada de interface nativa do NukaBoost: a janela oculta de mensagens e
//! o ícone da área de notificações ([`tray`]), os diálogos About e de aviso
//! de segurança ([`dialogs`]), e os IDs de recurso compartilhados com
//! `resources/nukaboost.rc` ([`resource_ids`]).
//!
//! Deliberadamente implementado em Win32 puro via a crate `windows` — sem
//! WebView, sem framework de UI multiplataforma e sem uma *event loop* de
//! terceiros — conforme a seção 16 do plano do produto. Este crate não
//! decide *quando* ativar ou desativar o NukaBoost; ele apenas traduz
//! eventos de UI em valores do tipo [`tray::TrayEvent`] e exibe o estado
//! que `apps/nukaboost` lhe fornece.

pub mod dialogs;
pub mod resource_ids;
pub mod tray;
pub mod wide;

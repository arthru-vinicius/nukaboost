//! # nukaboost-core
//!
//! Núcleo de domínio do NukaBoost, sem nenhuma dependência de interface
//! gráfica. Este crate concentra toda a lógica que pode ser testada sem um
//! notebook real e sem uma sessão interativa do Windows:
//!
//! - [`power`] — plano de energia temporário, Power Requests e a thread de
//!   `SetThreadExecutionState` (seção 6 do plano).
//! - [`state`] — máquina de estados (`Inactive`/`Activating`/`Active`/
//!   `Deactivating`/`Error`) e leases de agentes (seções 3 e 12).
//! - [`recovery`] — journal atômico e identificação robusta de processos
//!   para o watchdog (seção 9).
//! - [`ipc`] — protocolo de mensagens do Named Pipe local (seção 11).
//! - [`config`] — preferências persistidas do usuário (idioma, aviso de
//!   segurança confirmado).
//! - [`i18n`] — textos localizados em inglês (padrão) e português.
//! - [`startup`] — atalho de inicialização com o Windows (seção 14).
//!
//! Os binários `apps/nukaboost` (bandeja) e `apps/nukaboostctl` (CLI), bem
//! como o crate `nukaboost-win32` (janelas e diálogos nativos), dependem
//! deste crate mas nunca o contrário.

pub mod config;
pub mod error;
pub mod fs_atomic;
pub mod i18n;
pub mod ipc;
pub mod paths;
pub mod power;
pub mod recovery;
pub mod startup;
pub mod state;

pub use error::{NukaError, NukaResult};

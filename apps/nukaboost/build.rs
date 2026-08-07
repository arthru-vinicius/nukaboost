//! Compila e embute os recursos Win32 de `NukaBoost.exe`: os três ícones de
//! estado, informações de versão, o manifesto (Common Controls v6,
//! reconhecimento de DPI, `asInvoker`) e os diálogos `DIALOGEX` (About e
//! aviso de segurança).
//!
//! O `.rc` fica fisicamente em `crates/nukaboost-win32/resources/`, junto
//! do código Rust que o consome (`tray/`, `dialogs/`), mas só pode ser
//! compilado e linkado a partir do `build.rs` de um crate com alvo
//! `[[bin]]` — daí ele rodar aqui, e não no crate de biblioteca.

fn main() {
    println!("cargo:rerun-if-changed=../../crates/nukaboost-win32/resources/nukaboost.rc");
    println!("cargo:rerun-if-changed=../../crates/nukaboost-win32/resources/nukaboost.manifest");
    println!("cargo:rerun-if-changed=../../assets/inactive.ico");
    println!("cargo:rerun-if-changed=../../assets/active.ico");
    println!("cargo:rerun-if-changed=../../assets/error.ico");

    embed_resource::compile(
        "../../crates/nukaboost-win32/resources/nukaboost.rc",
        embed_resource::NONE,
    )
    .manifest_required()
    .unwrap();
}

//! Embute manifesto, ícone e metadados de versão no executável de console.

fn main() {
    println!("cargo:rerun-if-changed=../../crates/nukaboost-win32/resources/nukaboostctl.rc");
    println!("cargo:rerun-if-changed=../../crates/nukaboost-win32/resources/nukaboostctl.manifest");
    println!("cargo:rerun-if-changed=../../assets/active.ico");

    embed_resource::compile(
        "../../crates/nukaboost-win32/resources/nukaboostctl.rc",
        embed_resource::NONE,
    )
    .manifest_required()
    .unwrap();
}

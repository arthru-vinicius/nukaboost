# Contribuindo

1. Abra uma issue para mudanças de comportamento relevantes.
2. Crie uma branch curta e mantenha o escopo focado.
3. Execute antes do pull request:

   ```powershell
   cargo fmt --all -- --check
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   cargo build --release
   ```

4. Mudanças em energia/recuperação devem incluir testes e explicar como o
   plano original continua recuperável em cada caminho de erro.
5. Nunca inclua certificados, chaves, arquivos PFX ou senhas no repositório.

Pull requests devem descrever risco, validação feita e impacto no protocolo
IPC/instalador. Vulnerabilidades seguem [SECURITY.md](SECURITY.md), não issues
públicas.

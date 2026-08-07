# NukaBoost

Utilitário nativo para Windows 11 que mantém o sistema acordado durante
compilações, downloads e outras tarefas longas, sem manter a tela ligada.
Escrito em Rust, opera pela área de notificação e também oferece um CLI com
leases temporárias para automação.

> [!IMPORTANT]
> O NukaBoost altera temporariamente configurações do plano de energia. Ele
> preserva a ação de bateria crítica e restaura o plano original ao parar,
> sair, desinstalar ou após uma interrupção detectada pelo watchdog. Ainda
> assim, não substitui alimentação elétrica confiável nem impede desligamentos
> térmicos, críticos, explícitos ou impostos por política/firmware.

## Estado do projeto

O código está em preparação para a primeira release pública. Builds locais e
artefatos de CI sem assinatura são destinados a teste. Não publique o MSI
como release estável até concluir a assinatura Authenticode descrita em
[docs/RELEASE.md](docs/RELEASE.md).

## Recursos

- plano temporário; o plano original nunca é modificado;
- proteção para tomada e bateria, inclusive com Economia de Bateria;
- tela livre para apagar normalmente;
- confirmação por releitura antes de exibir o estado ativo;
- journal atômico, RunOnce e watchdog para recuperação;
- leases com TTL, isolamento por identificador e expiração automática;
- Named Pipe local limitado ao usuário/administradores e a clientes locais;
- interface em inglês e português;
- instalação por usuário, sem executar o tray como administrador.

## Uso

```powershell
nukaboostctl status --json
nukaboostctl start
nukaboostctl stop
nukaboostctl acquire --reason "cargo build" --owner "codex" --ttl 2h --json
nukaboostctl release <lease-id>
nukaboostctl run -- cargo build --release
nukaboostctl startup enable
```

Na primeira execução, confirme o aviso de segurança pela interface. Uma
ativação via CLI é recusada enquanto esse aviso não tiver sido reconhecido.

## Desenvolvimento

Pré-requisitos: Windows 11, Rust 1.94.1 e WiX Toolset 7 com as extensões UI e
Util.

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release

wix build installer\NukaBoost.wxs `
  -arch x64 `
  -b assets=assets `
  -b release=target\release `
  -ext WixToolset.UI.wixext `
  -ext WixToolset.Util.wixext `
  -out installer\NukaBoost.msi

wix msi validate -sice ICE91 installer\NukaBoost.msi
```

ICE91 é suprimido porque o pacote é intencionalmente apenas por usuário e
instala em `%LocalAppData%`; todas as demais regras ICE continuam ativas.

## Documentação

- [Plano do produto](docs/plano.md)
- [Release e assinatura](docs/RELEASE.md)
- [Política de assinatura de código](CODE_SIGNING_POLICY.md)
- [Política de segurança](SECURITY.md)
- [Privacidade](PRIVACY.md)
- [Como contribuir](CONTRIBUTING.md)

Licenciado sob a [MIT License](LICENSE).

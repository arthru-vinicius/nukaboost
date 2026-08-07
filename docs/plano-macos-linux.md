# Plano de portabilidade do NukaBoost para macOS e Linux

## 1. Propósito

Este documento complementa o [plano original para Windows](plano.md). O objetivo
é transformar o NukaBoost em um produto multiplataforma sem duplicar o domínio,
o protocolo, a gestão de leases ou as traduções.

A portabilidade não consiste apenas em trocar chamadas Win32 por chamadas Unix.
Cada sistema possui mecanismos diferentes para energia, interface, IPC,
inicialização automática, instalação e assinatura. Além disso, macOS e Linux
não oferecem exatamente as mesmas garantias que o Windows atual.

Princípios obrigatórios:

- usar apenas APIs públicas e documentadas;
- nunca simular entrada de teclado ou mouse;
- nunca alterar permanentemente a política de energia do usuário;
- não exigir processo permanente como `root`;
- liberar toda inibição quando o processo terminar;
- manter a tela livre para apagar normalmente;
- informar com precisão quais proteções estão realmente disponíveis;
- preservar o comportamento `manual_hold || leases > 0` em todas as plataformas.

## 2. Decisão organizacional

### 2.1 Um único repositório

A implementação deve continuar neste repositório, no mesmo Rust workspace. Não
é recomendado criar um fork permanente nem um segundo repositório para macOS e
Linux.

Um fork é apropriado quando outro mantenedor pretende desenvolver uma variante
independente, com decisões e histórico próprios. Neste caso, o produto, o
protocolo e grande parte das regras são os mesmos. Separar os repositórios
duplicaria código e facilitaria divergências de segurança, documentação e
comportamento.

O fluxo recomendado é:

1. Criar uma branch como `feature/macos-linux`.
2. Implementar as etapas deste plano em commits pequenos.
3. Manter a `main` protegida por ruleset.
4. Integrar cada etapa por pull request revisado.
5. Publicar artefatos distintos por sistema na mesma GitHub Release.

Branches experimentais podem existir no mesmo repositório. Um repositório
separado só se justificaria se a versão Unix se tornasse outro produto.

### 2.2 Um produto, vários artefatos

O nome e a versão do produto permanecem comuns. Uma tag `v1.2.0`, por exemplo,
pode publicar:

- `NukaBoost-1.2.0-windows-x86_64.msi`;
- `NukaBoost-1.2.0-macos-universal.dmg`;
- `nukaboost-1.2.0-linux-x86_64.tar.gz`;
- pacotes Linux adicionais quando estiverem maduros.

O versionamento do protocolo é independente do formato dos pacotes.

## 3. Escopo e promessas por plataforma

O aplicativo não deve traduzir diferenças técnicas em promessas falsas.

| Capacidade | Windows | Linux com systemd-logind | Linux via portal | macOS |
|---|---|---|---|---|
| Impedir repouso automático por inatividade | Sim | Sim | Sim | Sim |
| Permitir que a tela apague | Sim | Sim | Sim | Sim |
| Inibir ação automática da tampa | Sim, conforme o plano atual | Geralmente sim, sujeito a logind e PolicyKit | Não | Não há garantia pública geral |
| Funcionar sem administrador | Sim, salvo política corporativa | Normalmente, sujeito à política do sistema | Sim, sujeito ao portal | Sim |
| Liberar proteção após crash | Watchdog e recuperação | Fechamento automático do descritor | Fechamento da requisição | Liberação automática da assertion |
| Impedir desligamento térmico ou crítico | Não | Não | Não | Não |
| Impedir ação explícita do usuário | Não prometido | Não prometido | Não prometido | Não prometido |

No macOS, fechar a tela de um notebook é uma ação normal de repouso documentada
pela Apple. O modo com tampa fechada depende das condições aceitas pelo próprio
sistema, como alimentação e periféricos externos. A versão macOS deve dizer
“impedir repouso por inatividade”, nunca “funcionar com a tampa fechada” de forma
irrestrita. Referências: [repouso no Mac](https://support.apple.com/en-euro/guide/mac-help/mh10330/mac)
e [uso de monitor externo com a tampa fechada](https://support.apple.com/en-us/102501).

## 4. Arquitetura pretendida

O workspace deve separar regras portáveis de adaptadores de sistema:

```text
crates/
  nukaboost-core/          estado, leases, protocolo, i18n e modelos puros
  nukaboost-platform/      traits, capacidades e erros de plataforma
  nukaboost-runtime/       orquestração comum sobre os traits
  nukaboost-windows/       energia, IPC, caminhos e Startup do Windows
  nukaboost-win32/         interface Win32 existente
  nukaboost-macos/         IOKit, AppKit, LaunchAgent e socket local
  nukaboost-linux/         logind, portal, desktop Linux e socket local
apps/
  nukaboost/               binário gráfico selecionado por cfg
  nukaboostctl/            CLI comum com transporte selecionado por cfg
packaging/
  windows/
  macos/
  linux/
```

Os nomes podem ser ajustados durante a extração, mas as fronteiras não devem ser
misturadas novamente. Em particular, `nukaboost-core` não pode depender do crate
`windows`, AppKit, IOKit ou D-Bus.

Dependências específicas devem ficar condicionadas ao alvo no `Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]

[target.'cfg(target_os = "macos")'.dependencies]

[target.'cfg(target_os = "linux")'.dependencies]
```

## 5. Componentes portáveis

Os seguintes elementos devem ser preservados ou extraídos do código atual:

- máquina de estados `Inactive`, `Activating`, `Active`, `Deactivating` e
  `ProtectionError`;
- regra de atividade manual mais leases;
- expiração por TTL com relógio monotônico;
- identificadores de lease e isolamento por proprietário;
- mensagens JSON e regras de compatibilidade do protocolo;
- parser de duração e validação de argumentos;
- i18n em inglês e português;
- serialização atômica da configuração;
- política de logs sem dados sensíveis;
- semântica de confirmação antes de informar estado ativo.

Os seguintes itens hoje acoplados ao Windows devem sair do núcleo:

- plano de energia, Power Requests e GUIDs;
- Named Pipe e descritores de segurança;
- `RunOnce` e Startup;
- caminhos baseados em `%LocalAppData%`;
- single instance por mutex Win32;
- detecção de bateria por APIs Win32;
- dialogs, área de notificação e message window.

## 6. Contrato de plataforma

O runtime comum deve depender de interfaces pequenas. Uma direção possível é:

```rust
pub trait SleepInhibitor {
    fn probe(&self) -> Result<CapabilityReport, PlatformError>;
    fn acquire(&self, reason: &str) -> Result<Box<dyn InhibitionGuard>, PlatformError>;
}

pub trait InhibitionGuard: Send {
    fn verify(&self) -> Result<CapabilityReport, PlatformError>;
}

pub trait PlatformPaths {
    fn config_dir(&self) -> Result<PathBuf, PlatformError>;
    fn state_dir(&self) -> Result<PathBuf, PlatformError>;
    fn log_dir(&self) -> Result<PathBuf, PlatformError>;
    fn ipc_endpoint(&self) -> Result<PathBuf, PlatformError>;
}

pub trait Autostart {
    fn status(&self) -> Result<bool, PlatformError>;
    fn enable(&self) -> Result<(), PlatformError>;
    fn disable(&self) -> Result<(), PlatformError>;
}
```

O objeto retornado por `acquire` deve controlar a vida da proteção. Quando o
guard for destruído ou o processo morrer, o sistema operacional deve liberar a
inibição. Não usar um booleano isolado como prova de proteção.

## 7. Relatório de capacidades e protocolo

O relatório atual contém detalhes exclusivos do Windows. Uma versão nova do
protocolo deve representar capacidades, não implementações:

```json
{
  "protocol_version": 3,
  "platform": "linux",
  "backend": "systemd-logind",
  "state": "active",
  "capabilities": {
    "idle_sleep_inhibit": "active",
    "lid_switch_inhibit": "active",
    "explicit_sleep_inhibit": "not_requested",
    "display_sleep_inhibit": "not_requested"
  }
}
```

Valores de capacidade:

- `available`;
- `active`;
- `unsupported`;
- `denied`;
- `unavailable`;
- `not_requested`.

Regras:

- as chaves JSON nunca são traduzidas;
- o protocolo v2 permanece aceito no Windows durante uma janela de migração;
- clientes devem recusar uma versão futura incompatível com erro claro;
- a UI só exibe `Active` quando todas as proteções obrigatórias daquela
  plataforma estiverem confirmadas;
- limitações opcionais aparecem separadamente e não são mascaradas.

## 8. Backend do macOS

### 8.1 Energia

Usar IOKit com `IOPMAssertionCreateWithDescription` e a assertion
`kIOPMAssertionTypePreventUserIdleSystemSleep`. A Apple documenta esses tipos em
[IOPMAssertionTypes](https://developer.apple.com/documentation/iokit/iopmlib_h/iopmassertiontypes)
e a criação em
[IOPMAssertionCreateWithDescription](https://developer.apple.com/documentation/iokit/1557078-iopmassertioncreatewithdescripti).

Regras do backend:

1. Criar a assertion com nome e motivo legíveis.
2. Armazenar o `IOPMAssertionID` em um guard RAII.
3. Considerar a ativação válida somente após retorno `kIOReturnSuccess`.
4. Liberar com `IOPMAssertionRelease` ao desativar.
5. Nunca solicitar `PreventUserIdleDisplaySleep`.
6. Não usar `caffeinate` como backend de produção. Ele pode servir apenas como
   referência manual de teste.
7. Não usar APIs privadas nem contornar repouso térmico, crítico ou explícito.

### 8.2 Tampa fechada

O backend deve reportar `lid_switch_inhibit = unsupported`. Se o macOS mantiver
o computador ativo em modo fechado por condições próprias, isso é uma
capacidade do sistema, não uma garantia fornecida pelo NukaBoost.

### 8.3 Interface

Criar um aplicativo de menu bar com AppKit e `NSStatusItem`, que é a API nativa
documentada pela Apple para itens na barra de menus:
[NSStatusItem](https://developer.apple.com/documentation/appkit/nsstatusitem).

Requisitos:

- `LSUIElement = true` por padrão, sem ícone permanente no Dock;
- clique principal alterna estado com proteção contra cliques concorrentes;
- menu oferece iniciar, parar, status, iniciar no login, sobre e sair;
- ícones ativo, inativo e erro respeitam o padrão visual do macOS;
- textos deixam clara a limitação de tampa fechada;
- acessibilidade, navegação por teclado e VoiceOver são testados.

Uma camada Rust pode chamar AppKit por bindings, mas o event loop e o modelo de
thread da AppKit devem ser respeitados. A lógica de energia não deve rodar
dentro do callback visual por tempo prolongado.

## 9. Backend do Linux

### 9.1 Backend principal com systemd-logind

Usar D-Bus diretamente no método `org.freedesktop.login1.Manager.Inhibit`. O
método recebe `What`, `Who`, `Why` e `Mode` e devolve um descritor de arquivo. A
proteção existe enquanto esse descritor estiver aberto. A documentação de
[systemd-inhibit](https://www.freedesktop.org/software/systemd/man/latest/systemd-inhibit.html)
descreve os mesmos locks.

Solicitação inicial:

```text
What = "idle:handle-lid-switch"
Who  = "NukaBoost"
Why  = motivo da sessão
Mode = "block"
```

O lock genérico `sleep` não deve ser adquirido por padrão, pois também pode
interferir em uma ação explícita do usuário. Ele só pode ser adicionado após
teste de produto, opção explícita e documentação clara.

Regras:

- manter o descritor aberto no guard RAII;
- consultar `ListInhibitors` para diagnóstico e confirmação quando necessário;
- tratar separadamente ausência de systemd, rejeição por PolicyKit e falha de
  D-Bus;
- nunca escrever em `logind.conf`;
- nunca instalar regra ampla de PolicyKit silenciosamente;
- nunca solicitar `sudo` para a operação normal;
- reportar proteção de tampa como `denied` ou `unavailable` quando ela não for
  concedida.

### 9.2 Fallback para XDG Desktop Portal

Em Flatpak ou quando o portal for o mecanismo apropriado, usar
`org.freedesktop.portal.Inhibit`. O portal documenta flags para suspensão e
inatividade, mas não oferece proteção da tampa:
[XDG Desktop Portal Inhibit](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Inhibit.html).

O handle da requisição deve permanecer vivo e ser fechado na desativação. Esse
backend informa `lid_switch_inhibit = unsupported`.

Ordem de seleção inicial:

1. Aplicativo nativo fora de sandbox: systemd-logind.
2. Aplicativo em sandbox: XDG Desktop Portal.
3. Nenhum backend compatível: estado `ProtectionError`, com explicação e
   instrução de diagnóstico.

Não usar `xdotool`, teclas falsas, loops de movimento de mouse ou alteração
global persistente do desktop.

### 9.3 Interface Linux

A fragmentação dos desktops impede depender apenas do tray. A versão inicial
deve oferecer:

- StatusNotifierItem/AppIndicator quando o ambiente oferecer suporte;
- menu com as mesmas ações conceituais das outras plataformas;
- `nukaboostctl` como interface completa e sempre disponível;
- mensagem clara quando o desktop oculta ou não implementa tray;
- nenhuma dependência obrigatória de X11, para funcionar em Wayland.

GNOME, KDE Plasma e ao menos um ambiente sem tray devem fazer parte da matriz de
testes. O app não deve falhar se a interface gráfica não estiver disponível.

## 10. Ativação e desativação transacionais

### 10.1 Ativação comum

1. Validar configuração e consentimento de segurança.
2. Detectar plataforma e backend.
3. Consultar capacidades.
4. Adquirir os guards obrigatórios.
5. Confirmar que os handles são válidos e que o backend os reconhece quando
   houver API de consulta.
6. Publicar o estado `Active`.
7. Atualizar UI e responder ao CLI.

Se qualquer etapa falhar, destruir todos os guards já adquiridos e retornar ao
estado seguro. Uma ativação parcial nunca pode aparecer como ativa.

### 10.2 Desativação comum

1. Bloquear novas transações incompatíveis.
2. Destruir ou fechar todos os guards.
3. Confirmar a remoção quando a plataforma permitir.
4. Publicar `Inactive`.
5. Atualizar UI e responder ao CLI.

No Unix, não há plano de energia temporário a excluir. Isso elimina a principal
necessidade do journal e do watchdog do Windows para rollback de energia.

## 11. Crash, encerramento e recuperação

IOKit, logind e o portal associam a proteção ao processo, assertion, descritor
ou handle. Ao morrer o processo, o sistema deve liberar a proteção. Ainda assim:

- instalar handlers de encerramento apenas para desligamento ordenado;
- não tentar fazer trabalho complexo dentro de signal handlers;
- remover socket obsoleto ao iniciar, depois de verificar que não existe uma
  instância legítima;
- persistir leases somente se houver uma política explícita de restauração;
- por padrão, não restaurar leases ativos após reboot ou logout;
- registrar causa da última saída quando ela puder ser determinada com
  segurança.

O journal de recuperação do Windows permanece no adaptador Windows. Não
replicar esse mecanismo no Unix sem um recurso persistente que realmente exija
rollback.

## 12. IPC local e instância única

Usar Unix domain sockets no macOS e Linux. Requisitos mínimos:

- diretório pai acessível somente ao usuário;
- socket com modo `0600`;
- Linux em `$XDG_RUNTIME_DIR/nukaboost/control.sock`;
- macOS em diretório privado do usuário, preferencialmente sob o diretório
  temporário seguro fornecido pelo sistema;
- validar UID do peer com `SO_PEERCRED` no Linux e `getpeereid` no macOS;
- rejeitar peer de outro usuário mesmo que as permissões tenham sido alteradas;
- limite de tamanho por frame, timeout e JSON estrito;
- não seguir symlinks ao remover um socket obsoleto;
- usar lock file ou bind exclusivo do socket para single instance.

A especificação XDG determina que `$XDG_RUNTIME_DIR` pertence ao usuário, usa
modo `0700` e é apropriado para sockets:
[XDG Base Directory](https://specifications.freedesktop.org/basedir/).

## 13. CLI e leases

Os comandos permanecem comuns:

```text
nukaboostctl status [--json]
nukaboostctl start
nukaboostctl stop
nukaboostctl acquire --reason <texto> --owner <nome> --ttl <duração> [--json]
nukaboostctl release <lease-id>
nukaboostctl run -- <comando> [argumentos]
nukaboostctl startup status|enable|disable
```

Requisitos adicionais:

- `run` deve encaminhar sinais ao processo filho e liberar a lease em qualquer
  término observado;
- códigos de saída são estáveis e documentados;
- `--json` é idêntico entre sistemas, salvo campos de capacidade;
- locale afeta mensagens humanas e help, nunca chaves JSON;
- motivos enviados ao backend são limitados, sanitizados e sem segredos;
- a skill de automação continua usando somente o CLI e não conhece APIs de
  plataforma.

## 14. Caminhos de dados

### 14.1 macOS

```text
Aplicativo: /Applications/NukaBoost.app ou ~/Applications/NukaBoost.app
Configuração e estado: ~/Library/Application Support/NukaBoost/
Logs: ~/Library/Logs/NukaBoost/
Login item: registrado pelo Service Management
```

### 14.2 Linux

Seguir a especificação XDG e respeitar variáveis personalizadas:

```text
Configuração: $XDG_CONFIG_HOME/nukaboost/ ou ~/.config/nukaboost/
Estado: $XDG_STATE_HOME/nukaboost/ ou ~/.local/state/nukaboost/
Logs: dentro do state dir, salvo integração explícita com journald
Socket: $XDG_RUNTIME_DIR/nukaboost/control.sock
Autostart: $XDG_CONFIG_HOME/autostart/nukaboost.desktop
```

Arquivos novos devem ser criados com permissões restritivas. Escritas de
configuração usam arquivo temporário, `fsync` quando relevante e rename atômico.

## 15. Iniciar com o sistema

### 15.1 macOS

A versão mínima proposta é macOS 13. Nela, usar `SMAppService.mainApp` para
registrar e remover o aplicativo nos Login Items, consultar o status real e
direcionar o usuário às configurações quando a aprovação for necessária. A
Apple recomenda `SMAppService` no macOS 13 ou posterior e o apresenta como
substituto da instalação manual em `~/Library/LaunchAgents`:
[SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice).

Se o projeto decidir suportar macOS anterior ao 13, documentar um fallback
LaunchAgent isolado. Não manter os dois mecanismos ativos ao mesmo tempo, não
executar como root e não esconder do usuário o estado do login item.

### 15.2 Linux

Usar inicialmente a especificação de autostart do desktop com um arquivo
`.desktop` por usuário:
[Desktop Application Autostart](https://specifications.freedesktop.org/autostart-spec/).

Um serviço systemd de usuário pode ser oferecido futuramente para modo sem UI,
mas não deve ser requisito para o aplicativo gráfico nem competir com o
autostart do desktop.

## 16. Empacotamento e distribuição

### 16.1 macOS

Artefato inicial:

- `NukaBoost.app` com `Info.plist`, ícones e os dois executáveis;
- `.dmg` para instalação por arrastar;
- binário `arm64` obrigatório;
- binário universal `arm64 + x86_64` apenas quando ambos os alvos forem
  compilados e testados.

O `nukaboostctl` pode ficar em `NukaBoost.app/Contents/MacOS/` e também ser
publicado como artefato separado. A aplicação pode oferecer uma instrução ou
ação optativa para criar um link em um diretório do usuário já presente no
`PATH`. Ela não deve escrever silenciosamente em `/usr/local/bin` nem pedir
elevação apenas para disponibilizar o CLI.

Distribuição pública fora da App Store:

1. Assinar executáveis e bundle com `Developer ID Application`.
2. Ativar hardened runtime.
3. Usar secure timestamp.
4. Enviar com `notarytool`.
5. Aguardar aceitação e anexar o ticket com `stapler`.
6. Verificar com `codesign`, `spctl` e `stapler validate`.

A Apple exige Developer ID, hardened runtime e timestamp para notarização:
[Notarizing macOS software](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
e [Developer ID](https://developer.apple.com/support/developer-id/). A
distribuição reconhecida pelo Gatekeeper exige participação no Apple Developer
Program. Uma assinatura ad hoc ou autocertificada serve para desenvolvimento,
mas não substitui Developer ID em uma release pública.

### 16.2 Linux

Não existe um instalador ou assinatura universal equivalente ao Authenticode.
Fases recomendadas:

1. `.tar.gz` portátil com executáveis, `.desktop`, ícones, licença e README.
2. SHA-256 de todos os artefatos e manifesto assinado.
3. `.deb` para Debian e Ubuntu.
4. `.rpm` para Fedora e openSUSE.
5. Flatpak quando o backend de portal e as limitações de tray estiverem
   validados.
6. AppImage somente se houver demanda e rotina de atualização segura.

Pacotes nativos devem ser assinados pelas ferramentas de cada ecossistema. As
chaves de release não ficam no repositório nem em runners de pull request.

## 17. CI, supply chain e releases

A matriz mínima do GitHub Actions deve conter:

```text
windows-latest: fmt, clippy, testes e MSI
macos-latest: fmt, clippy, testes, .app e artefato não assinado em PR
ubuntu-latest: fmt, clippy, testes e tarball
```

Regras:

- usar `Cargo.lock` versionado;
- negar warnings em código de produção;
- executar auditoria de dependências e licenças;
- fixar Actions por commit ou política de atualização controlada;
- builds de PR nunca acessam certificados ou segredos de assinatura;
- assinatura e notarização rodam somente para tags protegidas;
- gerar SBOM, checksums e proveniência para a release;
- testar os artefatos instalados, não apenas `cargo test`;
- manter um workflow manual de release com ambiente protegido e aprovação.

Cada release publica notas comuns e uma seção por sistema com capacidades e
limitações.

## 18. Segurança e privacidade

- nenhuma telemetria por padrão;
- nenhum comando arbitrário recebido pelo servidor IPC;
- `nukaboostctl run` executa o comando no próprio processo cliente;
- validar comprimento, UTF-8 e schema de toda mensagem;
- aplicar backpressure e limite de conexões locais;
- não registrar argumentos completos quando puderem conter segredos;
- evitar permissões globais em sockets, arquivos e autostart;
- não carregar bibliotecas de diretórios graváveis por outros usuários;
- hardened runtime no macOS sem entitlements desnecessários;
- nenhuma regra PolicyKit ampla ou serviço root no Linux;
- documentar resposta a vulnerabilidades em `SECURITY.md` para todos os alvos.

## 19. Plano de execução

### Fase 0: prova de portabilidade

- fazer `nukaboost-core` compilar sem dependências Windows;
- executar testes do domínio em Windows, macOS e Linux;
- criar backends mínimos que apenas consultam capacidades;
- registrar decisões de bibliotecas FFI e D-Bus.

Saída: CI verde nos três sistemas sem interface ou instaladores completos.

### Fase 1: fronteiras arquiteturais

- criar `nukaboost-platform` e `nukaboost-runtime`;
- mover implementação Windows sem mudar comportamento;
- adicionar mocks de `SleepInhibitor`, paths, startup e IPC;
- manter todos os testes e o MSI atuais funcionando.

Saída: Windows funcional sobre as novas interfaces.

### Fase 2: CLI Unix

- implementar socket Unix seguro e single instance;
- implementar logind no Linux e IOKit no macOS;
- portar `nukaboostctl`, leases e `run`;
- introduzir relatório de capacidades e protocolo v3;
- testar crash, TTL, sinais e concorrência.

Saída: produto utilizável pelo terminal nos dois sistemas.

### Fase 3: interface macOS

- criar bundle AppKit e menu bar;
- integrar i18n, consentimento e autostart;
- produzir `.app` e `.dmg` de teste;
- testar Apple Silicon e, se suportado, Intel;
- documentar claramente a limitação da tampa.

Saída: beta macOS não assinada para teste local.

### Fase 4: interface Linux

- integrar StatusNotifierItem/AppIndicator;
- oferecer comportamento seguro sem tray;
- implementar autostart XDG;
- testar GNOME, KDE Plasma, Wayland e sessão sem GUI;
- validar fallback do portal.

Saída: beta Linux portátil.

### Fase 5: pacotes e assinatura

- automatizar DMG, notarização e verificação do Gatekeeper;
- produzir tarball, DEB e RPM reproduzíveis quando possível;
- assinar manifestos e pacotes Linux;
- gerar checksums, SBOM e proveniência;
- testar atualização, downgrade e remoção completa.

Saída: release candidate assinada.

### Fase 6: estabilização

- executar testes em hardware real;
- fazer revisão de segurança do IPC e da cadeia de release;
- revisar traduções completas do app e CLI;
- atualizar README, política de suporte e solução de problemas;
- publicar primeiro como prerelease e recolher relatórios.

Saída: release estável após critérios de aceitação.

## 20. Matriz de testes obrigatória

### Comuns

- ativar e desativar repetidamente;
- alternar durante ativação em andamento;
- leases concorrentes, TTL e relógio monotônico;
- crash do tray, crash do CLI e logout;
- processo filho encerrado normalmente e por sinal;
- servidor IPC malformado, mensagens grandes e peer inválido;
- idioma inglês e português em todos os comandos;
- tela apaga enquanto a proteção está ativa;
- estado de erro nunca usa ícone ou texto de ativo.

### macOS

- Apple Silicon em hardware real;
- Intel apenas se for alvo publicado;
- repouso por inatividade com tampa aberta;
- repouso explícito continua sob controle do usuário;
- fechar a tampa não é anunciado como protegido;
- bateria baixa e condição térmica não são mascaradas;
- LaunchAgent habilita, desabilita e não duplica instâncias;
- Gatekeeper aceita o DMG notarizado.

### Linux

- Ubuntu LTS com GNOME e Wayland;
- Fedora com GNOME;
- KDE Plasma;
- login1 ausente;
- D-Bus indisponível;
- PolicyKit nega lock de tampa;
- proteção de tampa em notebook real, na tomada e na bateria;
- portal em Flatpak;
- sessão sem tray e sessão sem GUI;
- instalação, upgrade e remoção de DEB e RPM.

## 21. Critérios de aceitação

Uma plataforma só pode ser anunciada como suportada quando:

- o backend usa API pública e passa na matriz de hardware relevante;
- o estado ativo corresponde a handles reais e confirmados;
- a tela pode apagar normalmente;
- o término do processo libera toda proteção;
- CLI, interface e autostart funcionam sem privilégios elevados no cenário
  comum;
- IPC aceita somente o usuário correto;
- todos os textos estão completos em inglês e português;
- artefatos têm checksums e passam nas verificações da plataforma;
- limitações são mostradas na UI, documentação e release notes;
- instalação, atualização e desinstalação não deixam processos ou arquivos
  ativos indevidamente.

## 22. Fora de escopo inicial

- suporte a BSD ou outros Unix;
- daemon de sistema executado como root;
- alteração persistente de `logind.conf`, `pmset` ou políticas globais;
- contorno de MDM, firmware, repouso térmico ou bateria crítica;
- garantia de tampa fechada no macOS;
- suporte completo a todo desktop e toda distribuição Linux na primeira versão;
- Mac App Store antes da versão Developer ID estar estável;
- atualização automática antes de existir assinatura e validação ponta a ponta.

## 23. Decisões que devem permanecer explícitas

Antes da implementação final, registrar em ADRs curtos:

1. biblioteca de D-Bus e estratégia async do Linux;
2. bindings AppKit/IOKit e integração com o event loop do macOS;
3. versão mínima de macOS e baseline de glibc;
4. suporte ou não a Mac Intel;
5. backend gráfico Linux e política para ambientes sem tray;
6. compatibilidade e prazo de remoção do protocolo v2;
7. formato e custódia das chaves de assinatura Linux;
8. política de restauração de leases após reinício do app.

O resultado desejado não é uma cópia do comportamento interno do Windows. É o
mesmo contrato de produto implementado com o mecanismo oficial e a garantia
realmente disponível em cada sistema.

# Plano de implementação — NukaBoost v1

## 1. Objetivo do produto

NukaBoost será um utilitário nativo para Windows 11, escrito em Rust, executado na área de notificações e sem janela principal.

Quando ativado, ele deve:

- Impedir suspensão automática por inatividade.
- Continuar funcionando conectado à tomada ou usando bateria.
- Continuar ativo no modo Economia de Bateria.
- Impedir suspensão ao fechar a tampa.
- Permitir que a tela apague normalmente em qualquer situação.
- Manter compilações, downloads e processos em execução.
- Expor controle por interface gráfica mínima e por CLI.
- Restaurar todas as configurações originais ao ser desativado, encerrado ou desinstalado.

## 2. Limites obrigatórios e honestidade do estado

“NukaBoost ativo” só pode ser exibido depois que todas as proteções necessárias forem aplicadas e verificadas por leitura posterior.

A garantia deve ser definida assim:

> Enquanto o Windows estiver executando normalmente, a bateria não estiver em estado crítico, nenhuma política administrativa bloquear o aplicativo e o hardware aceitar as configurações, o NukaBoost impede suspensão por inatividade e pelo fechamento da tampa.

O programa não pode garantir funcionamento após:

- Esgotamento físico da bateria.
- Estado de bateria crítica definido pelo firmware.
- Desligamento térmico.
- Desligamento ou reinicialização do Windows.
- Suspensão/hibernação explicitamente solicitada pelo usuário.
- Atualização do Windows que force reinicialização.
- Política de grupo/MDM que proíba alterar o plano de energia.
- Firmware que trate o fechamento da tampa fora do controle do Windows.

Não desativar a ação de **bateria crítica**. Deixar o notebook consumir até 0% produziria perda abrupta de energia e risco de corrupção de dados. O NukaBoost pode ignorar a ação de “bateria baixa”, mas deve preservar a proteção de “bateria crítica”. O Windows diferencia esses dois estados. [Configurações de bateria](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/battery-settings)

## 3. Comportamento visual

### Inicialização

Ao abrir normalmente ou pelo Startup:

1. Adquirir o guard de instância única.
2. Executar recuperação de uma sessão anterior incompleta sob esse guard.
3. Iniciar sempre no estado `Inactive`.
4. Adicionar o ícone inativo à área de notificações.
5. Nunca restaurar automaticamente o estado ativo anterior.
6. Exibir o aviso de segurança, exceto se “não mostrar novamente” estiver salvo.

A ativação deve ser sempre consequência de:

- Clique esquerdo no ícone.
- Item `Start/Iniciar`.
- Comando explícito pelo CLI.
- Lease solicitado por um agente.

### Estados e ícones

Embora tenham sido solicitados dois ícones, recomenda-se um terceiro estado para evitar informação falsa:

- `Inactive`: ícone cinza/apagado.
- `Active`: ícone colorido, por exemplo verde ou âmbar.
- `Error/Degraded`: ícone vermelho com exclamação.

Tooltips:

- `NukaBoost — Inactive`
- `NukaBoost — Active`
- `NukaBoost — Protection error`

Em português:

- `NukaBoost — Desativado`
- `NukaBoost — Ativo`
- `NukaBoost — Erro de proteção`

### Limitação da área de notificações

O Windows não permite que um programa tire automaticamente seu ícone do menu `^` e o fixe ao lado dele. Essa escolha pertence ao usuário. O aplicativo pode emitir uma notificação que torne o ícone temporariamente visível, mas não pode fixá-lo usando APIs oficiais. [Diretrizes oficiais da área de notificações](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)

O NukaBoost deve:

- Emitir uma notificação ao ativar/desativar.
- Informar no About como o usuário pode fixar o ícone manualmente.
- Nunca modificar configurações internas do Explorer ou registro para forçar isso.

### Menu de contexto

Menu em inglês:

```text
Start / Stop
Language
    ● EN — English
    ○ PT — Português
About
────────────
Exit
```

Menu em português:

```text
Iniciar / Parar
Idioma
    ● EN — English
    ○ PT — Português
Sobre
────────────
Fechar
```

Regras:

- Clique esquerdo alterna entre ativo e inativo.
- Clique direito abre o menu.
- `Stop/Parar` desativa, restaura tudo e cancela leases existentes.
- `Exit/Fechar` primeiro desativa e restaura tudo, depois encerra.
- Se a restauração falhar, não encerrar silenciosamente: mostrar erro e conservar o arquivo de recuperação.
- Ativação por teclado no ícone deve funcionar.

### About

Janela nativa pequena, não redimensionável, sem aparecer como janela permanente na barra de tarefas.

Conteúdo:

- Ícone e nome NukaBoost.
- Versão.
- Estado atual.
- Descrição localizada.
- Informação clara: “NukaBoost nunca mantém a tela ligada”.
- Aviso sobre ventilação.
- Link opcional para repositório/licença.
- Botão `Close/Fechar`.

Implementar com Win32 nativo e recursos `.rc`, evitando frameworks grandes apenas para essa janela.

## 4. Aviso de segurança

Exibir na inicialização, antes de permitir ativação:

> **EN:** NukaBoost can keep your laptop running with the lid closed. Never place it inside a bag, sleeve, or poorly ventilated space while active. Heavy workloads can cause excessive heat, battery drain, or thermal shutdown.
>
> **PT:** O NukaBoost pode manter o notebook funcionando com a tampa fechada. Nunca coloque o notebook dentro de uma mochila, capa ou espaço sem ventilação enquanto ele estiver ativo. Cargas intensas podem causar aquecimento excessivo, consumo da bateria ou desligamento térmico.

Controles:

```text
[ ] Do not show this warning again
    Não mostrar este aviso novamente

                         [ OK ]
```

Regras:

- O texto do aviso é sempre bilíngue.
- O restante da interface segue o idioma escolhido.
- Enquanto o aviso não for confirmado, o estado permanece inativo.
- Se o CLI tentar ativar enquanto o aviso aguarda confirmação, retornar `safety_ack_required`.
- Não oferecer parâmetro silencioso para ignorar o primeiro aviso.

## 5. Arquitetura recomendada

Usar um workspace Rust:

```text
nukaboost/
├── Cargo.toml
├── crates/
│   ├── nukaboost-core/
│   │   ├── power/
│   │   ├── state/
│   │   ├── recovery/
│   │   ├── ipc/
│   │   ├── config/
│   │   └── i18n/
│   └── nukaboost-win32/
│       ├── tray/
│       ├── dialogs/
│       └── resources/
├── apps/
│   ├── nukaboost/
│   │   └── NukaBoost.exe
│   └── nukaboostctl/
│       └── nukaboostctl.exe
├── assets/
│   ├── inactive.ico
│   ├── active.ico
│   └── error.ico
├── installer/
│   └── NukaBoost.wxs
├── skill/
│   └── manage-nukaboost/
│       ├── SKILL.md
│       └── agents/openai.yaml
└── tests/
```

Dois executáveis são preferíveis:

- `NukaBoost.exe`: subsistema Windows, sem console.
- `nukaboostctl.exe`: subsistema Console, adequado para terminal e agentes.

O watchdog pode reutilizar `NukaBoost.exe --watchdog ...`, sem um terceiro binário.

## 6. Estratégia de energia

Não alterar diretamente o plano original. Criar um plano temporário derivado do atual:

1. Consultar o plano ativo com `PowerGetActiveScheme`.
2. Duplicá-lo com `PowerDuplicateScheme`.
3. Nomeá-lo `NukaBoost Temporary`.
4. Alterar somente as configurações necessárias.
5. Ativar o plano temporário.
6. Ao parar, reativar o plano original e excluir o temporário.

O Windows oferece APIs oficiais para duplicar, ativar e excluir planos. [Gerenciamento de planos de energia](https://learn.microsoft.com/en-us/windows/win32/power/managing-power-schemes)

### Configurações do plano temporário

Aplicar e verificar tanto para AC quanto DC:

| Configuração | Valor |
|---|---:|
| `SUB_BUTTONS / LIDACTION` | `0` — não fazer nada |
| `SUB_SLEEP / STANDBYIDLE` | `0` — nunca suspender por inatividade |
| `SUB_SLEEP / SYSTEMREQUIRED` | `1` — aceitar solicitações de aplicativos |
| `SUB_BATTERY / BATACTIONLOW` | `0` — não suspender em bateria baixa |

Constantes:

```text
SUB_BUTTONS
4f971e89-eebd-4455-a8de-9e59040e7347

LIDACTION
5ca83367-6e45-459f-a27b-476b1d01c936

SUB_SLEEP
238c9fa8-0aad-41ed-83f4-97be242c8f20

STANDBYIDLE
29f6c1db-86da-48c5-9fdb-f2b67b1f44da

SYSTEMREQUIRED
a4b195f5-8225-47d8-8012-9d41369786e2

SUB_BATTERY
e73a048d-bf27-4f12-9731-8b2076e8891f

BATACTIONLOW
d8742dcb-3e6a-4b3c-b3fe-374623cdcf06
```

Não alterar:

- Timeout da tela.
- `PowerRequestDisplayRequired`.
- `ES_DISPLAY_REQUIRED`.
- Ação ou limite de bateria crítica.
- Botão físico de energia.
- Parâmetros de CPU/desempenho.
- Hibernação adaptativa do firmware.
- Configurações térmicas.

O valor `STANDBYIDLE = 0` significa “nunca suspender por inatividade”. [Documentação do timeout](https://learn.microsoft.com/en-gb/windows-hardware/customize/power-settings/sleep-settings-sleep-idle-timeout)

### Solicitações redundantes de execução

Além do plano temporário, manter:

- `PowerRequestSystemRequired`
- `PowerRequestExecutionRequired`
- Uma thread dedicada com:

```rust
SetThreadExecutionState(
    ES_CONTINUOUS | ES_SYSTEM_REQUIRED
);
```

Na desativação, a mesma thread chama:

```rust
SetThreadExecutionState(ES_CONTINUOUS);
```

A redundância é intencional:

- O plano remove os gatilhos de suspensão.
- `PowerRequestSystemRequired` informa formalmente ao Windows que há trabalho ativo.
- `ExecutionRequired` ajuda a manter o processo executável.
- `SetThreadExecutionState` oferece uma segunda via oficial.

Não usar:

- Movimento falso de mouse.
- Teclas sintéticas.
- Timers que simulam presença.
- Alteração bruta do registro.
- `AwayModeRequired`.
- APIs não documentadas.

## 7. Transação de ativação

A ativação deve ser atômica:

```text
Inactive
  → Activating
  → Active
```

Procedimento:

1. Obter mutex interno de mudança de estado.
2. Consultar `PowerSettingAccessCheck`.
3. Ler o plano original.
4. Duplicar o plano.
5. Configurar o plano temporário.
6. Ler todos os valores novamente e confirmar.
7. Escrever e sincronizar o journal de recuperação.
8. Registrar recuperação no próximo logon.
9. Iniciar o watchdog.
10. Ativar o plano temporário.
11. Confirmar que ele é realmente o plano ativo.
12. Criar as duas Power Requests.
13. Iniciar a thread de `SetThreadExecutionState`.
14. Iniciar monitoramento.
15. Somente então mudar para `Active`.
16. Atualizar ícone, tooltip e notificação.

Se qualquer etapa falhar:

1. Limpar requests já criadas.
2. Restaurar o plano original.
3. Excluir o plano temporário.
4. Preservar journal se a restauração não for confirmada.
5. Entrar em `Error`, nunca em `Active`.

## 8. Desativação

Ordem recomendada:

1. Entrar em `Deactivating`.
2. Manter as Power Requests durante a restauração.
3. Parar o monitor.
4. Reativar o plano original.
5. Confirmar por leitura que ele está ativo.
6. Excluir o plano temporário.
7. Encerrar a thread de execution state.
8. Limpar `ExecutionRequired`.
9. Limpar `SystemRequired`.
10. Desarmar o watchdog.
11. Remover recuperação de próximo logon.
12. Apagar o journal.
13. Entrar em `Inactive`.
14. Atualizar ícone e notificação.

## 9. Recuperação contra falhas

### Journal

Arquivo:

```text
%LocalAppData%\NukaBoost\recovery.json
```

Conteúdo mínimo:

```json
{
  "schema_version": 1,
  "session_id": "uuid",
  "main_pid": 1234,
  "phase": "active",
  "original_scheme_guid": "...",
  "temporary_scheme_guid": "...",
  "created_at_utc": "..."
}
```

Gravar atomicamente:

1. Criar arquivo temporário.
2. Executar flush.
3. Renomear/substituir atomicamente.
4. Só então ativar o plano temporário.

### Watchdog

Quando o NukaBoost é ativado:

```text
NukaBoost.exe --watchdog <pid> <session-id>
```

O watchdog deve:

- Abrir o handle do processo principal.
- Esperar seu encerramento.
- Verificar se a sessão foi desarmada corretamente.
- Se o processo morreu inesperadamente:
    - Restaurar o plano original.
    - Excluir o plano temporário.
    - Manter logs da recuperação.

Evitar depender apenas de PID; validar também `session-id` e horário de criação para impedir problemas com reutilização de PID.

### Reinicialização e queda de energia

Enquanto ativo, registrar uma entrada de recuperação `RunOnce`, separada do Startup normal:

```text
NukaBoost.exe --recover-only <session-id>
```

Assim:

- Se o computador for desligado abruptamente, a restauração ocorre no próximo logon.
- `--recover-only` nunca inicia o modo ativo.
- Após recuperar, o processo encerra sem criar ícone.

Também tratar:

- `WM_QUERYENDSESSION`
- `WM_ENDSESSION`
- Encerramento normal
- Logoff
- Reinicialização

## 10. Monitoramento durante o estado ativo

Registrar notificações de energia e também fazer uma verificação periódica, por exemplo a cada cinco segundos.

Verificar:

- Plano temporário continua ativo.
- `LIDACTION` permanece `0` em AC e DC.
- `STANDBYIDLE` permanece `0`.
- `SYSTEMREQUIRED` permanece `1`.
- `BATACTIONLOW` permanece `0`.
- Handles das Power Requests continuam válidos.
- Thread de execution state continua viva.
- Fonte mudou entre tomada e bateria.
- Economia de bateria foi ativada.

Se houver alteração externa:

1. Tentar reaplicar uma vez.
2. Confirmar por leitura.
3. Se continuar falhando, desativar com rollback e entrar em `Error`.
4. Nunca deixar o ícone verde com proteção incompleta.

Não entrar em loop brigando indefinidamente com Group Policy.

## 11. Comunicação com terminal

Usar Named Pipe local:

```text
\\.\pipe\NukaBoost-{user-sid}
```

Aplicar DACL permitindo apenas:

- Usuário atual.
- Opcionalmente administradores locais.
- Negar acesso remoto/network logon.

Named Pipes possuem controle de acesso próprio; não usar o descritor padrão, que pode ser permissivo demais. [Segurança de Named Pipes](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)

Protocolo:

- Mensagens JSON delimitadas por tamanho.
- `protocol_version`.
- `request_id`.
- Tamanho máximo pequeno, por exemplo 64 KB.
- Nenhum comando arbitrário de shell.

### Comandos do CLI

```powershell
nukaboostctl status
nukaboostctl status --json

nukaboostctl start
nukaboostctl stop
nukaboostctl toggle

nukaboostctl language en
nukaboostctl language pt

nukaboostctl startup enable
nukaboostctl startup disable
nukaboostctl startup status

nukaboostctl exit
```

Se `start` for executado sem processo principal:

1. Iniciar `NukaBoost.exe`.
2. Esperar o Named Pipe.
3. Solicitar ativação.
4. Falhar se o aviso de segurança ainda não estiver confirmado.

### JSON de status

Exemplo:

```json
{
  "protocol_version": 2,
  "app_version": "1.0.0",
  "process_running": true,
  "state": "active",
  "language": "en",
  "manual_hold": false,
  "leases": 1,
  "power_source": "battery",
  "battery_percent": 42,
  "battery_saver": true,
  "protections": {
    "temporary_scheme_active": true,
    "lid_action_ac": true,
    "lid_action_dc": true,
    "idle_sleep_ac": true,
    "idle_sleep_dc": true,
    "system_required_ac": true,
    "system_required_dc": true,
    "low_battery_action": true,
    "system_power_request": true,
    "execution_power_request": true,
    "thread_execution_state": true,
    "display_request_absent": true
  },
  "last_error": null
}
```

Chaves e valores do JSON nunca devem ser localizados.

## 12. Leases para agentes

Não orientar agentes a simplesmente executar `start` e `stop`. Isso pode fazer um agente desligar o NukaBoost enquanto outro processo ainda depende dele.

Adicionar leases:

```powershell
nukaboostctl acquire `
  --reason "cargo build --release" `
  --owner "codex" `
  --ttl 2h `
  --json

nukaboostctl release <lease-id>
```

O estado efetivo é ativo quando:

```text
manual_hold == true OR active_leases > 0
```

Adicionar também o comando mais seguro:

```powershell
nukaboostctl run -- cargo build --release
```

Esse comando deve:

1. Adquirir uma lease.
2. Confirmar `state == active`.
3. Executar o processo filho.
4. Manter a lease enquanto ele existir.
5. Liberar a lease mesmo se receber Ctrl+C.
6. Retornar o mesmo exit code do processo filho.

Regras:

- Toda lease deve ter TTL.
- `release` libera somente a lease informada.
- O clique manual em `Stop` tem prioridade humana e pode cancelar todas as leases.
- `stop --force` existe para emergência, mas não deve ser usado pela skill.
- Leases não são restauradas depois de crash/reboot.

## 13. Skill para Codex e outros agentes

Criar uma skill curta chamada `manage-nukaboost`.

Fluxo obrigatório:

1. Executar `nukaboostctl status --json`.
2. Preferir `nukaboostctl run -- <comando>` para uma única operação.
3. Para tarefas com várias etapas, adquirir uma lease com TTL.
4. Confirmar que `state` é `active`.
5. Confirmar todas as proteções obrigatórias.
6. Executar a tarefa.
7. Liberar somente a lease criada pelo próprio agente em uma etapa equivalente a `finally`.
8. Confirmar a liberação.
9. Nunca desativar um `manual_hold` do usuário.
10. Se a ativação falhar, não iniciar a tarefa crítica.

Estrutura:

```text
manage-nukaboost/
├── SKILL.md
└── agents/
    └── openai.yaml
```

Não são necessários scripts dentro da skill; `nukaboostctl` já é a interface determinística. Para Claude Code, fornecer a mesma sequência no formato de skill/configuração suportado por ele. O protocolo CLI permanece independente do agente.

Exemplo de gatilho:

> Use quando uma compilação, renderização, download, migração ou teste prolongado não puder ser interrompido pela suspensão do Windows.

## 14. Inicialização com o Windows

Oferecer duas maneiras:

- Opção no instalador: `Start NukaBoost with Windows`.
- CLI: `nukaboostctl startup enable|disable`.

Usar um atalho em `FOLDERID_Startup` com:

```text
NukaBoost.exe --startup
```

Regras de `--startup`:

- Adquirir a instância única e fazer a recuperação antes de criar a interface.
- Iniciar inativo.
- Nunca carregar estado ativo anterior.
- Exibir aviso de segurança se não estiver suprimido.
- Não exibir janela principal.
- Não ativar por causa de leases antigas.

Deixar o Startup desabilitado por padrão no instalador é a opção mais respeitosa ao usuário.

## 15. Instalação e desinstalação

Usar MSI com WiX Toolset.

Instalação por usuário:

```text
%LocalAppData%\Programs\NukaBoost\
```

O MSI deve registrar:

- Nome do produto.
- Versão.
- Publicador.
- Ícone.
- Local de instalação.
- Comando de desinstalação.
- URL opcional.
- Atalho no menu Iniciar.
- Entrada em `Configurações → Aplicativos → Aplicativos instalados`.

O Windows Installer possui suporte próprio para registrar programas em Adicionar/Remover Programas. [Documentação MSI](https://learn.microsoft.com/en-us/windows/win32/msi/configuring-add-remove-programs-with-windows-installer)

### Desinstalação

Antes de remover arquivos:

1. Enviar comando de desativação.
2. Cancelar leases.
3. Restaurar o plano original.
4. Confirmar a restauração.
5. Encerrar o processo principal e watchdog.
6. Excluir qualquer plano temporário do NukaBoost.
7. Remover Startup e RunOnce.
8. Remover arquivos de configuração, logs e journal.
9. Remover executáveis.
10. Remover entrada do PATH, caso adicionada.

Se a restauração falhar, o desinstalador deve:

- Tentar `NukaBoost.exe --recover-only`.
- Não apagar o recovery journal.
- Exibir erro claro.
- Preferencialmente abortar a remoção antes de deixar o sistema em estado desconhecido.

Upgrades devem executar o mesmo cleanup antes de substituir binários.

Assinar os executáveis e o MSI com Authenticode antes de uma distribuição pública.

## 16. Crates e tecnologias

Sugestão:

- `windows` ou `windows-sys`: Win32, energia, shell e IPC.
- `serde` e `serde_json`: configuração, journal e protocolo.
- `thiserror`: erros internos.
- `tracing`: logs.
- `lexopt` ou `clap`: CLI.
- `embed-resource`: manifest, ícones, version info e dialogs.
- `uuid`: IDs de sessão e leases.

Evitar no processo principal:

- Electron.
- WebView.
- Tokio, salvo necessidade real.
- Framework de interface multiplataforma.
- Bibliotecas de tray que tragam uma event loop pesada.

Compilar para `x86_64-pc-windows-msvc`. ARM64 pode ser uma segunda etapa.

## 17. Requisitos de qualidade

### Logs

Local:

```text
%LocalAppData%\NukaBoost\logs\
```

Registrar:

- Ativação/desativação.
- Plano original e temporário.
- Resultado das APIs.
- Mudanças de energia.
- Ações do watchdog.
- Recuperações.
- Comandos IPC, sem dados sensíveis.
- Motivo de erro.

Rotacionar arquivos e limitar o tamanho.

### Segurança

- Nenhum comando IPC que execute shell.
- Named Pipe restrito ao usuário.
- Arquivos de estado acessíveis somente pelo usuário.
- Nenhuma execução elevada permanente.
- Não rodar o tray como administrador.
- Se o dispositivo exigir elevação para alterar energia, reportar incompatibilidade inicialmente; um helper privilegiado deve ser considerado apenas em uma versão posterior.
- Validar limites de todas as mensagens e strings.

### Explorer

Registrar e tratar a mensagem `TaskbarCreated`. Se o Explorer reiniciar, recriar o ícone automaticamente.

Usar um GUID estável para o ícone via `Shell_NotifyIconW`.

## 18. Plano de execução

### Fase 0 — Prova de viabilidade no notebook real

Antes de escrever toda a UI:

- Detectar S3 versus Modern Standby com `powercfg /a`.
- Confirmar que o usuário consegue duplicar e ativar um plano sem elevação.
- Testar `LIDACTION = 0` em AC e DC.
- Testar `STANDBYIDLE = 0`.
- Testar bateria baixa sem alterar bateria crítica.
- Testar leitura posterior de todos os valores.
- Fechar a tampa e confirmar que uma compilação continua.
- Confirmar que a tela interna desliga.
- Confirmar que Group Policy não bloqueia alterações.

Critério: não avançar para a UI antes do núcleo funcionar no equipamento-alvo.

### Fase 1 — Core de energia

- Wrapper seguro para APIs Win32.
- Plano temporário.
- Power Requests.
- Thread execution state.
- Ativação e rollback transacionais.
- Testes unitários dos estados.

### Fase 2 — Recuperação

- Journal atômico.
- Watchdog.
- RunOnce.
- Recuperação no startup.
- Testes com `taskkill /F`.

### Fase 3 — Área de notificações

- Hidden message window.
- Ícones.
- Clique esquerdo.
- Menu.
- Notificações.
- Recuperação após reinício do Explorer.

### Fase 4 — Idiomas e dialogs

- EN padrão.
- PT alternável.
- Warning bilíngue.
- About.
- Persistência da preferência.

### Fase 5 — IPC e CLI

- Named Pipe seguro.
- `status`, `start`, `stop`, `toggle`, `exit`.
- Leases.
- `run --`.
- JSON e exit codes estáveis.

### Fase 6 — Instalador

- MSI WiX.
- Installed Apps.
- Startup opcional.
- Upgrade.
- Desinstalação segura.
- Assinatura.

### Fase 7 — Skill

- Criar `manage-nukaboost`.
- Validar a skill.
- Testar com uma compilação real.
- Confirmar liberação da lease em sucesso, erro e cancelamento.

## 19. Critérios de aceite

O projeto só está concluído quando estes testes passarem:

- Sempre inicia inativo, inclusive no Startup.
- Clique esquerdo ativa e desativa.
- Ícone e tooltip refletem o estado real.
- A tela apaga no timeout original enquanto ativo.
- Nenhuma API de display é solicitada.
- Não suspende após duas vezes o timeout normal de suspensão.
- Funciona em AC e DC.
- Continua ao conectar ou remover o carregador.
- Continua com Economia de Bateria ativada.
- Fechar a tampa mantém uma compilação executando.
- Bateria baixa não suspende.
- Bateria crítica continua protegida pelo Windows.
- `powercfg /requests` mostra as solicitações do NukaBoost.
- Matar o processo principal aciona restauração pelo watchdog.
- Matar processo e watchdog deixa recuperação para o próximo logon.
- Reiniciar o Explorer não remove permanentemente o ícone.
- `nukaboostctl run -- cargo build` ativa, executa e desativa corretamente.
- Duas leases simultâneas não interferem uma na outra.
- Desinstalar enquanto ativo restaura tudo.
- Após desinstalação não sobra plano temporário, Startup ou RunOnce.
- Política administrativa negada resulta em erro, não em falso estado ativo.

Essa arquitetura usa múltiplas camadas oficiais, mantém o plano original intacto e dá aos agentes uma interface segura. O principal refinamento em relação à ideia inicial é o plano temporário mais watchdog e leases: isso resolve tanto a recuperação de falhas quanto a concorrência entre usuário, Codex e Claude.

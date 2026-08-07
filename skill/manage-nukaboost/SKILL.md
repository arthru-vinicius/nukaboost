---
name: manage-nukaboost
description: Use quando uma compilação, renderização, download, migração ou teste prolongado não puder ser interrompido pela suspensão do Windows. Adquire uma lease do NukaBoost antes da tarefa e a libera ao final, sem nunca desativar a retenção manual do usuário.
---

# manage-nukaboost

Gerencia leases do NukaBoost para manter o Windows acordado durante uma
tarefa longa (seção 13 do plano do produto), usando exclusivamente
`nukaboostctl` — não há scripts nesta skill porque `nukaboostctl` já é a
interface determinística e com saída JSON estável.

## Quando usar

Sempre que a próxima ação for uma compilação, renderização, download,
migração de dados ou teste prolongado que não deva ser interrompido por
suspensão automática do Windows. Não use para tarefas rápidas (segundos) —
o custo de gerenciar uma lease não compensa.

## Regras invioláveis

- **Nunca** execute `nukaboostctl stop` ou `nukaboostctl stop --force`.
  `stop --force` cancela leases de **todos** os donos, não só a sua, e
  pode desligar uma proteção da qual outro processo (o usuário, outro
  agente) ainda depende. A prioridade de parar o NukaBoost é sempre humana
  — clique manual no ícone ou `Stop`/`Parar` no menu.
- **Nunca** desative a retenção manual do usuário (`manual_hold`). Você só
  controla a lease que você mesmo criou.
- Sempre libere exatamente a lease que você adquiriu, mesmo se a tarefa
  falhar ou for cancelada — trate a liberação como um bloco `finally`.
- Se a ativação falhar (por exemplo, `safety_ack_required` ou
  `policy_denied`), **não inicie a tarefa crítica**. Prosseguir sem a
  proteção confirmada é pior do que não ter tentado.

## Procedimento

1. Consultar o estado atual:

   ```powershell
   nukaboostctl status --json
   ```

2. **Para uma única operação simples**, prefira o comando composto, que já
   implementa os passos 3–8 abaixo de forma atômica:

   ```powershell
   nukaboostctl run -- cargo build --release
   ```

   `run --` adquire uma lease, confirma que o estado é `active`, executa o
   comando informado, e libera a lease ao final — inclusive se o comando
   falhar ou receber `Ctrl+C`. O código de saída retornado é o mesmo do
   comando filho. Use isso sempre que possível; os passos manuais abaixo
   só são necessários para orquestrar **várias** etapas em torno da mesma
   lease.

3. **Para uma tarefa com várias etapas**, adquira uma lease com TTL
   explícito (nunca omita `--ttl`; superestime moderadamente a duração
   esperada da tarefa):

   ```powershell
   nukaboostctl acquire --reason "cargo build --release" --owner "claude" --ttl 2h --json
   ```

   A resposta traz `lease_id` — guarde-o, ele é necessário para liberar a
   lease no passo 7.

4. Confirmar que `state` é `"active"` na resposta de `status --json` (ou
   que `acquire` não retornou erro — a ativação é automática ao adquirir
   uma lease). Se não estiver ativo, pare aqui.

5. Confirmar que `protections` não contém nenhum valor `false` relevante
   (equivalente a `protections.all_ok()` internamente). Se alguma proteção
   estiver incompleta, trate como falha de ativação — não prossiga.

6. Executar a tarefa normalmente.

7. Liberar **somente** a lease criada neste procedimento, em uma etapa
   equivalente a `finally` (sempre executada, mesmo em caso de erro):

   ```powershell
   nukaboostctl release <lease-id>
   ```

8. Confirmar a liberação: a resposta de `release` deve indicar sucesso
   (`lease_released`). Se falhar, registre o problema — não tente
   compensar chamando `stop`.

## Referência rápida de comandos

```powershell
nukaboostctl status --json                 # estado + proteções, JSON estável
nukaboostctl run -- <comando>               # lease + execução + liberação, atômico
nukaboostctl acquire --reason R --owner O --ttl 2h --json
nukaboostctl release <lease-id>
```

Chaves e valores de `status --json` nunca são localizados (sempre em
inglês), mesmo que a interface gráfica esteja em português — é seguro
tomar decisões programáticas sobre eles independentemente do idioma
configurado pelo usuário.

## Códigos de erro relevantes

| `code` | Significado | Ação do agente |
|---|---|---|
| `safety_ack_required` | O usuário ainda não confirmou o aviso de segurança inicial. | Não prosseguir; informar o usuário que ele precisa abrir o NukaBoost uma vez e confirmar o aviso. |
| `policy_denied` | Uma política administrativa (Group Policy/MDM) bloqueia a alteração do plano de energia. | Não prosseguir; a máquina não pode oferecer a garantia. |
| `protection_not_confirmed` | Uma proteção foi aplicada mas não confirmada por releitura. | Não prosseguir; tratar como falha de ativação. |
| `lease_not_found` | A lease informada em `release` já não existe (expirou ou foi liberada). | Não é necessariamente um erro fatal — a lease já não está em vigor; prosseguir sem repetir a liberação. |

## Por que isto existe

A alternativa ingênua — orientar um agente a rodar `nukaboostctl start`
antes e `nukaboostctl stop` depois — quebra assim que dois processos
concorrentes dependem do NukaBoost ao mesmo tempo: o segundo agente a
terminar chamaria `stop` e desligaria a proteção enquanto o primeiro ainda
precisa dela. Leases com TTL resolvem tanto a concorrência entre
usuário/Codex/Claude quanto a recuperação de falhas (uma lease esquecida
expira sozinha; nada fica preso ativado para sempre).

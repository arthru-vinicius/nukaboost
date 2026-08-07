---
name: manage-nukaboost
description: Mantém o Windows acordado durante tarefas longas usando uma lease temporária do NukaBoost.
---

# NukaBoost para Claude

Localize `nukaboostctl.exe` no `PATH`. Se o processo tiver sido iniciado antes
da instalação, use `%LocalAppData%\Programs\NukaBoost\nukaboostctl.exe`.

Antes de uma compilação, renderização, migração, download ou teste longo,
prefira envolver um único comando assim:

```powershell
nukaboostctl run -- <comando>
```

Esse comando adquire uma lease, confirma todas as proteções, executa o filho,
trata Ctrl+C e libera a lease ao final. Para várias etapas, use:

```powershell
nukaboostctl acquire --reason "<motivo>" --owner "claude" --ttl 2h --json
nukaboostctl status --json
# executar etapas
nukaboostctl release <lease-id>
```

Regras:

- nunca use `stop` ou `stop --force`;
- libere somente a lease que você adquiriu, em comportamento equivalente a
  `finally`;
- confirme código de saída zero em `release` e consulte o status novamente;
- não prossiga se o estado não for `active` ou qualquer proteção for `false`;
- não contorne `safety_ack_required`, `policy_denied` ou
  `protection_not_confirmed`;
- nunca tente alterar `manual_hold`.

Consulte o procedimento completo em `../SKILL.md`.

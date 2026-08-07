# Privacidade

O NukaBoost não possui telemetria, anúncios, conta de usuário nem comunicação
de rede própria. O aplicativo lê apenas informações locais necessárias para
operar: plano de energia, fonte de alimentação, nível de bateria, idioma e
estado de seus próprios processos.

Os seguintes dados ficam no perfil local do usuário, em
`%LocalAppData%\NukaBoost`:

- `config.json`: idioma e confirmação do aviso de segurança;
- `recovery.json`: existe somente durante uma sessão ativa ou recuperação
  pendente;
- `logs\`: diagnósticos técnicos, retidos por até 14 arquivos e limitados a
  aproximadamente 20 MiB.

A desinstalação remove esses dados. O sistema operacional, o GitHub e o método
de distribuição escolhido podem manter seus próprios registros, sujeitos às
políticas deles.

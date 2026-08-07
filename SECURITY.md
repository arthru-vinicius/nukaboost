# Política de segurança

## Versões suportadas

Até a publicação da primeira release estável, apenas o código da branch
`main` recebe correções de segurança.

## Reportando uma vulnerabilidade

Não abra uma issue pública com detalhes exploráveis. Use **Security → Report a
vulnerability** no repositório GitHub para criar um advisory privado. Se o
botão ainda não estiver disponível, abra apenas uma issue curta pedindo um
canal privado, sem revelar a vulnerabilidade. Inclua no relato privado:

- versão/commit afetado;
- impacto e cenário de ameaça;
- passos mínimos para reprodução;
- logs ou prova de conceito sem dados pessoais;
- sugestão de correção, se houver.

O recebimento será confirmado assim que possível. A correção e a divulgação
serão coordenadas antes que o advisory se torne público.

## Escopo importante

São especialmente relevantes falhas que permitam a outro usuário controlar o
Named Pipe, impedir a restauração do plano original, apagar ou forjar o
journal, executar comandos pelo instalador ou contornar a confirmação de
segurança.

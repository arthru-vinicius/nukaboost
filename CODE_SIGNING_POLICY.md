# Política de assinatura de código

## Estado atual

O NukaBoost ainda não foi aceito por um provedor público de assinatura. Os
artefatos produzidos pelo CI e pelo helper local sem thumbprint são
explicitamente não assinados e não devem ser promovidos como release estável.

O provedor gratuito pretendido é a SignPath Foundation. Depois da aprovação,
esta seção será atualizada com a identificação do projeto e a declaração
exigida pelo provedor antes de qualquer binário ser publicado como assinado.

## Escopo

Somente estes artefatos construídos a partir deste repositório podem receber a
assinatura do projeto:

- `NukaBoost.exe`;
- `nukaboostctl.exe`;
- `NukaBoost.msi`, depois de incorporar os dois executáveis assinados.

Não são assinados forks, binários de terceiros, builds locais arbitrários,
pull requests não mesclados ou artefatos cujo commit/tag não possa ser
rastreado.

## Processo

1. O código é validado pelo workflow de CI no Windows.
2. A versão vem de uma tag do repositório e todos os metadados de arquivo/MSI
   devem corresponder a ela.
3. Os executáveis são assinados antes da criação do MSI; o MSI é assinado por
   último.
4. Cada pedido de assinatura requer aprovação humana explícita.
5. Assinaturas e hashes SHA-256 são verificados antes de anexar os arquivos a
   uma release draft.
6. Os bytes assinados nunca são modificados nem substituídos após publicação.

## Responsabilidades

- Committer e reviewer: [@arthru-vinicius](https://github.com/arthru-vinicius)
- Approver de assinatura: [@arthru-vinicius](https://github.com/arthru-vinicius)

Contribuições de pessoas sem acesso direto ao repositório exigem revisão antes
do merge. A conta do repositório e a conta do provedor de assinatura devem usar
autenticação multifator.

## Privacidade e mudanças no sistema

O comportamento de energia é anunciado no aviso inicial, no README e no
instalador. Há desinstalação automatizada com tentativa obrigatória de
restauração. Conforme a [política de privacidade](PRIVACY.md), o programa não
transfere informações para outros sistemas em rede.

Suspeitas de comprometimento da cadeia de build ou uso indevido da assinatura
seguem [SECURITY.md](SECURITY.md) e podem exigir suspensão da publicação,
revogação e divulgação coordenada.

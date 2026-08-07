# Changelog

Todas as mudanças relevantes deste projeto serão documentadas aqui. O formato
segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e as versões
seguem [Semantic Versioning](https://semver.org/lang/pt-BR/).

## [Unreleased]

### Added

- CI para formatação, testes, Clippy, release x64 e validação MSI.
- Documentação de release, assinatura, segurança e privacidade.
- Limite, validação, expiração periódica e reconciliação automática de leases.
- Verificação real dos handles de Power Request e da thread de execution state.
- Retenção limitada de logs e helper reproduzível de release.
- Aviso de segurança e licença MIT próprios no fluxo do instalador.

### Changed

- Recuperação inicial agora ocorre somente depois de adquirir a instância única.
- Rollback preserva watchdog, RunOnce e journal até confirmar a restauração.
- CLI `run` confirma proteções, trata Ctrl+C e preserva o exit code do filho.
- Protocolo IPC v2 inclui os estados `SYSTEMREQUIRED` de AC e DC.
- MSI usa bind paths, arquitetura x64, mostra a pasta de instalação e mantém a
  seleção interativa de Startup visível por padrão.
- Ícones permanentes do executável e dos atalhos usam `active.ico`; somente o
  ícone da área de notificação representa o estado operacional.
- Diálogo Sobre foi simplificado e não inclui instruções de configuração da
  área de notificação do Windows.
- Textos visíveis usam pontuação comum e não empregam travessões como
  separadores artificiais.
- Versão do produto avançou para 1.0.1; o MSI detecta upgrades e rebuilds da
  mesma versão, além de disponibilizar o CLI no `PATH` do usuário.

### Fixed

- Segunda inicialização não desmonta mais uma sessão ativa saudável.
- Falhas de desativação não descartam mais a sessão necessária para retry.
- Monitor não tenta mais dar `join` na própria thread.
- Confirmação do aviso vale para a execução atual mesmo sem marcar a opção de
  persistência.
- Atalho criado pelo CLI aponta para `NukaBoost.exe`, não para
  `nukaboostctl.exe`.
- Encerramento não fecha o processo quando a restauração do plano falha.
- Erros WiX WIX0103/WIX1077 e validações ICE38/ICE64.
- Checagem de política fornece o GUID exigido por `ACCESS_SCHEME` e preserva
  erros Win32 que não representam bloqueio administrativo.
- Respostas IPC de status não repetem mais `protocol_version`, permitindo que
  `nukaboostctl status` e `status --json` sejam desserializados corretamente.
- Clique esquerdo no ícone ignora a mensagem legada duplicada enviada junto
  da notificação v4, evitando ativar e desativar na mesma interação.
- CI aceita a licença OSMF do WiX 7 somente depois de uma confirmação
  explícita por variável do repositório, permitindo instalar as extensões no
  runner sem o erro WIX7015.
- Help do `nukaboostctl` agora é montado no idioma salvo antes do parsing e
  cobre todos os comandos, argumentos e subcomandos em inglês e português.

[Unreleased]: https://github.com/arthru-vinicius/nukaboost/compare/v1.0.0...HEAD

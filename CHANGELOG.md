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

### Changed

- Recuperação inicial agora ocorre somente depois de adquirir a instância única.
- Rollback preserva watchdog, RunOnce e journal até confirmar a restauração.
- CLI `run` confirma proteções, trata Ctrl+C e preserva o exit code do filho.
- Protocolo IPC v2 inclui os estados `SYSTEMREQUIRED` de AC e DC.
- MSI usa bind paths, arquitetura x64 e seleção interativa de Startup.

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

[Unreleased]: https://github.com/arthru-vinicius/nukaboost/compare/v1.0.0...HEAD

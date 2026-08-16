# Release e assinatura no Windows

## Regra de publicação

Não publique uma release estável com executáveis ou MSI sem assinatura.
Assinatura Authenticode comprova origem e integridade, mas não é uma garantia
de que o SmartScreen nunca exibirá aviso: reputação também depende do
certificado, do hash e do histórico de downloads.

## Opção gratuita recomendada

Para este projeto open source, solicite participação na
[SignPath Foundation](https://signpath.org/). O serviço é gratuito para
projetos aceitos, mantém a chave em HSM e vincula o binário ao repositório de
origem. A aprovação não é automática. Entre as condições publicadas estão:
licença aprovada pela OSI, ausência de componentes proprietários/malware,
projeto ativo, documentado e já disponibilizado na forma a assinar, aviso das
mudanças no sistema, desinstalação, MFA dos mantenedores, política pública de
assinatura, build verificável e aprovação manual de cada release. Consulte os
[termos atuais da SignPath Foundation](https://signpath.org/terms) antes de se
candidatar.

Alternativas:

- Microsoft Store: melhor experiência contra alertas do SmartScreen, mas
  exige adequar o empacotamento e a conta de desenvolvedor;
- Microsoft Artifact Signing: serviço oficial para distribuição fora da
  Store, porém pago;
- certificado OV/EV de uma autoridade pública: pago;
- certificado autoassinado: útil apenas em máquinas administradas por você;
  para usuários comuns, equivale essencialmente a não assinar.

## Checklist da versão

1. Leia os [termos OSMF do WiX 7](https://docs.firegiant.com/wix/osmf/) e,
   caso concorde e esteja em conformidade, configure a variável de
   repositório `WIX7_OSMF_EULA_ACCEPTED=true` para permitir o build do MSI no
   GitHub Actions.
2. Atualize a versão em `Cargo.toml`, `installer/NukaBoost.wxs` e nos
   recursos `VERSIONINFO`/manifesto em `crates/nukaboost-win32/resources`.
   No MSI, mantenha o `UpgradeCode` permanente, gere um `ProductCode` novo
   para a versão e não o altere novamente em rebuilds dessa mesma versão.
3. Atualize as notas, confirme que `docs/plano.md` continua coerente e ative
   **Settings → Security → Private vulnerability reporting** no GitHub.
4. Execute:

   ```powershell
   cargo fmt --all -- --check
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   cargo build --release
   ```

5. Assine **primeiro** `NukaBoost.exe` e `nukaboostctl.exe`.
6. Gere o MSI com o comando do README.
7. Assine o MSI por último. Qualquer modificação posterior invalida a
   assinatura.
8. Verifique assinatura, MSI e hashes:

   ```powershell
   signtool verify /pa /all /v target\release\NukaBoost.exe
   signtool verify /pa /all /v target\release\nukaboostctl.exe
   signtool verify /pa /all /v installer\NukaBoost.msi
   wix msi validate -sice ICE91 installer\NukaBoost.msi
   Get-FileHash target\release\NukaBoost.exe, `
                target\release\nukaboostctl.exe, `
                installer\NukaBoost.msi -Algorithm SHA256
   ```

O helper `scripts/build-release.ps1` automatiza build, validação e hashes. Se
for informado um thumbprint de certificado já instalado no repositório
pessoal do Windows, ele também assina e verifica os três artefatos:

```powershell
.\scripts\build-release.ps1 -CertificateThumbprint "SEU_THUMBPRINT"
```

Sem o parâmetro, o resultado fica explicitamente marcado como build não
assinado para testes.

## Criando a release no GitHub

Releases são associadas a uma tag. Faça a primeira publicação como draft,
confira todos os arquivos e só então publique:

```powershell
git switch main
git pull --ff-only
git tag -s v1.0.1 -m "NukaBoost v1.0.1"
git push origin v1.0.1

gh release create v1.0.1 `
  dist\NukaBoost.exe `
  dist\nukaboostctl.exe `
  dist\NukaBoost.msi `
  dist\SHA256SUMS.txt `
  --draft `
  --verify-tag `
  --generate-notes `
  --title "NukaBoost v1.0.1"
```

`git push origin v1.0.1` também dispara `.github/workflows/publish-container.yml`:
ele reconstrói o MSI do zero e publica
`ghcr.io/<owner>/nukaboost:v1.0.1` — a imagem que serve o instalador via
`irm | iex` no homelab (ver `docker/`). Nenhum segredo extra é necessário;
o workflow usa o `GITHUB_TOKEN` padrão com permissão `packages: write`.

**Só na primeira publicação**, o pacote é criado como privado por padrão no
GHCR — abra **github.com/arthru-vinicius?tab=packages → nukaboost → Package
settings** e mude a visibilidade para *Public* (senão `docker compose pull`
no homelab falha com `unauthorized`). Publicações seguintes reaproveitam essa
configuração.

Se você ainda não usa GitHub CLI, crie a tag e depois abra **Releases → Draft
a new release**, selecione a tag, anexe os mesmos quatro arquivos e salve como
draft. Não anexe `target/` inteiro, PDBs, certificados ou chaves.

## Authenticode local

Com um certificado público instalado no Windows, use SHA-256 e timestamp RFC
3161:

```powershell
signtool sign /sha1 SEU_THUMBPRINT /fd SHA256 `
  /tr http://timestamp.digicert.com /td SHA256 arquivo.exe
```

Timestamp preserva a validade da assinatura depois que o certificado expira.
Use sempre a mesma identidade de publicação para acumular reputação e nunca
publique novamente um arquivo depois de modificar seus bytes.

## Verificação antimalware

Antes do draft final, execute Microsoft Defender atualizado e confira os
hashes. Um resultado limpo reduz risco, mas nenhum scanner pode prometer que
jamais haverá falso positivo. Se ocorrer uma detecção, investigue e use o
portal oficial de submissão do fornecedor; não tente ofuscar o binário para
“enganar” antivírus.

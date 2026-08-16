# NukaBoost -- instalador via uma linha, servido por este mesmo host:
#
#   irm https://nukaboost.arthru.com | iex
#
# O host acima ainda depende de confirmacao do subdominio pelo lado do
# homelab (ver tmp/solicitacao.txt, item 6); atualize as duas ocorrencias
# de $installerHost abaixo se o subdominio final for outro, e reconstrua a
# imagem -- o valor fica embutido no script, nao e resolvido em runtime.
#
# Baixa o .msi embutido nesta mesma imagem (rota /NukaBoost.msi) e instala
# silenciosamente via msiexec /qn. Sem elevacao: o MSI e per-user por
# desenho (ver docs/plano.md secao 15).

$ErrorActionPreference = 'Stop'

$installerHost = 'https://nukaboost.arthru.com'
$msiUrl        = "$installerHost/NukaBoost.msi"
$msiPath       = Join-Path $env:TEMP 'NukaBoost.msi'

try {
    Write-Host 'Downloading NukaBoost...'
    Invoke-WebRequest -Uri $msiUrl -OutFile $msiPath -UseBasicParsing

    Write-Host 'Installing NukaBoost...'
    $process = Start-Process -FilePath 'msiexec.exe' `
        -ArgumentList @('/i', "`"$msiPath`"", '/qn', '/norestart') `
        -Wait -PassThru

    if ($process.ExitCode -ne 0) {
        Write-Error "NukaBoost installation failed (msiexec exit code $($process.ExitCode))."
        exit $process.ExitCode
    }

    Write-Host 'NukaBoost installed. Launch it from the Start Menu, or run: NukaBoost.exe'
}
finally {
    Remove-Item -Path $msiPath -ErrorAction SilentlyContinue
}

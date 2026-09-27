param(
    [string]$Minecraft = 'E:\mc',
    [string]$Java = 'D:\Program Files\Microsoft\jdk-25.0.3.9-hotspot\bin\java.exe'
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$run = Join-Path $repo ('target\sign-bank-runtime-' + [guid]::NewGuid().ToString('N'))
if (-not $run.StartsWith($repo + '\target\')) { throw 'Runtime output escaped target/' }
New-Item -ItemType Directory -Path (Join-Path $run 'packs') | Out-Null
$compiler = Join-Path $repo 'target\debug\mclang.exe'
& $compiler build (Join-Path $repo 'examples\sign_bank') -o (Join-Path $run 'packs\sign_bank') --deny-raw
if ($LASTEXITCODE -ne 0) { throw 'MCL build failed' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'pack') -Destination (Join-Path $run 'packs\verification') -Recurse
$version = Get-Content -Raw -LiteralPath (Join-Path $Minecraft 'versions\26.3\26.3.json') | ConvertFrom-Json
$jars = @((Join-Path $Minecraft 'versions\26.3\26.3.jar'))
foreach ($library in $version.libraries) {
    if ($library.downloads.artifact.path) {
        $candidate = Join-Path (Join-Path $Minecraft 'libraries') $library.downloads.artifact.path
        if (Test-Path -LiteralPath $candidate) { $jars += $candidate }
    }
}
$classpath = $jars -join ';'
$universe = Join-Path $run 'world'
if (Test-Path -LiteralPath $universe) { throw 'Refusing to replace an existing world' }
Write-Output "Runtime artifacts: $run"
Push-Location $run
try {
    & $Java -Xmx2G -XX:ActiveProcessorCount=4 -cp $classpath (Join-Path $PSScriptRoot 'SignBankRuntime.java') --universe $universe --packs (Join-Path $run 'packs') --tests 'sign_bank_test:integration' --report (Join-Path $run 'results.xml')
    $result = $LASTEXITCODE
    if ($result -eq 0) {
        $log = Get-Content -Raw -LiteralPath (Join-Path $run 'logs\latest.log')
        if ($log -notmatch 'SIGN_BANK_RUNTIME_PASS' -or $log -match 'Serialization errors|Failed to load function|Invalid color name') {
            throw 'Runtime assertions or resource decoding failed; inspect logs/latest.log'
        }
    }
} finally { Pop-Location }
exit $result

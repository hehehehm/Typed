# Compatible with Windows PowerShell 5.1 and PowerShell 7.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

try {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    $manifest = Join-Path $repoRoot 'Cargo.toml'
    if (-not (Test-Path -LiteralPath $manifest)) {
        throw "Не найден Cargo.toml: $manifest"
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'Не найден cargo. Установите Rust: https://rustup.rs/ и откройте новое окно PowerShell.'
    }

    # Read only [workspace.package], not a dependency version.
    $toml = Get-Content -LiteralPath $manifest -Raw
    $section = [regex]::Match($toml, '(?ms)^\[workspace\.package\]\s*\r?\n(?<body>.*?)(?=^\[|\z)')
    $versionMatch = [regex]::Match($section.Groups['body'].Value, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $section.Success -or -not $versionMatch.Success) {
        throw 'Не найдена версия [workspace.package] в Cargo.toml.'
    }
    $version = $versionMatch.Groups[1].Value

    $iscc = $null
    $candidates = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
        'C:\Program Files\Inno Setup 6\ISCC.exe',
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 7\ISCC.exe'),
        'C:\Program Files (x86)\Inno Setup 7\ISCC.exe',
        'C:\Program Files\Inno Setup 7\ISCC.exe'
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            $iscc = $candidate
            break
        }
    }
    if (-not $iscc) {
        $command = Get-Command iscc.exe -ErrorAction SilentlyContinue
        if (-not $command) { $command = Get-Command iscc -ErrorAction SilentlyContinue }
        if ($command) { $iscc = $command.Source }
    }
    if (-not $iscc) {
        throw 'Установите Inno Setup 6 или 7: https://jrsoftware.org/isinfo.php (можно только для текущего пользователя), затем запустите этот скрипт снова.'
    }

    Push-Location $repoRoot
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build завершился с кодом $LASTEXITCODE" }
    } finally {
        Pop-Location
    }

    $buildDir = Join-Path $repoRoot 'target\release'
    $exe = Join-Path $buildDir 'layout-keeper.exe'
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "Не найден Windows-бинарник: $exe. Запускайте сборку на Windows без переопределения target."
    }
    $outputDir = Join-Path $repoRoot 'dist'
    $script = Join-Path $PSScriptRoot 'layout-keeper.iss'
    & $iscc "/DMyAppVersion=$version" "/DBuildDir=$buildDir" "/DInstallerOutputDir=$outputDir" $script
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup завершился с кодом $LASTEXITCODE" }

    $installer = Join-Path $outputDir "Typed-Setup-$version.exe"
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
        throw "Установщик не найден: $installer"
    }
    Write-Host "Готово! Отправьте тестерам один файл: $installer"
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}

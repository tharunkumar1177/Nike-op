param(
    [string]$Configuration = "Release",
    [string]$OutputDirectory = "$PSScriptRoot\..\target\release"
)

$project = Join-Path $PSScriptRoot "..\apps\EdgeOptimizer.Settings.WinUI\EdgeOptimizer.Settings.WinUI.csproj"

dotnet publish $project --configuration $Configuration --output $OutputDirectory
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$settingsExecutable = Join-Path $OutputDirectory "EdgeOptimizer.Settings.WinUI.exe"
if (-not (Test-Path -LiteralPath $settingsExecutable)) {
    throw "WinUI Settings publish did not produce $settingsExecutable."
}

param(
    [switch]$Create,
    [string]$Repo,
    [switch]$NoCapture,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    [Console]::Error.WriteLine("Usage: scripts\live-github.ps1 -Create | -Repo sethjuarez/fake-repo-example [-NoCapture]")
}

if ($Help) {
    Show-Usage
    exit 0
}

if (($Create -and $Repo) -or (-not $Create -and -not $Repo)) {
    Show-Usage
    exit 2
}

if ($Repo -and ($Repo -notmatch '^sethjuarez/(fake-repo-|autorepo-test-).+')) {
    [Console]::Error.WriteLine("Live GitHub tests may target only sethjuarez/fake-repo-* or sethjuarez/autorepo-test-*")
    exit 2
}

$env:AUTOREPO_LIVE_GITHUB = "1"

if ($Create) {
    $env:AUTOREPO_LIVE_CREATE = "1"
    Remove-Item Env:\AUTOREPO_LIVE_REPO -ErrorAction SilentlyContinue
} else {
    $env:AUTOREPO_LIVE_REPO = $Repo
    Remove-Item Env:\AUTOREPO_LIVE_CREATE -ErrorAction SilentlyContinue
}

$cargoArgs = @("test", "--test", "live_github", "--", "--ignored")
if ($NoCapture) {
    $cargoArgs += "--nocapture"
}

& cargo @cargoArgs
exit $LASTEXITCODE

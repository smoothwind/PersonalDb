# UI E2E smoke suite - launch app with WebView2 CDP debug port, run read-only probes, PASS/FAIL verdict.
# How it works: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS injects --remote-debugging-port into WebView2,
#   then Runtime.evaluate drives in-page JS via DevTools protocol (zero extra deps; no tauri-driver needed).
# Usage: powershell -File ui-smoke.ps1 [-ExePath "D:\Program Files (x86)\PersonalDb\personal-db.exe"]
# Note: kills the running personal-db process first (restarts it with the debug port).
param([string]$ExePath = "D:\Program Files (x86)\PersonalDb\personal-db.exe")
$ErrorActionPreference = 'Continue'
. (Join-Path $PSScriptRoot 'cdp-lib.ps1')

# 1) restart app with debug port
Stop-Process -Name personal-db -Force -ErrorAction SilentlyContinue
Start-Sleep 2
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9223"
Start-Process $ExePath

# 2) wait for CDP port
$ready = $false
for ($i = 0; $i -lt 20; $i++) {
    Start-Sleep 2
    try { $null = Invoke-RestMethod -Uri "http://127.0.0.1:9223/json/list" -ErrorAction Stop; $ready = $true; break } catch { }
}
if (-not $ready) { Write-Output "SMOKE FAIL: CDP port not ready"; exit 1 }
Start-Sleep 3  # wait for frontend mount (splash -> app_ready main window)

# 3) probes (read-only, never touch user data)
$results = @()

# P1 app mounted + splash closed
$r = Invoke-CdpEval -Expr 'JSON.stringify({title: document.title, mounted: !!document.querySelector(''#app'').__vue_app__})'
$results += @{ name = "P1 app mounted"; raw = $r; ok = ($r -match '"title"\s*:\s*"PersonalDb"') -and ($r -match '"mounted":true') }

# P2 menu bar structure (menu items present)
$r = Invoke-CdpEval -Expr 'document.querySelectorAll(''.el-menu-item'').length'
$n = 0.0
$rOk = [double]::TryParse("$r".Trim(), [ref]$n) -and ($n -ge 15)
$results += @{ name = "P2 menu bar"; raw = $r; ok = $rOk }

# P3 editor + result grid + panel rail
$r = Invoke-CdpEval -Expr 'JSON.stringify({cm: document.querySelectorAll(''.cm-editor'').length, grid: document.querySelectorAll(''.el-table'').length, rail: !!document.querySelector(''[class*=rail]'')})'
$results += @{ name = "P3 editor/grid/rail"; raw = $r; ok = ($r -match '"cm":\d+') -and ($r -match '"rail":true') }

# P4 connection tree rendered
$r = Invoke-CdpEval -Expr '!!document.querySelector(''.el-tree'')'
$results += @{ name = "P4 connection tree"; raw = $r; ok = ($r -match 'true') }

# P5 status bar version guard: must equal the hardcoded version in repo src/App.vue
# (real find: local machine had a pre-bump build installed showing stale version)
$srcVer = $null
$appVue = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'src\App.vue'
$m = Select-String -Path $appVue -Pattern 'PersonalDb (v0\.\d+\.\d+)' | Select-Object -First 1
if ($m) { $srcVer = $m.Matches.Groups[1].Value }
$r = Invoke-CdpEval -Expr 'JSON.stringify({ver: (document.body.innerText.match(/PersonalDb v0\.\d+\.\d+/)||[''''])[0]})'
$results += @{ name = "P5 status-bar version guard (expect $srcVer)"; raw = $r; ok = ($null -ne $srcVer) -and ($r -match [regex]::Escape("PersonalDb $srcVer")) }

# 4) summary
$fail = 0
foreach ($x in $results) {
    $mark = if ($x.ok) { 'PASS' } else { $fail++; 'FAIL' }
    Write-Output "[$mark] $($x.name)"
    if (-not $x.ok) { Write-Output "       raw: $($x.raw.Trim())" }
}
Write-Output "SMOKE: $($results.Count - $fail)/$($results.Count) PASS"
if ($fail -gt 0) { exit 1 }

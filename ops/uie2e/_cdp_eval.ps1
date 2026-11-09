# CLI wrapper: evaluate one JS expression in the running app page via CDP.
# Usage: powershell -File _cdp_eval.ps1 -Expr '<JS expression>'
param([Parameter(Mandatory=$true)][string]$Expr)
. (Join-Path $PSScriptRoot 'cdp-lib.ps1')
Invoke-CdpEval -Expr $Expr

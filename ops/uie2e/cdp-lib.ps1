# CDP eval library - dot-source me: . (Join-Path $PSScriptRoot 'cdp-lib.ps1')
# function: Invoke-CdpEval -Expr '<JS expression>'  -> string result (or CDP-ERROR payload)
# requires the app running with WebView2 --remote-debugging-port=9223.
function Invoke-CdpEval {
    param([Parameter(Mandatory=$true)][string]$Expr, [int]$Port = 9223)
    $pages = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/list"
    $page = $pages | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
    if (-not $page) { throw "no page on CDP port $Port - is the app running with the debug port?" }
    $ws = New-Object System.Net.WebSockets.ClientWebSocket
    $ct = [System.Threading.CancellationToken]::None
    $ws.ConnectAsync([Uri]$page.webSocketDebuggerUrl, $ct).Wait()

    $payload = @{
        id = 1
        method = 'Runtime.evaluate'
        params = @{ expression = $Expr; returnByValue = $true; awaitPromise = $false }
    } | ConvertTo-Json -Depth 6 -Compress
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
    $ws.SendAsync([ArraySegment[byte]]::new($bytes), [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $ct).Wait()

    $buffer = New-Object byte[] (16MB)
    $ms = New-Object System.IO.MemoryStream
    while ($true) {
        $seg = [ArraySegment[byte]]::new($buffer)
        $r = $ws.ReceiveAsync($seg, $ct).Result
        $ms.Write($buffer, 0, $r.Count)
        if ($r.EndOfMessage) {
            $text = [System.Text.Encoding]::UTF8.GetString($ms.ToArray())
            $ms.SetLength(0)
            try {
                $resp = $text | ConvertFrom-Json
                if ($resp.id -eq 1) {
                    if ($resp.result.exceptionDetails) {
                        return "CDP-ERROR: " + ($resp.result.exceptionDetails | ConvertTo-Json -Depth 5 -Compress)
                    }
                    $v = $resp.result.result.value
                    if ($null -eq $v) { return "(null/undefined)" }
                    if ($v -is [string]) { return $v }  # already text - avoid double JSON encoding
                    return ($v | ConvertTo-Json -Depth 5 -Compress)
                }
            } catch { }  # non-JSON frames (events) - keep reading
        }
    }
    $ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, 'done', $ct).Wait()
    $ws.Dispose()
}

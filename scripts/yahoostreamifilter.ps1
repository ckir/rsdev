Add-Type -AssemblyName System.Net.WebSockets
Add-Type -AssemblyName System.Runtime.Extensions

# NASDAQ-100 tickers (as of current composition)
$nasdaq100 = @(
    "AAPL","MSFT","AMZN","NVDA","GOOG","GOOGL","META","TSLA","AVGO","PEP",
    "COST","ADBE","NFLX","AMD","INTC","CSCO","CMCSA","TXN","QCOM","AMGN",
    "HON","SBUX","INTU","AMAT","MDLZ","BKNG","ADI","PYPL","GILD","LRCX",
    "REGN","VRTX","MU","PANW","ADP","MAR","CSX","CHTR","MRNA","KLAC",
    "SNPS","CDNS","KDP","MELI","MNST","FTNT","AEP","ORLY","PDD","CTAS",
    "NXPI","IDXX","KHC","PAYX","ODFL","PCAR","XEL","ROST","AZN","CRWD",
    "ABNB","TEAM","WDAY","LCID","BIDU","ZM","DOCU","SPLK","OKTA","DDOG",
    "ZS","MDB","VRSN","EA","EXC","FAST","FISV","FOX","FOXA","JD","LULU",
    "MRVL","NTES","SGEN","SIRI","SWKS","VRSK","WBA","WBD","WDC","BMRN",
    "ALGN","ANSS","CPRT","DLTR","EBAY","ISRG","MTCH","TSCO","TTWO"
)

$uri = "wss://streamer.finance.yahoo.com/?version=2"
$ws = [System.Net.WebSockets.ClientWebSocket]::new()

Write-Host "Connecting to Yahoo Finance WebSocket..."
$ws.ConnectAsync($uri, [Threading.CancellationToken]::None).Wait()

# Build subscription message
$subscribe = @{ subscribe = $nasdaq100 } | ConvertTo-Json
$bytes = [System.Text.Encoding]::UTF8.GetBytes($subscribe)
$buffer = [System.ArraySegment[byte]]::new($bytes)
$ws.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, [Threading.CancellationToken]::None).Wait()

Write-Host "Subscribed to NASDAQ-100. Listening..."

$receiveBuffer = New-Object Byte[] 4096
$global:count = 0

while ($ws.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
    $segment = [System.ArraySegment[byte]]::new($receiveBuffer)
    $result = $ws.ReceiveAsync($segment, [Threading.CancellationToken]::None).Result

    if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close) {
        Write-Host "WebSocket closed by server"
        break
    }

    $msg = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)

    # Count every incoming message
    $global:count++
    if ($global:count % 1000 -eq 0) {
        Write-Host "Processed $($global:count) messages..."
    }

    # Try to parse JSON
    try {
        $json = $msg | ConvertFrom-Json -ErrorAction Stop
    } catch {
        continue
    }

    # Filter out pricing messages
    if ($json.type -eq "pricing") {
        continue
    }

    Write-Host "Filtered:" ($json | ConvertTo-Json -Depth 10)
}
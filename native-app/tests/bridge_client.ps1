# Talks to a running VyomKunji desktop over its browser link, the way the
# native host does. Usage: pwsh tests/bridge_client.ps1 '{"type":"ping"}'
param([string]$Json)
$dir = Join-Path $env:APPDATA 'VyomKunji'
$port = [int](Get-Content (Join-Path $dir 'gui.port')).Trim()
$token = (Get-Content (Join-Path $dir 'gui.token')).Trim()
$req = $Json | ConvertFrom-Json
$req | Add-Member -NotePropertyName token -NotePropertyValue $token -Force
$client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', $port)
$stream = $client.GetStream()
$writer = [System.IO.StreamWriter]::new($stream)
$writer.WriteLine(($req | ConvertTo-Json -Compress -Depth 5))
$writer.Flush()
$reader = [System.IO.StreamReader]::new($stream)
$reader.ReadLine()
$client.Close()

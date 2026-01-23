#!/usr/bin/env pwsh
$tcpClient = New-Object System.Net.Sockets.TcpClient("localhost", 18081)
$stream = $tcpClient.GetStream()
$message = '{"message":{"lang":"en","text":"My log Message"},"options":{"cron":"1/7 * * * * *","repeat":1,"interval":0,"voptions":["-a 50 -s 130 -p 80 -v mb-us1"]}}'
$writer = New-Object System.IO.StreamWriter($stream)
$writer.WriteLine($message)
$writer.Flush()
$writer.Close()
$tcpClient.Close()

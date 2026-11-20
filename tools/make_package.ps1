$ErrorActionPreference = 'Stop'
$root = "C:\Users\Administrator\Desktop\scp"
$exe = "$root\scp_sl_3d\target\release\scp_sl_3d.exe"
$destDir = "$root\scp_sl_3d\dist\scp_sl_3d"
$zip = "$root\releases\scp_sl_3d.zip"

Copy-Item $exe "$destDir\scp_sl_3d.exe" -Force
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$destDir\*" -DestinationPath $zip -CompressionLevel Optimal
Get-Item $zip | Select-Object FullName, @{N='MB';E={[math]::Round($_.Length/1MB,1)}} | Format-List

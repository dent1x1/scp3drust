param([string]$Out = "C:\Users\Administrator\.arcanic\shot_3d.png", [switch]$Menu, [string]$Keys = "", [switch]$PostEnter, [string]$Exe = "C:\Users\Administrator\.arcanic\scp_sl_3d\target\debug\scp_sl_3d.exe")

$ErrorActionPreference = 'Stop'
if (-not $Menu) {
    $env:SCP_AUTO = '1'
    $env:SCP_NOLOCK = '1'
    $env:SCP_NOLOOK = '1'
} else {
    Remove-Item Env:SCP_AUTO -ErrorAction SilentlyContinue
    Remove-Item Env:SCP_NOLOCK -ErrorAction SilentlyContinue
    Remove-Item Env:SCP_NOLOOK -ErrorAction SilentlyContinue
}
"AUTO=[$env:SCP_AUTO] MENU=[$Menu]"

$exe = $Exe
$p = Start-Process -FilePath $exe -PassThru `
    -RedirectStandardOutput "$env:TEMP\scp3d_out.txt" `
    -RedirectStandardError "$env:TEMP\scp3d_err.txt"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinFg {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$h = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 500
    $p.Refresh()
    if ($p.HasExited) { break }
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $h = $p.MainWindowHandle; break }
}

$gameH = [IntPtr]::Zero
for ($j = 0; $j -lt 20; $j++) {
    $gameH = [WinFg]::FindWindow($null, "SCP: Secret Laboratory — 3D (Rust + Bevy)")
    if ($gameH -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 500
}
if ($gameH -ne [IntPtr]::Zero) { $h = $gameH }

$alive = -not $p.HasExited
if ($h -ne [IntPtr]::Zero) {
    [WinFg]::ShowWindow($h, 9) | Out-Null   # SW_RESTORE
    [WinFg]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Seconds 2
    if ($PostEnter) {
        Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinMsg {
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
"@
        [WinMsg]::PostMessage($h, 0x0100, [IntPtr]0x0D, [IntPtr]0x001C0001) | Out-Null
        Start-Sleep -Milliseconds 50
        [WinMsg]::PostMessage($h, 0x0101, [IntPtr]0x0D, [IntPtr]0xC01C0001) | Out-Null
        Start-Sleep -Seconds 2
    }
    if ($Keys -ne "") {
        Add-Type -AssemblyName System.Windows.Forms
        [System.Windows.Forms.SendKeys]::SendWait($Keys)
        Start-Sleep -Milliseconds 1500
    }
    Start-Sleep -Seconds 1
}

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

if ($alive) { Stop-Process -Id $p.Id -Force }
"hwnd=$h alive=$alive out=$Out"
Get-Content "$env:TEMP\scp3d_err.txt" -Tail 15

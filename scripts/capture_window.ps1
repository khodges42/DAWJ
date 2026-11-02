# SPDX-License-Identifier: GPL-3.0-or-later
# Capture only the explicitly supplied test process's native window; no desktop capture.
param([Parameter(Mandatory=$true)][int]$ProcessId,[Parameter(Mandatory=$true)][string]$Destination)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class DawjCapture {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd,IntPtr dc,uint flags);
}
'@
$testProcess=Get-Process -Id $ProcessId
$testProcess.Refresh();$windowHandle=$testProcess.MainWindowHandle
if($windowHandle -eq [IntPtr]::Zero){throw 'Test process has no native window'}
$rect=New-Object DawjCapture+Rect
if(-not [DawjCapture]::GetWindowRect($windowHandle,[ref]$rect)){throw 'Cannot read test-window bounds'}
$width=$rect.Right-$rect.Left;$height=$rect.Bottom-$rect.Top
if($width -lt 100 -or $height -lt 100 -or $width -gt 4096 -or $height -gt 4096){throw 'Invalid capture dimensions'}
$bitmap=New-Object System.Drawing.Bitmap($width,$height)
$graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
try{if(-not [DawjCapture]::PrintWindow($windowHandle,$dc,2)){throw 'Native capture unavailable'}}finally{$graphics.ReleaseHdc($dc);$graphics.Dispose()}
try{$bitmap.Save($Destination,[Drawing.Imaging.ImageFormat]::Png)}finally{$bitmap.Dispose()}

param([Parameter(Mandatory=$true)][string]$SessionDir, [string]$TestClient)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$jobFile = Join-Path $SessionDir 'job.json'
$exitFile = Join-Path $SessionDir 'exit-code'
$outFile = Join-Path $SessionDir 'stdout.log'
$errFile = Join-Path $SessionDir 'stderr.log'
$vpnProcess = $null
$result = 1
$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::OutputEncoding = $utf8
$splitGroups = @()
$networkReady = $false
$connected = $false
$authFailed = $false
$terminal = $false
$dnsReady = $false
$lastHealthCheck = [DateTime]::MinValue
$lastServiceCheck = [DateTime]::MinValue
$serviceFailures = 0
$readyAt = $null
$networkScriptPath = Join-Path $PSScriptRoot 'windows-network.ps1'
function Log-Error([string]$text) { [IO.File]::AppendAllText($errFile, "ERROR: $text`n", $utf8) }
function Write-Status([string]$phase, [string]$message = '') {
    $status = @{ phase=$phase; message=$message; time=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() } | ConvertTo-Json -Compress
    [IO.File]::WriteAllText((Join-Path $SessionDir 'status.json'), $status, $utf8)
}
function Save-SplitDns {
    $file = Join-Path $SessionDir 'split-dns.json'
    [IO.File]::WriteAllText($file, (ConvertTo-Json -InputObject @($script:splitGroups) -Depth 5 -Compress), $utf8)
}
function Get-HealthDecision($health, [bool]$wasConnected, [int]$previousFailures, [double]$startupSeconds) {
    if ($health.ok) { return @{ phase='connected'; failures=0 } }
    if ($wasConnected -and $health.category -eq 'service') {
        # A service probe tells us about the application behind the VPN, not
        # the tunnel itself. Keep the adapter/session connected and retry the
        # probe without reauthenticating; topology and OpenConnect terminal
        # events remain the authoritative disconnect signals.
        return @{ phase='connected'; failures=[Math]::Min(3, $previousFailures + 1) }
    }
    if (!$wasConnected -and $startupSeconds -le 15) { return @{ phase='waiting'; failures=0 } }
    return @{ phase='disconnected'; failures=0 }
}
function Write-ClientLine([string]$line, [string]$file) {
    if ($line -match '^(Microsoft \(R\) Windows Script Host|Copyright \(C\) Microsoft Corporation)') { return }
    if ($line -eq 'TUNNELYARD_NETWORK_READY') { $script:networkReady=$true; $script:readyAt=[DateTime]::UtcNow }
    if ($line -match '^WARNING: Got split-DNS domains (.+) \(not yet implemented\)$') {
        $domains = @($Matches[1] -split ',' | ForEach-Object { $_.Trim().TrimEnd('.').ToLowerInvariant() })
        $script:splitGroups += @{ domains=$domains; servers=@() }
        Save-SplitDns
    } elseif ($line -match '^WARNING: Got split-DNS server ([0-9.]+) \(not yet implemented\)$' -and $script:splitGroups.Count) {
        $script:splitGroups[-1].servers += $Matches[1]
        Save-SplitDns
    }
    if ($line -match 'Invalid credentials|Authentication failed|User input required|Cookie was rejected by server|Cookie is no longer valid') { $script:authFailed=$true }
    if ($line -match 'Cookie was rejected by server|Cookie is no longer valid|Dead peer detected|DTLS.*dead peer|Failed to reconnect|Session authentication expired|Server terminated|VPN session ended') {
        $script:terminal=$true
        $script:connected=$false
        Write-Status 'disconnected' $line
        [IO.File]::AppendAllText($outFile, "TUNNELYARD_TUNNEL_DOWN: $line`n", $utf8)
    }
    [IO.File]::AppendAllText($file, "$line`n", $utf8)
}
function Quote-Argument([string]$value) {
    # CommandLineToArgvW quoting, including trailing backslashes and quotes.
    return '"' + [regex]::Replace([regex]::Replace($value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1') + '"'
}
try {
    $job = Get-Content -LiteralPath $jobFile -Raw -Encoding UTF8 | ConvertFrom-Json
    Remove-Item -LiteralPath $jobFile
    if (!(Test-Path -LiteralPath $job.bin -PathType Leaf) -or [IO.Path]::GetFileName($job.bin) -ne 'openconnect.exe') { throw 'OpenConnect executable not found.' }
    # The unprivileged job file is data, not an arbitrary elevated command.
    # TestClient is an explicit command-line-only hook for localhost CI tests;
    # the application never supplies it and job JSON cannot enable it.
    $allowedBins = @($env:ProgramW6432, $env:ProgramFiles, ${env:ProgramFiles(x86)}) |
        Where-Object { $_ } | ForEach-Object { Join-Path $_ 'OpenConnect\openconnect.exe' }
    if ($TestClient) { $allowedBins = @([IO.Path]::GetFullPath($TestClient)) }
    if ([IO.Path]::GetFullPath($job.bin) -notin $allowedBins) { throw 'Install OpenConnect in its standard Program Files location.' }
    $networkScript = Join-Path $PSScriptRoot 'vpnc-script-win.js'
    $validatedArgs = @()
    foreach ($arg in $job.args) {
        if ($arg -match '[\r\n\x00]') { throw 'Invalid client argument.' }
        if ($arg.StartsWith('--script=')) {
            if ([IO.Path]::GetFullPath($arg.Substring(9)) -ne [IO.Path]::GetFullPath($networkScript)) { throw 'Only the bundled VPN network script is allowed.' }
            continue
        }
        if ($arg -notmatch '^(--protocol=fortinet|--passwd-on-stdin|--non-inter|--disable-ipv6|--no-dtls|--reconnect-timeout=1|--force-dpd=10|--user=.+|--usergroup=.+|--servercert=pin-sha256:[A-Za-z0-9+/]+=*|--interface=TunnelYard-[A-Za-z0-9]+|--cafile=.+|--certificate=.+|--sslkey=.+|https://[^\s]+)$') { throw 'Unsupported client argument.' }
        $validatedArgs += [string]$arg
    }
    $validatedArgs = @("--script=$networkScript") + $validatedArgs
    # Patched OpenConnect builds use this opt-in to reproduce openfortivpn's
    # legacy FortiGate TLS hand-off for profiles that need it. Official builds
    # simply ignore the variable and keep their normal request sequence.
    $env:TUNNELYARD_FORTINET_LEGACY = if ($job.legacyTunnel) { '1' } else { '0' }
    if (Test-Path -LiteralPath (Join-Path $SessionDir 'stop')) { $result = 0; return }
    # This supervisor has its own hidden elevated console. Ctrl+C is delivered
    # to OpenConnect, whose handler logs out and calls the disconnect script.
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VpnConsole {
    delegate bool Handler(uint signal);
    static Handler handler = signal => true;
    [DllImport("kernel32.dll")] static extern bool SetConsoleCtrlHandler(Handler h, bool add);
    [DllImport("kernel32.dll")] static extern bool GenerateConsoleCtrlEvent(uint signal, uint group);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr security, uint creation, uint flags, IntPtr template);
    [DllImport("kernel32.dll")] static extern bool SetStdHandle(int which, IntPtr handle);
    [DllImport("kernel32.dll")] static extern bool SetHandleInformation(IntPtr handle, uint mask, uint flags);
    [DllImport("kernel32.dll")] static extern bool SetConsoleOutputCP(uint cp);
    [DllImport("kernel32.dll")] static extern bool GetConsoleMode(IntPtr handle, out uint mode);
    [DllImport("kernel32.dll")] static extern bool SetConsoleMode(IntPtr handle, uint mode);
    [StructLayout(LayoutKind.Explicit, CharSet=CharSet.Unicode, Size=20)]
    struct InputRecord {
        [FieldOffset(0)] public short EventType;
        [FieldOffset(4)] public int KeyDown;
        [FieldOffset(8)] public short RepeatCount;
        [FieldOffset(10)] public short KeyCode;
        [FieldOffset(12)] public short ScanCode;
        [FieldOffset(14)] public char UnicodeChar;
        [FieldOffset(16)] public uint ControlKeyState;
    }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool WriteConsoleInputW(IntPtr handle, InputRecord[] records, uint length, out uint written);
    static IntPtr input;
    public static void Prepare() {
        SetConsoleCtrlHandler(handler, true);
        SetConsoleOutputCP(65001);
        input = CreateFileW("CONIN$", 0xc0000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
        if (input == new IntPtr(-1)) throw new InvalidOperationException("A private console is required for the VPN supervisor.");
        SetHandleInformation(input, 1, 1);
        SetStdHandle(-10, input);
        uint mode;
        if (GetConsoleMode(input, out mode)) SetConsoleMode(input, mode & ~4u); // no credential echo
    }
    public static void SendInput(string text) {
        var records = new InputRecord[text.Length];
        for (int i=0; i<text.Length; i++) {
            char c = text[i] == '\n' ? '\r' : text[i];
            records[i] = new InputRecord { EventType=1, KeyDown=1, RepeatCount=1, UnicodeChar=c, KeyCode=(short)(c=='\r' ? 13 : 0) };
        }
        uint written;
        if (!WriteConsoleInputW(input, records, (uint)records.Length, out written) || written != records.Length)
            throw new InvalidOperationException("Could not send credentials to the VPN client.");
        Array.Clear(records, 0, records.Length);
    }
    public static void Stop() { GenerateConsoleCtrlEvent(0, 0); }
}
'@
    [VpnConsole]::Prepare()
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $job.bin
    $info.Arguments = (($validatedArgs | ForEach-Object { Quote-Argument $_ }) -join ' ')
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $false
    $info.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $info.RedirectStandardInput = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = $utf8
    $info.StandardErrorEncoding = $utf8
    $info.WorkingDirectory = Split-Path -Parent $job.bin
    $info.EnvironmentVariables['LANG'] = 'C'
    $info.EnvironmentVariables['LC_ALL'] = 'C'
    $info.EnvironmentVariables['TUNNELYARD_SESSION_DIR'] = $SessionDir
    $info.EnvironmentVariables['TUNNELYARD_SET_DNS'] = [int][bool]$job.setDns
    $info.EnvironmentVariables['TUNNELYARD_SET_ROUTES'] = [int][bool]$job.setRoutes
    if ($job.healthHost) {
        $healthIp = [Net.IPAddress]::Parse($job.healthHost)
        if ($healthIp.AddressFamily -ne 'InterNetwork' -or [int]$job.healthPort -lt 1 -or [int]$job.healthPort -gt 65535) { throw 'Invalid VPN health-check target.' }
        $info.EnvironmentVariables['TUNNELYARD_HEALTH_HOST'] = $healthIp.ToString()
        $info.EnvironmentVariables['TUNNELYARD_HEALTH_PORT'] = [string][int]$job.healthPort
    }
    $vpnProcess = New-Object System.Diagnostics.Process
    $vpnProcess.StartInfo = $info
    [void]$vpnProcess.Start()
    # OpenConnect 9.21's Windows pipe reader uses the legacy C code page.
    # Its ReadConsoleW path preserves Unicode. Supply input to this isolated,
    # hidden console without placing any secret in argv or simulating global keys.
    [VpnConsole]::SendInput([string]$job.password)
    $job.password = $null
    $outTask = $vpnProcess.StandardOutput.ReadLineAsync()
    $errTask = $vpnProcess.StandardError.ReadLineAsync()
    $stoppedAt = $null
    $startedAt = [DateTime]::UtcNow
    Write-Status 'connecting'
    while (!$vpnProcess.HasExited -or $null -ne $outTask -or $null -ne $errTask) {
        if ($null -ne $outTask -and $outTask.IsCompleted) {
            $line = $outTask.GetAwaiter().GetResult()
            if ($null -eq $line) { $outTask = $null } else {
                Write-ClientLine $line $outFile
                $outTask = $vpnProcess.StandardOutput.ReadLineAsync()
            }
        }
        if ($null -ne $errTask -and $errTask.IsCompleted) {
            $line = $errTask.GetAwaiter().GetResult()
            if ($null -eq $line) { $errTask = $null } else {
                Write-ClientLine $line $errFile
                $errTask = $vpnProcess.StandardError.ReadLineAsync()
            }
        }
        if (!$vpnProcess.HasExited) {
            if ($networkReady -and !$terminal -and !$stoppedAt -and (([DateTime]::UtcNow - $lastHealthCheck).TotalSeconds -ge 3)) {
                $lastHealthCheck = [DateTime]::UtcNow
                if (!$dnsReady) {
                    $dnsResult = & $networkScriptPath -Action ready -SessionDir $SessionDir
                    if ($dnsResult.ok) {
                        $dnsReady=$true
                        if ($dnsResult.splitDnsRules) { [IO.File]::AppendAllText($outFile, "VPN split-DNS configured and verified: $($dnsResult.splitDnsRules) domain policy rule(s).`n", $utf8) }
                    } else { Log-Error $dnsResult.message; $terminal=$true }
                }
                if ($dnsReady) {
                    # Keep topology checks frequent, but avoid opening a new
                    # unauthenticated service connection on every status tick.
                    $probeService = !$connected -or $serviceFailures -gt 0 -or (([DateTime]::UtcNow - $lastServiceCheck).TotalSeconds -ge 15)
                    $health = & $networkScriptPath -Action check -SessionDir $SessionDir -ProbeService $probeService
                    if ($probeService) { $lastServiceCheck = [DateTime]::UtcNow }
                    $decision = Get-HealthDecision $health $connected $serviceFailures (([DateTime]::UtcNow - $readyAt).TotalSeconds)
                    $serviceFailures = $decision.failures
                    if ($decision.phase -eq 'connected') {
                        Write-Status 'connected'
                        if ($decision.failures -gt 0) {
                            [IO.File]::AppendAllText($errFile, "WARNING: VPN service check $($decision.failures)/3 failed; keeping the healthy tunnel connected and retrying.`n", $utf8)
                        }
                        if (!$connected) { [IO.File]::AppendAllText($outFile, "TUNNELYARD_TUNNEL_UP`n", $utf8); $connected=$true }
                    } elseif ($decision.phase -eq 'disconnected') {
                        Log-Error $health.message
                        $terminal=$true
                        $connected=$false
                        Write-Status 'disconnected' $health.message
                        [IO.File]::AppendAllText($outFile, "TUNNELYARD_TUNNEL_DOWN: $($health.message)`n", $utf8)
                    }
                }
            }
            $heartbeat = Get-Item -LiteralPath (Join-Path $SessionDir 'heartbeat') -ErrorAction SilentlyContinue
            $orphan = $authFailed -or $terminal -or !$heartbeat -or (([DateTime]::UtcNow - $heartbeat.LastWriteTimeUtc).TotalSeconds -gt 15)
            if (!$connected -and (([DateTime]::UtcNow - $startedAt).TotalSeconds -gt 180)) { $orphan = $true }
            if (!$stoppedAt -and ($orphan -or (Test-Path -LiteralPath (Join-Path $SessionDir 'stop')))) {
                [VpnConsole]::Stop()
                $stoppedAt = [DateTime]::UtcNow
            }
            if ($stoppedAt -and (([DateTime]::UtcNow - $stoppedAt).TotalSeconds -gt 12)) {
                Log-Error 'OpenConnect did not stop in time; forcing exit and restoring owned network settings.'
                $vpnProcess.Kill()
            }
        }
        Start-Sleep -Milliseconds 50
    }
    $vpnProcess.WaitForExit()
    $result = $vpnProcess.ExitCode
} catch { Log-Error $_.Exception.Message }
finally {
    Write-Status 'disconnected'
    if ($vpnProcess -and !$vpnProcess.HasExited) {
        [VpnConsole]::Stop()
        if (!$vpnProcess.WaitForExit(12000)) { $vpnProcess.Kill(); $vpnProcess.WaitForExit() }
    }
    # Idempotent fallback also covers a crashed or forcibly stopped client.
    $env:TUNNELYARD_SESSION_DIR = $SessionDir
    $env:reason = 'disconnect'
    try { & (Join-Path $PSScriptRoot 'windows-network.ps1') } catch { Log-Error $_.Exception.Message; $result = 1 }
    [IO.File]::WriteAllText($exitFile, [string]$result, $utf8)
}
exit $result

param([string]$Action = $env:reason, [string]$SessionDir = $env:TUNNELYARD_SESSION_DIR, [bool]$ProbeService = $true)
# OpenConnect vpnc-script environment. Only manage settings owned by this tunnel.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$env:TUNNELYARD_SESSION_DIR = $SessionDir
$stateFile = Join-Path $env:TUNNELYARD_SESSION_DIR 'network-state.json'
$utf8 = New-Object Text.UTF8Encoding($false)
[Console]::OutputEncoding = $utf8
function Get-OtherRoutes {
    $root = Split-Path -Parent $env:TUNNELYARD_SESSION_DIR
    # TEMP may use an 8.3 path (RUNNER~1, for example), while enumeration
    # returns long paths. Compare canonical directory names within this root
    # so our own state cannot be mistaken for another active session.
    $currentName = (Get-Item -LiteralPath $env:TUNNELYARD_SESSION_DIR).Name
    foreach ($dir in Get-ChildItem -LiteralPath $root -Directory -Filter 'session-*') {
        $file = Join-Path $dir.FullName 'network-state.json'
        if ($dir.Name -ne $currentName -and (Test-Path -LiteralPath $file)) {
            try { (Get-Content -LiteralPath $file -Raw -Encoding UTF8 | ConvertFrom-Json).routes } catch { }
        }
    }
}
function Other-UsesRoute([string]$prefix, [int]$index, [string]$nextHop) {
    return [bool]@(Get-OtherRoutes | Where-Object { $_.prefix -eq $prefix -and $_.index -eq $index -and $_.nextHop -eq $nextHop }).Count
}
function Save-State { [IO.File]::WriteAllText($stateFile, ($script:state | ConvertTo-Json -Depth 8), $utf8) }
function Assert-Network($saved, [bool]$probe = $false) {
    $adapter = Get-NetAdapter -IncludeHidden | Where-Object { $_.ifIndex -eq $saved.index } | Select-Object -First 1
    if (!$adapter -or $adapter.InterfaceGuid.ToString() -ne $saved.guid -or [string]$adapter.Status -ne 'Up') { throw 'VPN adapter is absent or down.' }
    $iface = Get-NetIPInterface -InterfaceIndex $saved.index -AddressFamily IPv4
    if (!$iface -or [string]$iface.ConnectionState -ne 'Connected') { throw 'VPN IP interface is disconnected.' }
    if ([int]$iface.NlMtu -lt 576 -or [int]$iface.NlMtu -gt [int]$saved.mtu) { throw "VPN MTU mismatch: adapter=$($iface.NlMtu), negotiated=$($saved.mtu)." }
    $address = Get-NetIPAddress -InterfaceIndex $saved.index -IPAddress $saved.ip -ErrorAction SilentlyContinue
    if (!$address -or [string]$address.AddressState -notin @('Preferred', 'Deprecated')) { throw 'VPN IP address is not usable.' }
    foreach ($route in $saved.expectedRoutes) {
        if (!(Get-NetRoute -DestinationPrefix $route.prefix -InterfaceIndex $route.index -NextHop $route.nextHop -ErrorAction SilentlyContinue)) { throw "VPN route is missing: $($route.prefix)." }
    }
    if ($probe -and $saved.healthHost) {
        $route = Find-NetRoute -RemoteIPAddress $saved.healthHost | Where-Object { $_.NextHop } | Select-Object -First 1
        if (!$route -or $route.InterfaceIndex -ne $saved.index) { throw 'Health-check traffic would bypass the VPN adapter.' }
        $tcp = New-Object Net.Sockets.TcpClient
        try {
            $tcp.Client.Bind((New-Object Net.IPEndPoint([Net.IPAddress]::Parse($saved.ip), 0)))
            $pending = $tcp.BeginConnect([string]$saved.healthHost, [int]$saved.healthPort, $null, $null)
            try {
                if (!$pending.AsyncWaitHandle.WaitOne(3000)) { throw 'VPN service reachability check timed out.' }
                $tcp.EndConnect($pending)
            } finally { $pending.AsyncWaitHandle.Close() }
        } catch {
            $_.Exception.Data['healthCategory'] = 'service'
            throw
        } finally { $tcp.Close() }
    }
}
function Configure-Dns {
    if (!$script:state.setDns -or $script:state.dnsConfigured) { return }
    $splitFile = Join-Path $SessionDir 'split-dns.json'
    $groups = @()
    if (Test-Path -LiteralPath $splitFile) { $groups = @(Get-Content -LiteralPath $splitFile -Raw -Encoding UTF8 | ConvertFrom-Json) }
    if (!$groups.Count -and $script:state.splitDomains) { $groups = @(@{ domains=@($script:state.splitDomains -split ','); servers=@($script:state.vpnDns) }) }
    if ($groups.Count) {
        foreach ($group in $groups) {
            $servers = @($group.servers | ForEach-Object { $ip=[Net.IPAddress]::Parse($_); if ($ip.AddressFamily -ne 'InterNetwork') { throw 'Only IPv4 split-DNS servers are supported.' }; $ip.ToString() } | Select-Object -Unique)
            if (!$servers.Count) { throw 'Split-DNS has no valid servers.' }
            foreach ($domain in $group.domains) {
                if ($domain.Length -gt 253 -or $domain -notmatch '^(?=.{1,253}$)[a-zA-Z0-9](?:[a-zA-Z0-9.-]*[a-zA-Z0-9])?$' -or $domain.Contains('..')) { throw 'Invalid split-DNS domain.' }
                $namespace = @($domain, ".$domain")
                $existing = @(Get-DnsClientNrptRule | Where-Object { @($_.Namespace | Where-Object { $_ -in $namespace }).Count })
                foreach ($rule in $existing) {
                    if ((@($rule.NameServers -split '[,; ]+' | Where-Object { $_ } | Sort-Object) -join ',') -ne (($servers | Sort-Object) -join ',')) { throw "Conflicting DNS policy already exists for $domain." }
                }
                $covered = @($existing | ForEach-Object { $_.Namespace } | Select-Object -Unique)
                $missing = @($namespace | Where-Object { $_ -notin $covered })
                if ($existing.Count) {
                    # Preserve pre-existing policy. Our own rules may be shared
                    # by simultaneous sessions and are removed by the last user.
                    foreach ($rule in $existing) { $script:state.nrpt += @{ name=$rule.Name; owner=$rule.DisplayName } }
                    Save-State
                }
                if (!$missing.Count) { continue }
                $display = 'TunnelYard-' + (Get-Item -LiteralPath $SessionDir).Name + '-' + $domain
                $script:state.nrpt += @{ name=''; owner=$display }
                Save-State
                $rule = Add-DnsClientNrptRule -Namespace $missing -NameServers $servers -DisplayName $display -Comment 'Managed by TunnelYard; removed when the tunnel disconnects.' -PassThru
                $script:state.nrpt[-1].name = $rule.Name
                Save-State
                if (!(Get-DnsClientNrptRule -Name $rule.Name)) { throw 'Could not verify the split-DNS policy.' }
            }
        }
    } else {
        $servers = @($script:state.vpnDns)
        if (!$servers.Count) { throw 'set-dns=1 but no VPN DNS servers were received.' }
        $script:state.dnsChanged = $true
        Save-State
        Set-DnsClientServerAddress -InterfaceIndex $script:state.index -ServerAddresses $servers
        Set-NetIPInterface -InterfaceIndex $script:state.index -AddressFamily IPv4 -AutomaticMetric Disabled -InterfaceMetric 1
        if ($script:state.vpnSuffix) { Set-DnsClient -InterfaceIndex $script:state.index -ConnectionSpecificSuffix $script:state.vpnSuffix }
    }
    $script:state.dnsConfigured = $true
    Save-State
}
function Add-OwnedRoute([string]$prefix, [int]$index, [string]$nextHop) {
    $script:state.expectedRoutes += @{ prefix=$prefix; index=$index; nextHop=$nextHop }
    Save-State
    $exists = Get-NetRoute -DestinationPrefix $prefix -InterfaceIndex $index -NextHop $nextHop -ErrorAction SilentlyContinue
    $shared = $exists -and (Other-UsesRoute $prefix $index $nextHop)
    if (!$exists -or $shared) {
        # Record intent first so a partial failure can still be rolled back.
        # A route already provided by another My VPN session is referenced but
        # never owned by this session. This keeps the final disconnect from
        # deleting a route that was present before this tunnel started.
        $script:state.routes += @{ prefix=$prefix; index=$index; nextHop=$nextHop; owned=(!$exists) }
        Save-State
        if (!$exists) { New-NetRoute -DestinationPrefix $prefix -InterfaceIndex $index -NextHop $nextHop -RouteMetric 1 -PolicyStore ActiveStore | Out-Null }
    }
}
function Restore-Network {
    if (!(Test-Path -LiteralPath $stateFile)) { return }
    $saved = Get-Content -LiteralPath $stateFile -Raw -Encoding UTF8 | ConvertFrom-Json
    foreach ($entry in $saved.nrpt) {
        if ($entry.owner -notlike 'TunnelYard-session-*') { continue }
        $shared = $false
        foreach ($dir in Get-ChildItem -LiteralPath (Split-Path -Parent $SessionDir) -Directory -Filter 'session-*') {
            if ($dir.Name -eq (Get-Item -LiteralPath $SessionDir).Name) { continue }
            $other = Join-Path $dir.FullName 'network-state.json'
            if (Test-Path -LiteralPath $other) { if (@((Get-Content -LiteralPath $other -Raw -Encoding UTF8 | ConvertFrom-Json).nrpt | Where-Object { $_.owner -eq $entry.owner }).Count) { $shared = $true } }
        }
        if (!$shared) { Get-DnsClientNrptRule | Where-Object DisplayName -eq $entry.owner | Remove-DnsClientNrptRule -Force -ErrorAction Stop }
    }
    $adapter = Get-NetAdapter -IncludeHidden | Where-Object { $_.ifIndex -eq $saved.index } | Select-Object -First 1
    foreach ($route in $saved.routes) {
        if ($null -ne $route.owned -and !$route.owned) { continue }
        if ($route.index -eq $saved.index -and (!$adapter -or $adapter.InterfaceGuid.ToString() -ne $saved.guid)) { continue }
        if (Other-UsesRoute $route.prefix $route.index $route.nextHop) { continue }
        Get-NetRoute -DestinationPrefix $route.prefix -InterfaceIndex $route.index -NextHop $route.nextHop -ErrorAction SilentlyContinue |
            Remove-NetRoute -Confirm:$false -ErrorAction Stop
    }
    # A disabled/removed Wintun device can still appear in Get-NetAdapter after
    # its IP interface is gone. There is nothing left to restore in that case.
    $ipInterface = if ($adapter -and $adapter.InterfaceGuid.ToString() -eq $saved.guid) {
        Get-NetIPInterface -InterfaceIndex $saved.index -AddressFamily IPv4 -ErrorAction SilentlyContinue
    }
    if ($ipInterface) {
        if ($saved.mtuChanged) { Set-NetIPInterface -InterfaceIndex $saved.index -AddressFamily IPv4 -NlMtuBytes $saved.originalMtu -PolicyStore ActiveStore }
        if ($saved.dnsChanged) {
            if (@($saved.dns).Count) { Set-DnsClientServerAddress -InterfaceIndex $saved.index -ServerAddresses @($saved.dns) }
            else { Set-DnsClientServerAddress -InterfaceIndex $saved.index -ResetServerAddresses }
            Set-DnsClient -InterfaceIndex $saved.index -ConnectionSpecificSuffix ([string]$saved.suffix)
            if ($saved.automaticMetric -eq 'Enabled') { Set-NetIPInterface -InterfaceIndex $saved.index -AddressFamily IPv4 -AutomaticMetric Enabled }
            else { Set-NetIPInterface -InterfaceIndex $saved.index -AddressFamily IPv4 -AutomaticMetric Disabled -InterfaceMetric $saved.metric }
        }
        if ($saved.ipAdded) {
            Get-NetIPAddress -InterfaceIndex $saved.index -IPAddress $saved.ip -ErrorAction SilentlyContinue |
                Remove-NetIPAddress -Confirm:$false -ErrorAction Stop
        }
    }
    Remove-Item -LiteralPath $stateFile
}
$mutex = New-Object Threading.Mutex($false, 'Global\TunnelYardNetwork-v1')
$locked = $false
try {
    # The vpnc callback can have printed NETWORK_READY while its process is
    # still flushing stdout and releasing the same mutex. Give that short
    # hand-off time to finish instead of tearing down a valid tunnel.
    try { $locked = $mutex.WaitOne(15000) } catch [Threading.AbandonedMutexException] { $locked = $true }
    if (!$locked) { throw 'Timed out waiting for another VPN network operation.' }
    if ($Action -eq 'disconnect') { Restore-Network; return }
    if ($Action -in @('check', 'ready')) {
        $script:state = Get-Content -LiteralPath $stateFile -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($Action -eq 'ready') { Configure-Dns }
        else { Assert-Network $script:state $ProbeService }
        return [pscustomobject]@{ ok=$true; message=''; mtu=$script:state.mtu; splitDnsRules=@($script:state.nrpt).Count }
    }
    if ($Action -notin @('connect', 'reconnect')) { return }
    if ($Action -eq 'reconnect') { Restore-Network }
    $index = [int]$env:TUNIDX
    $adapter = Get-NetAdapter -IncludeHidden | Where-Object { $_.ifIndex -eq $index } | Select-Object -First 1
    if (!$adapter) { throw 'VPN adapter not found.' }
    $ip = [Net.IPAddress]::Parse($env:INTERNAL_IP4_ADDRESS)
    if ($ip.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork) { throw 'An IPv4 tunnel address is required.' }
    $mtu = 0
    if (![int]::TryParse($env:INTERNAL_IP4_MTU, [ref]$mtu) -or $mtu -lt 576 -or $mtu -gt 65535) { throw 'OpenConnect did not provide a valid negotiated IPv4 MTU.' }
    $script:state = @{ index=$index; guid=$adapter.InterfaceGuid.ToString(); routes=@(); expectedRoutes=@(); ip=$ip.ToString(); ipAdded=$false;
        mtu=$mtu; originalMtu=0; mtuChanged=$false; nrpt=@(); dnsConfigured=$false; setDns=($env:TUNNELYARD_SET_DNS -eq '1');
        vpnDns=@($env:INTERNAL_IP4_DNS -split '\s+' | Where-Object { $_ } | ForEach-Object { [Net.IPAddress]::Parse($_).ToString() });
        vpnSuffix=$env:CISCO_DEF_DOMAIN; splitDomains=$env:CISCO_SPLIT_DNS; healthHost=$env:TUNNELYARD_HEALTH_HOST; healthPort=$env:TUNNELYARD_HEALTH_PORT;
        dns=@((Get-DnsClientServerAddress -InterfaceIndex $index -AddressFamily IPv4).ServerAddresses);
        suffix=(Get-DnsClient -InterfaceIndex $index).ConnectionSpecificSuffix; dnsChanged=$false }
    $ipInterface = Get-NetIPInterface -InterfaceIndex $index -AddressFamily IPv4
    $script:state.metric = [int]$ipInterface.InterfaceMetric
    $script:state.automaticMetric = [string]$ipInterface.AutomaticMetric
    $script:state.originalMtu = [int]$ipInterface.NlMtu
    Save-State
    $setRoutes = $env:TUNNELYARD_SET_ROUTES -eq '1'
    # Apply negotiated MTU before assigning a tunnel IP or installing routes.
    # This is the IP MTU (already excludes TLS/DTLS/PPP overhead), not a fixed
    # Ethernet default and not a value inferred from a prior session's logs.
    $script:state.mtuChanged = $true
    Save-State
    Set-NetIPInterface -InterfaceIndex $index -AddressFamily IPv4 -NlMtuBytes $mtu -PolicyStore ActiveStore
    $effectiveMtu = [int](Get-NetIPInterface -InterfaceIndex $index -AddressFamily IPv4).NlMtu
    if ($effectiveMtu -lt 576 -or $effectiveMtu -gt $mtu) { throw "Adapter did not apply negotiated MTU $mtu (effective $effectiveMtu)." }
    Write-Output "VPN MTU applied: negotiated=$mtu, effective=$effectiveMtu"
    if ($setRoutes) {
        $gateway = [Net.IPAddress]::Parse($env:VPNGATEWAY)
        $transport = Find-NetRoute -RemoteIPAddress $gateway.ToString() | Where-Object { $_.NextHop } | Select-Object -First 1
        if (!$transport) { throw 'Could not determine the original route to the VPN gateway.' }
        $bits = if ($gateway.AddressFamily -eq [Net.Sockets.AddressFamily]::InterNetwork) { 32 } else { 128 }
        Add-OwnedRoute ($gateway.ToString() + '/' + $bits) $transport.InterfaceIndex $transport.NextHop
    }
    if (!(Get-NetIPAddress -InterfaceIndex $index -IPAddress $ip.ToString() -ErrorAction SilentlyContinue)) {
        $script:state.ipAdded = $true
        Save-State
        New-NetIPAddress -InterfaceIndex $index -IPAddress $ip.ToString() -PrefixLength 32 -PolicyStore ActiveStore | Out-Null
    }
    if ($setRoutes) {
        if ($env:CISCO_SPLIT_INC) {
            for ($i=0; $i -lt [int]$env:CISCO_SPLIT_INC; $i++) {
                $address = [Net.IPAddress]::Parse([Environment]::GetEnvironmentVariable("CISCO_SPLIT_INC_${i}_ADDR"))
                $mask = [int][Environment]::GetEnvironmentVariable("CISCO_SPLIT_INC_${i}_MASKLEN")
                if ($mask -lt 0 -or $mask -gt 32) { throw 'Invalid VPN route prefix.' }
                Add-OwnedRoute ($address.ToString() + '/' + $mask) $index '0.0.0.0'
            }
        } else {
            Add-OwnedRoute '0.0.0.0/1' $index '0.0.0.0'
            Add-OwnedRoute '128.0.0.0/1' $index '0.0.0.0'
        }
        for ($i=0; $i -lt [int]$env:CISCO_SPLIT_EXC; $i++) {
            $address = [Net.IPAddress]::Parse([Environment]::GetEnvironmentVariable("CISCO_SPLIT_EXC_${i}_ADDR"))
            $mask = [int][Environment]::GetEnvironmentVariable("CISCO_SPLIT_EXC_${i}_MASKLEN")
            if ($mask -lt 0 -or $mask -gt 32) { throw 'Invalid VPN excluded route prefix.' }
            Add-OwnedRoute ($address.ToString() + '/' + $mask) $transport.InterfaceIndex $transport.NextHop
        }
    }
    # OpenConnect waits at most 10s for this script. DNS and service probes run
    # in the supervisor afterwards, once the client's packet loop is running.
    Write-Output 'TUNNELYARD_NETWORK_READY'
} catch {
    if ($Action -in @('check', 'ready')) { return [pscustomobject]@{ ok=$false; message=$_.Exception.Message; category= $(if ($_.Exception.Data['healthCategory']) { 'service' } else { 'network' }) } }
    [Console]::Error.WriteLine("ERROR: VPN network configuration: " + $_.Exception.Message)
    [IO.File]::WriteAllText((Join-Path $env:TUNNELYARD_SESSION_DIR 'stop'), '', $utf8)
    if ($locked) { try { Restore-Network } catch { [Console]::Error.WriteLine("ERROR: VPN cleanup: " + $_.Exception.Message) } }
    exit 1
} finally {
    if ($locked) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}

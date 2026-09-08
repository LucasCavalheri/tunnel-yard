param([string]$SessionDir, [int]$Dns, [int]$Routes, [switch]$FailRoute, [switch]$Shared, [switch]$Preexisting, [switch]$ShortPath,
    [int]$Mtu=1351, [switch]$RejectMtu, [switch]$SplitDns, [switch]$DnsConflict, [switch]$ServiceFailure, [string]$Drop)
$ErrorActionPreference = 'Stop'
if ($ShortPath) {
    $fso = New-Object -ComObject Scripting.FileSystemObject
    $SessionDir = $fso.GetFolder($SessionDir).ShortPath
}
$global:testOperations = New-Object Collections.Generic.List[object]
$global:testRoutes = @()
$global:testIp = $null
$global:testMtu = 65535
$global:testAdapterUp = $true
$global:testInterfacePresent = $true
$global:testNrpt = @()
function Get-NetAdapter { param([switch]$IncludeHidden) if ($global:testAdapterUp) { @{ ifIndex=42; Status='Up'; InterfaceGuid=[guid]'12345678-1234-1234-1234-123456789012' } } }
function Get-DnsClientServerAddress { param($InterfaceIndex,$AddressFamily) @{ ServerAddresses=@('192.0.2.53') } }
function Get-DnsClient { param($InterfaceIndex) @{ ConnectionSpecificSuffix='before.test' } }
function Get-NetIPInterface { param($InterfaceIndex,$AddressFamily,$ErrorAction) if ($global:testAdapterUp -and $global:testInterfacePresent) { @{ InterfaceMetric=25; AutomaticMetric='Enabled'; NlMtu=$global:testMtu; ConnectionState='Connected' } } }
function Set-NetIPInterface {
    param($InterfaceIndex,$AddressFamily,$AutomaticMetric,$InterfaceMetric,$NlMtuBytes,$PolicyStore)
    if ($NlMtuBytes) {
        if (!$RejectMtu) { $global:testMtu=$NlMtuBytes }
        $global:testOperations.Add(@{action='mtu';value=$NlMtuBytes})
    } else { $global:testOperations.Add(@{action='metric';value=$InterfaceMetric}) }
}
function Get-DnsClientNrptRule { param($Name) $global:testNrpt | Where-Object { !$Name -or $_.Name -eq $Name } }
function Add-DnsClientNrptRule {
    param($Namespace,$NameServers,$DisplayName,$Comment,[switch]$PassThru)
    $rule=[pscustomobject]@{Name=[guid]::NewGuid().ToString();Namespace=$Namespace;NameServers=$NameServers;DisplayName=$DisplayName}
    $global:testNrpt += $rule
    $global:testOperations.Add(@{action='nrpt-add';namespaces=@($Namespace);servers=@($NameServers)})
    $rule
}
function Remove-DnsClientNrptRule {
    param([Parameter(ValueFromPipeline=$true)]$InputObject,[switch]$Force)
    process {
        $global:testNrpt=@($global:testNrpt | Where-Object Name -ne $InputObject.Name)
        $global:testOperations.Add(@{action='nrpt-remove'})
    }
}
function Find-NetRoute {
    param($RemoteIPAddress)
    if ($ServiceFailure -and $RemoteIPAddress -eq '198.18.0.2') { @{InterfaceIndex=42;NextHop='0.0.0.0'} }
    else { @{ InterfaceIndex=7; NextHop='192.0.2.1' } }
}
function Get-NetRoute {
    param($DestinationPrefix,$InterfaceIndex,$NextHop,$ErrorAction)
    $global:testRoutes | Where-Object { $_.prefix -eq $DestinationPrefix -and $_.index -eq $InterfaceIndex -and $_.nextHop -eq $NextHop }
}
function New-NetRoute {
    param($DestinationPrefix,$InterfaceIndex,$NextHop,$RouteMetric,$PolicyStore)
    if ($FailRoute -and $InterfaceIndex -eq 42) { throw 'Simulated route failure' }
    $route=@{ prefix=$DestinationPrefix; index=$InterfaceIndex; nextHop=$NextHop }
    $global:testRoutes += $route
    $global:testOperations.Add(@{ action='route-add'; prefix=$DestinationPrefix; index=$InterfaceIndex })
}
function Remove-NetRoute {
    param([Parameter(ValueFromPipeline=$true)]$InputObject,[switch]$Confirm)
    process {
        $global:testOperations.Add(@{ action='route-remove'; prefix=$InputObject.prefix; index=$InputObject.index })
        $global:testRoutes=@($global:testRoutes | Where-Object { !($_.prefix -eq $InputObject.prefix -and $_.index -eq $InputObject.index -and $_.nextHop -eq $InputObject.nextHop) })
    }
}
function Get-NetIPAddress { param($InterfaceIndex,$IPAddress,$ErrorAction) $global:testIp }
function New-NetIPAddress {
    param($InterfaceIndex,$IPAddress,$PrefixLength,$PolicyStore)
    $global:testIp=@{ ip=$IPAddress; index=$InterfaceIndex; AddressState='Preferred' }
    $global:testOperations.Add(@{ action='ip-add'; index=$InterfaceIndex })
}
function Remove-NetIPAddress {
    param([Parameter(ValueFromPipeline=$true)]$InputObject,[switch]$Confirm)
    process { $global:testIp=$null; $global:testOperations.Add(@{ action='ip-remove'; index=$InputObject.index }) }
}
function Set-DnsClientServerAddress {
    param($InterfaceIndex,$ServerAddresses,[switch]$ResetServerAddresses)
    $global:testOperations.Add(@{ action='dns'; index=$InterfaceIndex; servers=@($ServerAddresses) })
}
function Set-DnsClient { param($InterfaceIndex,$ConnectionSpecificSuffix) $global:testOperations.Add(@{ action='suffix'; value=$ConnectionSpecificSuffix }) }
$env:TUNNELYARD_SESSION_DIR=$SessionDir
$env:TUNNELYARD_SET_DNS=[string]$Dns
$env:TUNNELYARD_SET_ROUTES=[string]$Routes
$env:TUNIDX='42'
$env:VPNGATEWAY='203.0.113.5'
$env:INTERNAL_IP4_ADDRESS='198.18.0.2'
$env:INTERNAL_IP4_MTU=[string]$Mtu
$env:TUNNELYARD_HEALTH_HOST=''
$env:TUNNELYARD_HEALTH_PORT=''
if ($ServiceFailure) {
    # No OS address was created by these mocks. Binding a probe to that fake
    # VPN IP fails locally, before any SYN can leave the host.
    $env:TUNNELYARD_HEALTH_HOST='198.18.0.2'
    $env:TUNNELYARD_HEALTH_PORT='30015'
}
$env:CISCO_SPLIT_DNS=''
$env:INTERNAL_IP4_DNS='198.18.0.53 198.18.0.54'
$env:CISCO_DEF_DOMAIN='corp.test'
$env:CISCO_SPLIT_INC='1'
$env:CISCO_SPLIT_INC_0_ADDR='198.18.0.0'
$env:CISCO_SPLIT_INC_0_MASKLEN='24'
$env:CISCO_SPLIT_EXC='0'
$existingRoute=@{ prefix='198.18.0.0/24'; index=42; nextHop='0.0.0.0' }
if ($Preexisting) { $global:testRoutes += $existingRoute }
$network=Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'packaging/windows-network.ps1'
$env:reason='connect'
$connectOutput=@(& $network)
$connectedRoutes=@($global:testRoutes)
if ($SplitDns) {
    ConvertTo-Json -InputObject @(@{domains=@('corp.test');servers=@('198.18.0.53')}) -Depth 5 | Set-Content (Join-Path $SessionDir 'split-dns.json')
    if ($DnsConflict) { $global:testNrpt=@([pscustomobject]@{ Name='preexisting';Namespace=@('.corp.test');NameServers=@('192.0.2.53');DisplayName='Other VPN' }) }
}
$ready=$null
$health=$null
$brokenHealth=$null
if (Test-Path (Join-Path $SessionDir 'network-state.json')) {
    $ready=& $network -Action ready
    $health=& $network -Action check
    if ($Drop -eq 'adapter') { $global:testAdapterUp=$false }
    if ($Drop -eq 'interface') { $global:testInterfacePresent=$false; $global:testIp=$null }
    if ($Drop -eq 'route') { $global:testRoutes=@($global:testRoutes | Where-Object index -ne 42) }
    if ($Drop -eq 'ip') { $global:testIp=$null }
    if ($Drop -eq 'mtu') { $global:testMtu=1500 }
    if ($Drop) { $brokenHealth=& $network -Action check }
    $global:testAdapterUp=$true
}
if ($Shared) {
    $other=Join-Path (Split-Path -Parent $SessionDir) 'session-other'
    New-Item -ItemType Directory -Path $other -Force | Out-Null
    @{ routes=@(@{prefix='203.0.113.5/32';index=7;nextHop='192.0.2.1'}) } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $other 'network-state.json')
}
$env:reason='disconnect'
$null=& $network
$countAfterDisconnect=$global:testOperations.Count
$null=& $network
@{ connected=$connectOutput; ready=$ready; health=$health; brokenHealth=$brokenHealth; effectiveMtu=$global:testMtu; remainingNrpt=@($global:testNrpt);
   operations=@($global:testOperations.ToArray()); connectedRoutes=$connectedRoutes; remaining=@($global:testRoutes);
   duplicateCleanupChanges=($global:testOperations.Count-$countAfterDisconnect); stateExists=(Test-Path (Join-Path $SessionDir 'network-state.json')) } | ConvertTo-Json -Depth 8 -Compress

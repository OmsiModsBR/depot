<#
Runs the game once per MSAA level (or once, for a stutter capture) with OMSI_PROFILE and a private
home folder, so the user's own ~/.depot is left alone, then summarises the logs. Protocol:
docs/performance-measurement.md (#36, #43). It starts the game exactly as a player would.

  .\run-bench.ps1 -Root 'C:\OMSI 2' -Map maps/Munich/global.cfg -Bus Vehicles/X/X.bus `
      -Spawn '1000,2000,90,0' -View driver -Msaa 1,4,8 -Seconds 90 -Repeat 3
#>
param(
    [Parameter(Mandatory = $true)][string]$Root,
    [Parameter(Mandatory = $true)][string]$Map,
    [Parameter(Mandatory = $true)][string]$Bus,
    [Parameter(Mandatory = $true)][string]$Spawn,
    [string]$Exe = 'target/release/depot.exe',
    [ValidateSet('outside', 'driver')][string]$View = 'driver',
    [int[]]$Msaa = @(4),
    [int]$Seconds = 90,
    [int]$Repeat = 1,
    [int]$Traffic = 20,
    [string]$Time = '09:00',
    [string]$Date = '2017-10-07',
    [string]$Seed = '30794024',
    [switch]$Vsync,
    [switch]$GpuTimers,
    [switch]$DrawCounts,
    # extra OMSI_* switches for one-variable A/B runs, e.g. -Extra @{ OMSI_NO_MSAA_PREPASS = '1' }
    [hashtable]$Extra = @{},
    [string]$Out = 'target/perf-bench',
    [string]$Python = 'python'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$binary = if ([IO.Path]::IsPathRooted($Exe)) { $Exe } else { Join-Path $repo $Exe }
$out = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $repo $Out }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$outDir = Join-Path $out $stamp
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
$userSettings = Join-Path $env:USERPROFILE '.depot/settings.cfg'
$base = if (Test-Path $userSettings) { Get-Content $userSettings } else { @() }

$logs = @()
foreach ($level in $Msaa) {
    for ($run = 1; $run -le $Repeat; $run++) {
        $name = "msaa$level-run$run"
        $home_ = Join-Path $outDir "home-$name"
        New-Item -ItemType Directory -Path (Join-Path $home_ '.depot') -Force | Out-Null
        $over = @{ msaa = $level; vsync = $(if ($Vsync) { 1 } else { 0 }) }
        $lines = @($base | Where-Object { $key = ($_ -split '=', 2)[0]; -not $over.ContainsKey($key) })
        $lines += $over.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }
        Set-Content -Path (Join-Path $home_ '.depot/settings.cfg') -Value $lines
        $env_ = @{
            HOME = $home_; USERPROFILE = $home_
            RUST_LOG = 'info'; OMSI_PROFILE = '1'; OMSI_SEED = $Seed; OMSI_MAX_FPS = '0'
            OMSI_GPU_TIMERS = $(if ($GpuTimers) { '1' } else { $null })
            OMSI_DEBUG_DRAWS = $(if ($DrawCounts) { '1' } else { $null })
        }
        foreach ($k in $Extra.Keys) { $env_[$k] = $Extra[$k] }
        $saved = @{}
        foreach ($k in $env_.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k, 'Process') }
        try {
            foreach ($k in $env_.Keys) { [Environment]::SetEnvironmentVariable($k, $env_[$k], 'Process') }
            $arguments = @(
                '--root', ('"{0}"' -f $Root), '--no-menu', '--map', $Map, '--bus', ('"{0}"' -f $Bus),
                '--time', $Time, '--date', $Date, '--traffic', $Traffic, '--passengers', '--schedule',
                '--spawn', $Spawn, '--view', $View, '--exit-after', $Seconds
            )
            Write-Output "$name : $binary $($arguments -join ' ')"
            $p = Start-Process -FilePath $binary -ArgumentList $arguments -WorkingDirectory $repo `
                -WindowStyle Minimized -RedirectStandardOutput (Join-Path $outDir "$name.stdout.log") `
                -RedirectStandardError (Join-Path $outDir "$name.stderr.log") -PassThru
            $p.WaitForExit()
            if ($p.ExitCode -ne 0) { Write-Warning "$name exited with $($p.ExitCode)" }
        } finally {
            foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process') }
        }
        $gameLog = Join-Path $home_ '.depot/game.log'
        if (Test-Path $gameLog) { Copy-Item $gameLog (Join-Path $outDir "$name.log"); $logs += (Join-Path $outDir "$name.log") }
        else { Write-Warning "$name wrote no game.log" }
    }
}
if ($logs.Count -gt 0) {
    $summary = & $Python -I (Join-Path $PSScriptRoot 'analyze_log.py') @logs
    $summary | Set-Content (Join-Path $outDir 'summary.md')
    $summary
}
Write-Output "Logs and summary: $outDir"

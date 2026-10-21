# Installs, updates or repairs one Microsoft Store product through the Store's own install service
# (AppInstallManager, the API winget's Store source uses), with the account signed in to the Store.
# bedrock_store.rs puts $Action ('install', 'update' or 'repair') and $ProductId in front of this script.
# It prints one line per step for bedrock_store.rs to read; no double quotes here (see bedrock_store.rs).
$ErrorActionPreference = 'Stop'
function Say($s) { [Console]::Out.WriteLine($s); [Console]::Out.Flush() }
function Hex($e) { '0x{0:X8}' -f $e.HResult }
try {
  Add-Type -AssemblyName System.Runtime.WindowsRuntime
  $null = [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager, Windows.ApplicationModel.Store.Preview, ContentType = WindowsRuntime]
  $null = [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallItem, Windows.ApplicationModel.Store.Preview, ContentType = WindowsRuntime]
  $null = [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallOptions, Windows.ApplicationModel.Store.Preview, ContentType = WindowsRuntime]
  $null = [Windows.ApplicationModel.Store.Preview.InstallControl.AppUpdateOptions, Windows.ApplicationModel.Store.Preview, ContentType = WindowsRuntime]
} catch { Say 'nostore'; exit }
try {
  $asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
  function Await($op, [type]$t) { $task = $asTask.MakeGenericMethod($t).Invoke($null, @($op)); $task.Wait(-1) | Out-Null; $task.Result }
  $itemType = [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallItem]
  $m = [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager]::new()
  if (-not (Await ($m.GetIsAppAllowedToInstallAsync($ProductId)) ([bool]))) { Say 'blocked'; exit }
  if ($Action -eq 'update') {
    $o = [Windows.ApplicationModel.Store.Preview.InstallControl.AppUpdateOptions]::new()
    $o.AutomaticallyDownloadAndInstallUpdateIfFound = $true
    $found = Await ($m.SearchForUpdatesAsync($ProductId, '', '', 'ResinCraft', $o)) $itemType
    if ($null -eq $found) { Say 'uptodate'; exit }
    $items = @($found)
  } else {
    $o = [Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallOptions]::new()
    $o.Repair = ($Action -eq 'repair')
    $list = Await ($m.StartProductInstallAsync($ProductId, '', 'ResinCraft', '', $o)) ([System.Collections.Generic.IReadOnlyList`1].MakeGenericType($itemType))
    $items = @($list)
  }
  if ($items.Count -eq 0) { Say 'uptodate'; exit }
  # The list can hold what the game needs first (Gaming Services); the game itself is the item with its product id.
  $main = $items | Where-Object { $_.ProductId -eq $ProductId } | Select-Object -First 1
  if ($null -eq $main) { $main = $items[$items.Count - 1] }
  Say ('started ' + $items.Count)
  while ($true) {
    foreach ($i in $items) {
      if ($i -eq $main) { continue }
      $d = $i.GetCurrentStatus()
      if ('Completed', 'Canceled', 'Error' -notcontains [string]$d.InstallState) { Say ('dep ' + $d.InstallState + ' ' + [int]$d.PercentComplete) }
    }
    $s = $main.GetCurrentStatus()
    Say ('status ' + $s.InstallState + ' ' + [int]$s.PercentComplete + ' ' + $s.BytesDownloaded + ' ' + $s.DownloadSizeInBytes)
    switch ([string]$s.InstallState) {
      'Completed' { Say 'done'; exit }
      'Canceled' { Say 'canceled'; exit }
      'Error' {
        if ($null -eq $s.ErrorCode) { Say 'error 0x80004005 ' } else { Say ('error ' + (Hex $s.ErrorCode) + ' ' + ($s.ErrorCode.Message -replace '\s+', ' ')) }
        exit
      }
    }
    Start-Sleep -Milliseconds 500
  }
} catch {
  $e = $_.Exception
  while ($null -ne $e.InnerException) { $e = $e.InnerException }
  Say ('error ' + (Hex $e) + ' ' + ($e.Message -replace '\s+', ' '))
}

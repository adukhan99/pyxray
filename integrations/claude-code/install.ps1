# Wire pyxray into Claude Code's settings on Windows, without the plugin system.
#
#   .\integrations\claude-code\install.ps1            .claude\settings.json (project)
#   .\integrations\claude-code\install.ps1 -Global    ~\.claude\settings.json
#
# Claude Code on Windows runs hook commands through the Git Bash `sh` it also
# uses for its Bash tool, so the same launcher works; this just writes the
# path with forward slashes and points PYXRAY_PYTHON at your interpreter.
# Prefer the plugin when you can: /plugin marketplace add adukhan99/pyxray
param([switch]$Global)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$hook = (Join-Path $here "hooks\pyxray-hook") -replace "\\", "/"
$command = "sh `"$hook`""

$target = if ($Global) { Join-Path $HOME ".claude\settings.json" } else { ".claude\settings.json" }
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null

$settings = @{}
if (Test-Path $target) {
    try { $settings = Get-Content $target -Raw | ConvertFrom-Json -AsHashtable }
    catch { Write-Error "pyxray: $target is not valid JSON; not touching it"; exit 1 }
    if ($null -eq $settings) { $settings = @{} }
}
if (-not $settings.ContainsKey("hooks")) { $settings["hooks"] = @{} }
if (-not $settings["hooks"].ContainsKey("PreToolUse")) { $settings["hooks"]["PreToolUse"] = @() }

$entries = [System.Collections.ArrayList]@($settings["hooks"]["PreToolUse"])
foreach ($matcher in @("Bash", "NotebookEdit")) {
    $existing = $entries | Where-Object { $_["matcher"] -eq $matcher } | Select-Object -First 1
    $hookEntry = @{ type = "command"; command = $command; timeout = 5 }
    if ($existing) {
        $list = [System.Collections.ArrayList]@($existing["hooks"])
        $ours = $list | Where-Object { "$($_["command"])" -like "*pyxray-hook*" } | Select-Object -First 1
        if ($ours) { $ours["command"] = $command; $ours["timeout"] = 5 } else { [void]$list.Add($hookEntry) }
        $existing["hooks"] = @($list)
    } else {
        [void]$entries.Add(@{ matcher = $matcher; hooks = @($hookEntry) })
    }
}
$settings["hooks"]["PreToolUse"] = @($entries)

# Tell the launcher which interpreter has pyxray installed.
$python = (Get-Command python -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if ($python) {
    if (-not $settings.ContainsKey("env")) { $settings["env"] = @{} }
    if (-not $settings["env"].ContainsKey("PYXRAY_PYTHON")) { $settings["env"]["PYXRAY_PYTHON"] = ($python -replace "\\", "/") }
}

$settings | ConvertTo-Json -Depth 10 | Set-Content -Path $target -Encoding UTF8
Write-Host "pyxray: wired into $target"
Write-Host "now run:  pyx watch"

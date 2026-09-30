# PowerShell 7：验证完整 Skill 文件包与权限，不调用模型或改动已有包。
param([string]$Base = 'http://127.0.0.1:3001')
$ErrorActionPreference = 'Stop'
$config = Get-Content (Join-Path $PSScriptRoot '..\.env') | ConvertFrom-StringData
$session = New-Object Microsoft.PowerShell.Commands.WebRequestSession
$login = Invoke-RestMethod -Uri "$Base/api/v1/admin/login" -Method Post -ContentType 'application/json' -Body (@{ username = $config.ADMIN_USERNAME; password = $config.ADMIN_PASSWORD } | ConvertTo-Json) -WebSession $session
$headers = @{ 'X-CSRF-Token' = $login.csrf_token }
$endpoint = "$Base/api/v1/admin/agent/skills"
$name = "smoke-package-$([guid]::NewGuid().ToString('N').Substring(0, 10))"
$zipPath = Join-Path ([System.IO.Path]::GetTempPath()) "$name.zip"
$installed = $false
$stage = 'prepare package'

# JSON 错误请求检查确切状态，禁止将认证失败当作业务测试成功。
function Assert-Status([int]$Expected, [string]$Method, [string]$Uri, [hashtable]$Body, [bool]$Authenticated = $true) {
    $parameters = @{ Uri = $Uri; Method = $Method; SkipHttpErrorCheck = $true }
    if ($Authenticated) { $parameters.WebSession = $session; $parameters.Headers = $headers }
    if ($Body) { $parameters.Body = $Body | ConvertTo-Json; $parameters.ContentType = 'application/json' }
    $response = Invoke-WebRequest @parameters
    if ([int]$response.StatusCode -ne $Expected) { throw "Expected $Expected, received $($response.StatusCode)" }
}

try {
    $document = "---" + [Environment]::NewLine + "name: $name" + [Environment]::NewLine + 'description: Use this package for smoke testing' + [Environment]::NewLine + '---' + [Environment]::NewLine + '# Instructions'
    $stream = [System.IO.File]::Create($zipPath)
    $zip = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in @{
            "$name/SKILL.md" = [System.Text.Encoding]::UTF8.GetBytes($document)
            "$name/references/info.md" = [System.Text.Encoding]::UTF8.GetBytes('reference body')
            "$name/scripts/check.py" = [System.Text.Encoding]::UTF8.GetBytes("print('smoke')")
            "$name/assets/blob.bin" = [byte[]]@(0, 255, 12)
        }.GetEnumerator()) {
            $entry = $zip.CreateEntry($file.Key); $output = $entry.Open()
            try { $output.Write($file.Value) } finally { $output.Dispose() }
        }
    } finally { $zip.Dispose(); $stream.Dispose() }
    $stage = 'auth and CSRF'
    Assert-Status 401 GET $endpoint $null $false
    $noCsrf = Invoke-WebRequest -Uri "$endpoint/install" -Method Post -WebSession $session -Form @{ file = Get-Item $zipPath } -SkipHttpErrorCheck
    if ($noCsrf.StatusCode -ne 403) { throw 'Missing CSRF was accepted' }

    $stage = 'install complete package'
    $created = Invoke-RestMethod -Uri "$endpoint/install" -Method Post -WebSession $session -Headers $headers -Form @{ file = Get-Item $zipPath }
    $installed = $true
    if ($created.skill.name -ne $name -or $created.files.Count -ne 4 -or $created.skill.enabled) { throw 'Package files or default status incorrect' }
    $catalog = Invoke-RestMethod -Uri $endpoint -WebSession $session
    $metadata = $catalog.skills | Where-Object { $_.name -eq $name }
    if (!$metadata -or $metadata.PSObject.Properties.Name -contains 'document') { throw 'Catalog must contain only metadata' }
    $duplicate = Invoke-WebRequest -Uri "$endpoint/install" -Method Post -Headers $headers -WebSession $session -Form @{ file = Get-Item $zipPath } -SkipHttpErrorCheck
    if ($duplicate.StatusCode -ne 400) { throw 'Duplicate package was silently overwritten' }

    $stage = 'binary resource and download safety'
    $resource = Invoke-WebRequest -Uri "$endpoint/$name/files?path=assets%2Fblob.bin" -WebSession $session
    if ([Convert]::ToHexString([byte[]]$resource.Content) -ne '00FF0C') { throw 'Binary asset changed' }
    if ($resource.Headers['X-Content-Type-Options'] -ne 'nosniff') { throw 'Resource MIME safety missing' }
    Assert-Status 400 GET "$endpoint/$name/files?path=..%2F.state.json" $null

    $stage = 'status and raw document editing'
    Assert-Status 204 PATCH "$endpoint/$name/enabled" @{ enabled = $true }
    $detail = Invoke-RestMethod -Uri "$endpoint/$name" -WebSession $session
    if (!$detail.skill.enabled) { throw 'Enable status not persisted' }
    $updated = Invoke-RestMethod -Uri "$endpoint/$name" -Method Put -Headers $headers -WebSession $session -ContentType 'application/json' -Body (@{ document = $document + [Environment]::NewLine + 'Updated body' } | ConvertTo-Json)
    if ($updated.files.Count -ne 4) { throw 'Editing SKILL.md lost package files' }
    Assert-Status 400 PUT "$endpoint/$name" @{ document = $document.Replace($name, 'changed-name') }
    Assert-Status 400 POST "$endpoint/install/github" @{ url = 'https://localhost/repo'; overwrite = $false }

    $stage = 'export full ZIP and uninstall'
    $exported = Invoke-WebRequest -Uri "$endpoint/$name/export" -WebSession $session
    $memory = [System.IO.MemoryStream]::new([byte[]]$exported.Content)
    $archive = [System.IO.Compression.ZipArchive]::new($memory, [System.IO.Compression.ZipArchiveMode]::Read)
    try { if ($archive.Entries.Count -ne 4) { throw 'Export lost package files' } } finally { $archive.Dispose(); $memory.Dispose() }
    Assert-Status 204 DELETE "$endpoint/$name" $null
    $installed = $false
    Assert-Status 404 GET "$endpoint/$name" $null
    Write-Output 'Skill package smoke passed: auth, CSRF, ZIP, files, binary asset, metadata, status, editing, export, paths, uninstall'
}
catch { throw "Skill package smoke failed at $stage : $_" }
finally {
    if ($installed) {
        try { Invoke-RestMethod -Uri "$endpoint/$name" -Method Delete -Headers $headers -WebSession $session | Out-Null }
        catch { Write-Warning "临时技能包清理失败: $name" }
    }
    if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath }
}

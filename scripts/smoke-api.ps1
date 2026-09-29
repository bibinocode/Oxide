# 在本机开发数据库验证 API 主流程；临时文章和分类在 finally 中清理。
param([string]$Base = 'http://127.0.0.1:3001')

$ErrorActionPreference = 'Stop'
$config = Get-Content (Join-Path $PSScriptRoot '..\.env') | ConvertFrom-StringData
$session = New-Object Microsoft.PowerShell.Commands.WebRequestSession
$loginBody = @{ username = $config.ADMIN_USERNAME; password = $config.ADMIN_PASSWORD } | ConvertTo-Json
$login = Invoke-RestMethod -Uri "$base/api/v1/admin/login" -Method Post -ContentType 'application/json' -Body $loginBody -WebSession $session
$headers = @{ 'X-CSRF-Token' = $login.csrf_token }
$suffix = [guid]::NewGuid().ToString('N').Substring(0, 10)
$slug = "smoke-$suffix"
$articleId = $null
$categoryCreated = $false
$stage = 'create category'

try {
    $categoryBody = @{ name = '冒烟测试'; slug = $slug } | ConvertTo-Json
    Invoke-RestMethod -Uri "$base/api/v1/admin/categories" -Method Post -ContentType 'application/json' -Headers $headers -Body $categoryBody -WebSession $session | Out-Null
    $categoryCreated = $true

    $stage = 'create article'
    $document = @{ type = 'doc'; content = @(@{ type = 'paragraph'; content = @(@{ type = 'text'; text = '中文分词索引验证' }) }) }
    $articleBody = @{ title = '中文分词冒烟测试'; slug = $slug; summary = '临时测试文章'; document = $document } | ConvertTo-Json -Depth 10
    $created = Invoke-RestMethod -Uri "$base/api/v1/admin/articles" -Method Post -ContentType 'application/json' -Headers $headers -Body $articleBody -WebSession $session
    $articleId = $created.public_id
    if ($created.PSObject.Properties.Name -contains 'id') { throw '文章响应暴露内部 ID' }

    $stage = 'taxonomy and publish'
    $taxonomy = @{ categories = @($slug); tags = @() } | ConvertTo-Json
    Invoke-RestMethod -Uri "$base/api/v1/admin/articles/$articleId/taxonomy" -Method Put -ContentType 'application/json' -Headers $headers -Body $taxonomy -WebSession $session | Out-Null
    Invoke-RestMethod -Uri "$base/api/v1/admin/articles/$articleId/publish" -Method Post -Headers $headers -WebSession $session | Out-Null

    $stage = 'public article and category'
    $public = Invoke-RestMethod -Uri "$base/api/v1/articles/$slug"
    if ($public.public_id -ne $articleId -or $public.rendered_html -notmatch '中文分词索引验证') { throw '公开文章内容不正确' }
    $categoryPage = Invoke-RestMethod -Uri "$base/api/v1/categories/$slug/articles"
    if ($categoryPage.total -ne 1) { throw '分类文章未关联' }

    $stage = 'search index'
    $found = $false
    for ($attempt = 0; $attempt -lt 10; $attempt++) {
        $results = Invoke-RestMethod -Uri "$base/api/v1/search?q=%E5%88%86%E8%AF%8D"
        if ($results.items.public_id -contains $articleId) { $found = $true; break }
        Start-Sleep -Seconds 1
    }
    if (-not $found) { throw '搜索索引未收录文章' }

    $stage = 'comment submission'
    $commentBody = @{ nickname = '访客'; email = 'smoke@example.invalid'; body = '测试评论'; parent_public_id = $null } | ConvertTo-Json
    $comment = Invoke-RestMethod -Uri "$base/api/v1/articles/$slug/comments" -Method Post -ContentType 'application/json' -Body $commentBody
    if ($comment.status -ne 'pending') { throw '评论未进入待审核状态' }
    $stage = 'comment hidden check'
    $before = Invoke-RestMethod -Uri "$base/api/v1/articles/$slug/comments"
    if (@($before).Count -ne 0) { throw '待审核评论被公开' }
    $stage = 'comment review'
    $reviewBody = @{ status = 'approved' } | ConvertTo-Json
    Invoke-RestMethod -Uri "$base/api/v1/admin/comments/$($comment.public_id)" -Method Patch -ContentType 'application/json' -Headers $headers -Body $reviewBody -WebSession $session | Out-Null
    $stage = 'comment public check'
    $after = Invoke-RestMethod -Uri "$base/api/v1/articles/$slug/comments"
    if (@($after).Count -ne 1 -or $after[0].PSObject.Properties.Name -contains 'email') { throw '评论公开契约不正确' }
    $stage = 'avatar and RSS'
    $avatar = Invoke-WebRequest -Uri "$base$($after[0].avatar_url)"
    if ($avatar.StatusCode -ne 200) { throw '头像服务不可用' }
    $feed = Invoke-WebRequest -Uri "$base/feed.xml"
    if ($feed.Content -notmatch $slug) { throw 'RSS 未包含文章' }

    Write-Output "HTTP smoke passed: login, article, taxonomy, search, comments, avatar, RSS"
}
catch {
    throw "HTTP smoke failed at $stage : $_"
}
finally {
    if ($articleId) {
        try { Invoke-RestMethod -Uri "$base/api/v1/admin/articles/$articleId" -Method Delete -Headers $headers -WebSession $session | Out-Null }
        catch { Write-Warning "临时文章清理失败: $articleId" }
    }
    if ($categoryCreated) {
        try { Invoke-RestMethod -Uri "$base/api/v1/admin/categories/$slug" -Method Delete -Headers $headers -WebSession $session | Out-Null }
        catch { Write-Warning "临时分类清理失败: $slug" }
    }
}

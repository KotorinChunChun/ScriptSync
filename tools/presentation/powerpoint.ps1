$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$request = [Console]::In.ReadToEnd() | ConvertFrom-Json
$app = $null
$deck = $null
try {
    if ($request.action -notin @('open', 'goto')) { throw '未対応の操作です' }
    $deckPath = [System.IO.Path]::GetFullPath([string]$request.path)
    if ([System.IO.Path]::GetExtension($deckPath) -ine '.pptx' -or -not (Test-Path -LiteralPath $deckPath -PathType Leaf)) { throw '既存のPPTXファイルを指定してください' }
    $app = New-Object -ComObject PowerPoint.Application
    $app.Visible = -1
    # マクロ無効。元ファイルは読み取り専用で開き、保存しない。
    $app.AutomationSecurity = 3
    foreach ($candidate in $app.Presentations) {
        if ($candidate.FullName -ieq $deckPath) { $deck = $candidate; break }
    }
    if ($null -eq $deck) {
        if ($request.action -ne 'open') { throw '対象PPTXを連携画面から開いてください' }
        $deck = $app.Presentations.Open($deckPath, -1, 0, -1)
    }
    if ($request.action -eq 'goto') {
        $slideId = [int]$request.slideId
        if ($slideId -le 0) { throw 'スライドIDが不正です' }
        $slide = $deck.Slides.FindBySlideID($slideId)
        $show = $deck.SlideShowWindow
        if ($null -eq $show) { $show = $deck.SlideShowSettings.Run() }
        $show.View.GotoSlide($slide.SlideIndex, 0)
        @{ slideId = $slide.SlideID; index = $slide.SlideIndex } | ConvertTo-Json -Compress
    } else {
        $slides = @($deck.Slides | ForEach-Object {
            $title = "スライド $($_.SlideIndex)"
            if ($_.Shapes.HasTitle) { $title = $_.Shapes.Title.TextFrame.TextRange.Text }
            @{ id = $_.SlideID; index = $_.SlideIndex; title = $title }
        })
        @{ path = $deck.FullName; slides = $slides } | ConvertTo-Json -Depth 5 -Compress
    }
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
} finally {
    # 利用者のプレゼンテーションを閉じない。
    if ($null -ne $deck) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($deck) }
    if ($null -ne $app) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app) }
}

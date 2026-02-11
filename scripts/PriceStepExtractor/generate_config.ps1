# Генератор JSON структуры для per-exchange блеклистов
$content = Get-Content "tick_size_blacklist.txt"

$binance = @()
$bybit = @()
$okx = @()
$gate = @()
$currentExchange = ""

foreach ($line in $content) {
    if ($line -match "^# (Binance|Bybit|OKX|GateFutures)") {
        $currentExchange = $matches[1]
    }
    elseif ($line -match "^([A-Z0-9][A-Z0-9_]*_USDT)") {
        $symbol = $matches[1].Replace("_", "")
        switch ($currentExchange) {
            "Binance" { $binance += $symbol }
            "Bybit" { $bybit += $symbol }
            "OKX" { $okx += $symbol }
            "GateFutures" { $gate += $symbol }
        }
    }
}

# Добавить BTC и ETH в каждый
$binance = @("BTCUSDT", "ETHUSDT") + ($binance | Sort-Object -Unique)
$bybit = @("BTCUSDT", "ETHUSDT") + ($bybit | Sort-Object -Unique)
$okx = @("BTCUSDT", "ETHUSDT") + ($okx | Sort-Object -Unique)
$gate = @("BTCUSDT", "ETHUSDT") + ($gate | Sort-Object -Unique)

# Путь к appsettings.json
$appSettingsPath = Join-Path (Get-Location) "..\..\src\SpreadAggregator.Presentation\appsettings.json"

if (-not (Test-Path $appSettingsPath)) {
    Write-Error "Не найден appsettings.json по пути: $appSettingsPath"
    exit
}

# Читаем существующий конфиг
$jsonContent = Get-Content $appSettingsPath -Raw | ConvertFrom-Json

# Обновляем блеклисты
# Функция для обновления блеклиста конкретной биржи
function Update-ExchangeBlacklist($exchangeName, $blacklist) {
    if ($jsonContent.ExchangeSettings.Exchanges.$exchangeName) {
        $jsonContent.ExchangeSettings.Exchanges.$exchangeName.Blacklist = $blacklist
    }
}

Update-ExchangeBlacklist "Binance" $binance
Update-ExchangeBlacklist "Bybit" $bybit
Update-ExchangeBlacklist "OKX" $okx
Update-ExchangeBlacklist "GateFutures" $gate

# Сохраняем обновленный конфиг
# Используем встроенный ConvertTo-Json, так как он корректно работает с PSCustomObject
$updatedJson = $jsonContent | ConvertTo-Json -Depth 10

# Форматируем JSON (отступы) для красоты (опционально, ConvertTo-Json делает в одну строку часто для массивов)
# Но простой вариант - просто сохранить
$updatedJson | Set-Content $appSettingsPath

Write-Host "✅ Успешно обновлен appsettings.json"
Write-Host "Binance: $($binance.Count) symbols"
Write-Host "Bybit: $($bybit.Count) symbols"  
Write-Host "OKX: $($okx.Count) symbols"
Write-Host "Gate: $($gate.Count) symbols"

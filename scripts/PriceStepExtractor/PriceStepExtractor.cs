using SpreadAggregator.Infrastructure.Services.Exchanges;
using SpreadAggregator.Domain.Entities;
using System.Text.Json;
using System.Text;
using System.Globalization;

namespace PriceStepExtractor;

class Program
{
    static async Task Main(string[] args)
    {
        Console.WriteLine("=== Price Step Extractor + Impact Analysis ===");
        Console.WriteLine("Извлечение данных о шаге цены и расчет влияния на рыночные ордера...\n");

        // Читаем конфиг для получения MinUsdVolume
        var configPath = Path.Combine(Directory.GetCurrentDirectory(), "..", "..", "src", "SpreadAggregator.Presentation", "appsettings.json");
        var configJson = await File.ReadAllTextAsync(configPath);
        var configDoc = JsonDocument.Parse(configJson);
        var exchanges = configDoc.RootElement.GetProperty("ExchangeSettings").GetProperty("Exchanges");

        var binanceMinVolume = exchanges.GetProperty("Binance").GetProperty("VolumeFilter").GetProperty("MinUsdVolume").GetDecimal();
        var bybitMinVolume = exchanges.GetProperty("Bybit").GetProperty("VolumeFilter").GetProperty("MinUsdVolume").GetDecimal();
        var okxMinVolume = exchanges.GetProperty("OKX").GetProperty("VolumeFilter").GetProperty("MinUsdVolume").GetDecimal();
        var gateMinVolume = exchanges.GetProperty("GateFutures").GetProperty("VolumeFilter").GetProperty("MinUsdVolume").GetDecimal();

        Console.WriteLine($"Фильтры объема из конфига:");
        Console.WriteLine($"  Binance: >= ${binanceMinVolume:N0}");
        Console.WriteLine($"  Bybit: >= ${bybitMinVolume:N0}");
        Console.WriteLine($"  OKX: >= ${okxMinVolume:N0}");
        Console.WriteLine($"  Gate: >= ${gateMinVolume:N0}\n");

        var allData = new List<(string Exchange, string Symbol, decimal PriceStep, decimal QuantityStep, decimal MinNotional, decimal Volume24h, decimal CurrentPrice, decimal TickImpact, decimal LotImpact)>();

        // Binance
        bool binanceEnabled = exchanges.GetProperty("Binance").GetProperty("Enabled").GetBoolean();
        if (binanceEnabled)
        {
            Console.WriteLine("📊 Извлечение данных с Binance...");
            try
            {
                var binanceClient = new BinanceFuturesExchangeClient(new HttpClient());
                var symbols = await binanceClient.GetSymbolsAsync();
                var tickers = await binanceClient.GetTickersAsync();
                
                var tickerDict = tickers.ToDictionary(t => t.Symbol, t => new { t.Volume24h, t.LastPrice });
                
                var filtered = symbols.Where(s => 
                {
                    if (tickerDict.TryGetValue(s.Name, out var ticker))
                    {
                        return ticker.Volume24h >= binanceMinVolume;
                    }
                    return false;
                }).ToList();

                foreach (var s in filtered)
                {
                    var ticker = tickerDict[s.Name];
                    var tickImpact = ticker.LastPrice > 0 ? (s.PriceStep / ticker.LastPrice) * 100 : 0;
                    var lotImpact = s.QuantityStep * ticker.LastPrice;
                    allData.Add((s.Exchange, s.Name, s.PriceStep, s.QuantityStep, s.MinNotional, ticker.Volume24h, ticker.LastPrice, tickImpact, lotImpact));
                }
                
                Console.WriteLine($"   ✅ Получено {symbols.Count()} символов, после фильтра: {filtered.Count}");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"   ❌ Ошибка при получении данных с Binance: {ex.Message}");
            }
        }

        // Bybit
        bool bybitEnabled = exchanges.GetProperty("Bybit").GetProperty("Enabled").GetBoolean();
        if (bybitEnabled)
        {
            Console.WriteLine("📊 Извлечение данных с Bybit...");
            try
            {
                var bybitWebSocket = new BybitFuturesNativeWebSocketClient();
                var bybitClient = new BybitFuturesExchangeClient(new HttpClient(), bybitWebSocket);
                var symbols = await bybitClient.GetSymbolsAsync();
                var tickers = await bybitClient.GetTickersAsync();
                
                var tickerDict = tickers.ToDictionary(t => t.Symbol, t => new { t.Volume24h, t.LastPrice });
                
                var filtered = symbols.Where(s => 
                {
                    if (tickerDict.TryGetValue(s.Name, out var ticker))
                    {
                        return ticker.Volume24h >= bybitMinVolume;
                    }
                    return false;
                }).ToList();

                foreach (var s in filtered)
                {
                    var ticker = tickerDict[s.Name];
                    var tickImpact = ticker.LastPrice > 0 ? (s.PriceStep / ticker.LastPrice) * 100 : 0;
                    var lotImpact = s.QuantityStep * ticker.LastPrice;
                    allData.Add((s.Exchange, s.Name, s.PriceStep, s.QuantityStep, s.MinNotional, ticker.Volume24h, ticker.LastPrice, tickImpact, lotImpact));
                }
                
                Console.WriteLine($"   ✅ Получено {symbols.Count()} символов, после фильтра: {filtered.Count}");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"   ❌ Ошибка при получении данных с Bybit: {ex.Message}");
            }
        }

        // OKX
        bool okxEnabled = exchanges.GetProperty("OKX").GetProperty("Enabled").GetBoolean();
        if (okxEnabled)
        {
            Console.WriteLine("📊 Извлечение данных с OKX...");
            try
            {
                var okxClient = new OKXFuturesExchangeClient(new HttpClient());
                var symbols = await okxClient.GetSymbolsAsync();
                var tickers = await okxClient.GetTickersAsync();
                
                var tickerDict = tickers.ToDictionary(t => t.Symbol, t => new { t.Volume24h, t.LastPrice });
                
                var filtered = symbols.Where(s => 
                {
                    if (tickerDict.TryGetValue(s.Name, out var ticker))
                    {
                        return ticker.Volume24h >= okxMinVolume;
                    }
                    return false;
                }).ToList();

                foreach (var s in filtered)
                {
                    var ticker = tickerDict[s.Name];
                    var tickImpact = ticker.LastPrice > 0 ? (s.PriceStep / ticker.LastPrice) * 100 : 0;
                    var lotImpact = s.QuantityStep * ticker.LastPrice;
                    allData.Add((s.Exchange, s.Name, s.PriceStep, s.QuantityStep, s.MinNotional, ticker.Volume24h, ticker.LastPrice, tickImpact, lotImpact));
                }
                
                Console.WriteLine($"   ✅ Получено {symbols.Count()} символов, после фильтра: {filtered.Count}");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"   ❌ Ошибка при получении данных с OKX: {ex.Message}");
            }
        }

        // Gate
        bool gateEnabled = exchanges.GetProperty("GateFutures").GetProperty("Enabled").GetBoolean();
        if (gateEnabled)
        {
            Console.WriteLine("📊 Извлечение данных с Gate...");
            try
            {
                var gateClient = new GateFuturesExchangeClient();
                var symbols = await gateClient.GetSymbolsAsync();
                var tickers = await gateClient.GetTickersAsync();
                
                var tickerDict = tickers.ToDictionary(t => t.Symbol, t => new { t.Volume24h, t.LastPrice });
                
                var filtered = symbols.Where(s => 
                {
                    if (tickerDict.TryGetValue(s.Name, out var ticker))
                    {
                        return ticker.Volume24h >= gateMinVolume;
                    }
                    return false;
                }).ToList();

                foreach (var s in filtered)
                {
                    var ticker = tickerDict[s.Name];
                    var tickImpact = ticker.LastPrice > 0 ? (s.PriceStep / ticker.LastPrice) * 100 : 0;
                    var lotImpact = s.QuantityStep * ticker.LastPrice;
                    allData.Add((s.Exchange, s.Name, s.PriceStep, s.QuantityStep, s.MinNotional, ticker.Volume24h, ticker.LastPrice, tickImpact, lotImpact));
                }
                
                Console.WriteLine($"   ✅ Получено {symbols.Count()} символов, после фильтра: {filtered.Count}");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"   ❌ Ошибка при получении данных с Gate: {ex.Message}");
            }
        }

        if (allData.Count == 0)
        {
            Console.WriteLine("\n❌ Не удалось получить данные ни с одной биржи!");
            return;
        }

        // Экспорт в CSV
        var csvPath = Path.Combine(Directory.GetCurrentDirectory(), "price_steps_output.csv");
        var csv = new StringBuilder();
        
        // Заголовок
        csv.AppendLine("Exchange,Symbol,CurrentPrice,PriceStep,QuantityStep,MinNotional,Volume24h,TickImpact%,LotImpact$");
        
        // Данные, сортируем по TickImpact (худшие сверху)
        foreach (var item in allData.OrderByDescending(x => x.TickImpact))
        {
            csv.AppendLine($"{item.Exchange},{item.Symbol},{item.CurrentPrice.ToString(CultureInfo.InvariantCulture)},{item.PriceStep.ToString(CultureInfo.InvariantCulture)},{item.QuantityStep.ToString(CultureInfo.InvariantCulture)},{item.MinNotional.ToString(CultureInfo.InvariantCulture)},{item.Volume24h.ToString(CultureInfo.InvariantCulture)},{item.TickImpact.ToString("F6", CultureInfo.InvariantCulture)},{item.LotImpact.ToString(CultureInfo.InvariantCulture)}");
        }
        
        await File.WriteAllTextAsync(csvPath, csv.ToString());

        Console.WriteLine($"\n✅ Данные экспортированы в: {csvPath}");
        Console.WriteLine($"   Всего символов после фильтрации: {allData.Count}");
        Console.WriteLine($"   - Binance: {allData.Count(s => s.Exchange == "Binance")}");
        Console.WriteLine($"   - Bybit: {allData.Count(s => s.Exchange == "Bybit")}");
        Console.WriteLine($"   - OKX: {allData.Count(s => s.Exchange == "OKX")}");
        Console.WriteLine($"   - Gate: {allData.Count(s => s.Exchange == "GateFutures")}");

        // Генерация блеклиста
        const decimal TICK_IMPACT_THRESHOLD = 0.05m;
        var blacklisted = allData.Where(x => x.TickImpact >= TICK_IMPACT_THRESHOLD).ToList();
        
        if (blacklisted.Count > 0)
        {
            var blacklistPath = Path.Combine(Directory.GetCurrentDirectory(), "tick_size_blacklist.txt");
            var blacklistContent = new StringBuilder();
            
            blacklistContent.AppendLine($"# Tick Size Blacklist (Impact >= {TICK_IMPACT_THRESHOLD}%)");
            blacklistContent.AppendLine($"# Сгенерировано: {DateTime.Now:yyyy-MM-dd HH:mm:ss}");
            blacklistContent.AppendLine($"# Всего монет: {blacklisted.Count}");
            blacklistContent.AppendLine();
            
            // Группируем по биржам
            var byExchange = blacklisted.GroupBy(x => x.Exchange).OrderBy(g => g.Key);
            
            foreach (var group in byExchange)
            {
                blacklistContent.AppendLine($"# {group.Key} ({group.Count()} монет):");
                blacklistContent.AppendLine();
                
                foreach (var item in group.OrderByDescending(x => x.TickImpact))
                {
                    // Формат: SYMBOL_USDT  # Impact: 0.1234%, Price: $1.23, TickStep: 0.001
                    blacklistContent.AppendLine($"{item.Symbol}  # Impact: {item.TickImpact:F4}%, Price: ${item.CurrentPrice:N4}, TickStep: {item.PriceStep}");
                }
                
                blacklistContent.AppendLine();
            }
            
            await File.WriteAllTextAsync(blacklistPath, blacklistContent.ToString());
            
            Console.WriteLine($"\n📋 Блеклист сгенерирован: {blacklistPath}");
            Console.WriteLine($"   Всего монет в блеклисте: {blacklisted.Count}");
            Console.WriteLine($"   - Binance: {blacklisted.Count(s => s.Exchange == "Binance")}");
            Console.WriteLine($"   - Bybit: {blacklisted.Count(s => s.Exchange == "Bybit")}");
            Console.WriteLine($"   - OKX: {blacklisted.Count(s => s.Exchange == "OKX")}");
            Console.WriteLine($"   - Gate: {blacklisted.Count(s => s.Exchange == "GateFutures")}");
        }

        // Статистика по Tick Impact
        Console.WriteLine($"\n📊 Анализ Tick Size Impact:");
        var excellent = allData.Count(x => x.TickImpact < 0.01m);
        var good = allData.Count(x => x.TickImpact >= 0.01m && x.TickImpact < 0.05m);
        var acceptable = allData.Count(x => x.TickImpact >= 0.05m && x.TickImpact < 0.1m);
        var bad = allData.Count(x => x.TickImpact >= 0.1m);
        
        Console.WriteLine($"   < 0.01% (отлично): {excellent}");
        Console.WriteLine($"   0.01-0.05% (хорошо): {good}");
        Console.WriteLine($"   0.05-0.1% (приемлемо): {acceptable}");
        Console.WriteLine($"   >= 0.1% (плохо): {bad}");

        // ТОП-10 худших по Tick Impact
        Console.WriteLine($"\n⚠️ ТОП-10 худших по Tick Size Impact:");
        Console.WriteLine($"   {"Exchange",-10} {"Symbol",-20} {"Price",12} {"TickStep",10} {"Impact%",10}");
        Console.WriteLine($"   {new string('-', 70)}");
        
        foreach (var item in allData.OrderByDescending(x => x.TickImpact).Take(10))
        {
            Console.WriteLine($"   {item.Exchange,-10} {item.Symbol,-20} ${item.CurrentPrice,11:N2} {item.PriceStep,9:F6} {item.TickImpact,9:F4}%");
        }

        // ТОП-10 лучших
        Console.WriteLine($"\n✅ ТОП-10 лучших по Tick Size Impact:");
        Console.WriteLine($"   {"Exchange",-10} {"Symbol",-20} {"Price",12} {"TickStep",10} {"Impact%",10}");
        Console.WriteLine($"   {new string('-', 70)}");
        
        foreach (var item in allData.OrderBy(x => x.TickImpact).Take(10))
        {
            Console.WriteLine($"   {item.Exchange,-10} {item.Symbol,-20} ${item.CurrentPrice,11:N2} {item.PriceStep,9:F6} {item.TickImpact,9:F4}%");
        }
    }
}

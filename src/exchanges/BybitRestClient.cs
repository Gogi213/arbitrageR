using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Net.Http;
using System.Text.Json;
using System.Threading.Tasks;
using SpreadAggregator.Application.Abstractions;
using SpreadAggregator.Domain.Entities;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// WARM/COLD PATH - Bybit REST API client for futures market.
/// Standard allocations acceptable - not performance-critical.
/// Handles symbol metadata, ticker snapshots, and market data queries.
/// </summary>
public sealed class BybitRestClient
{
    private readonly HttpClient _httpClient;
    private readonly string _exchangeName = "Bybit";
    
    // Symbol cache for GetSymbolInfoAsync (synchronized with exchange client)
    private readonly ConcurrentDictionary<string, SymbolInfo> _symbolCache;
    
    private const string BaseUrl = "https://api.bybit.com";

    public BybitRestClient(HttpClient httpClient, ConcurrentDictionary<string, SymbolInfo> symbolCache)
    {
        _httpClient = httpClient;
        _symbolCache = symbolCache;
        _httpClient.BaseAddress = new Uri(BaseUrl);
    }

    /// <summary>
    /// Fetches all linear perpetual symbols from Bybit V5 API.
    /// WARM PATH - Allocations acceptable for infrequent calls.
    /// </summary>
    public async ValueTask<IEnumerable<SymbolInfo>> GetSymbolsAsync()
    {
        // GET /v5/market/instruments-info?category=linear&status=Trading&limit=1000
        var response = await _httpClient.GetAsync("/v5/market/instruments-info?category=linear&status=Trading&limit=1000");
        response.EnsureSuccessStatusCode();

        var content = await response.Content.ReadAsStringAsync();
        var doc = JsonDocument.Parse(content);
        var list = doc.RootElement.GetProperty("result").GetProperty("list");

        var symbols = new List<SymbolInfo>();

        foreach (var item in list.EnumerateArray())
        {
            var symbol = item.GetProperty("symbol").GetString();
            var quoteCoin = item.GetProperty("quoteCoin").GetString();
            var status = item.GetProperty("status").GetString();

            // Filter for USDT/USDC only
            if (status == "Trading" && (quoteCoin == "USDT" || quoteCoin == "USDC"))
            {
                // Parse priceFilter and lotSizeFilter for actual values
                decimal priceStep = 0.01m; // fallback
                decimal quantityStep = 0.001m; // fallback
                decimal minNotional = 5m; // fallback

                if (item.TryGetProperty("priceFilter", out var priceFilter))
                {
                    if (priceFilter.TryGetProperty("tickSize", out var tickSize))
                    {
                        if (decimal.TryParse(tickSize.GetString(), System.Globalization.NumberStyles.Float,
                            System.Globalization.CultureInfo.InvariantCulture, out var parsedTickSize))
                        {
                            priceStep = parsedTickSize;
                        }
                    }
                }

                if (item.TryGetProperty("lotSizeFilter", out var lotSizeFilter))
                {
                    if (lotSizeFilter.TryGetProperty("qtyStep", out var qtyStep))
                    {
                        if (decimal.TryParse(qtyStep.GetString(), System.Globalization.NumberStyles.Float,
                            System.Globalization.CultureInfo.InvariantCulture, out var parsedQtyStep))
                        {
                            quantityStep = parsedQtyStep;
                        }
                    }

                    if (lotSizeFilter.TryGetProperty("minOrderQty", out var minOrderQty))
                    {
                        if (decimal.TryParse(minOrderQty.GetString(), System.Globalization.NumberStyles.Float,
                            System.Globalization.CultureInfo.InvariantCulture, out var parsedMinOrderQty))
                        {
                            minNotional = parsedMinOrderQty;
                        }
                    }
                }

                var symbolInfo = new SymbolInfo 
                { 
                    Name = NormalizeSymbol(symbol!), 
                    Exchange = _exchangeName
                };
                symbolInfo.SetPriceStep(priceStep);
                symbolInfo.SetQuantityStep(quantityStep);
                symbolInfo.SetMinNotional(minNotional);
                symbols.Add(symbolInfo);
            }
        }
        return symbols;
    }

    /// <summary>
    /// Gets symbol info for a specific symbol from cache or fetches from exchange.
    /// WARM PATH - Optimized with ValueTask for zero-alloc cache hits.
    /// </summary>
    public ValueTask<SymbolInfo?> GetSymbolInfoAsync(string symbol)
    {
        var normalizedSymbol = symbol.Contains("_") ? symbol : NormalizeSymbol(symbol);

        // Synchronous Cache Lookup (Zero Alloc)
        if (_symbolCache.TryGetValue(normalizedSymbol, out var cached))
        {
            return new ValueTask<SymbolInfo?>(cached);
        }

        // Asynchronous Fallback
        return GetSymbolInfoAsyncSlow(normalizedSymbol);
    }

    private async ValueTask<SymbolInfo?> GetSymbolInfoAsyncSlow(string normalizedSymbol)
    {
        var symbols = await GetSymbolsAsync();
        foreach (var s in symbols)
        {
            _symbolCache.TryAdd(s.Name, s);
        }

        return _symbolCache.TryGetValue(normalizedSymbol, out var info) ? info : null;
    }

    /// <summary>
    /// Gets 24h ticker statistics for all symbols.
    /// WARM PATH - Called periodically (~10 min intervals).
    /// </summary>
    public async ValueTask<IEnumerable<TickerData>> GetTickersAsync()
    {
        // GET /v5/market/tickers?category=linear
        var response = await _httpClient.GetAsync("/v5/market/tickers?category=linear");
        response.EnsureSuccessStatusCode();

        var content = await response.Content.ReadAsStringAsync();
        var doc = JsonDocument.Parse(content);
        var list = doc.RootElement.GetProperty("result").GetProperty("list");

        var tickers = new List<TickerData>();

        foreach (var item in list.EnumerateArray())
        {
            var symbol = item.GetProperty("symbol").GetString();
            if (symbol == null) continue;
            
            var lastPriceStr = item.GetProperty("lastPrice").GetString();
            var turnover24hStr = item.GetProperty("turnover24h").GetString();
            var priceChangePercent24hStr = item.GetProperty("price24hPcnt").GetString();
            var bidPriceStr = item.GetProperty("bid1Price").GetString();
            var askPriceStr = item.GetProperty("ask1Price").GetString();
            
            if (decimal.TryParse(lastPriceStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var price) &&
                decimal.TryParse(turnover24hStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var quoteVolume))
            {
                 decimal changePct = 0;
                 if (decimal.TryParse(priceChangePercent24hStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var change))
                 {
                     changePct = change * 100; 
                 }

                 decimal bestBid = 0;
                 decimal bestAsk = 0;
                 if (decimal.TryParse(bidPriceStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var bid)) bestBid = bid;
                 if (decimal.TryParse(askPriceStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var ask)) bestAsk = ask;

                 tickers.Add(new TickerData
                 {
                     Symbol = NormalizeSymbol(symbol),
                     LastPrice = price,
                     QuoteVolume = quoteVolume, 
                     PriceChangePercent24h = changePct,
                     Volume24h = quoteVolume,
                     BestBid = bestBid,
                     BestAsk = bestAsk,
                     Timestamp = DateTime.UtcNow
                 });
            }
        }
        return tickers;
    }

    /// <summary>
    /// Gets book ticker data (BestBid/BestAsk).
    /// For Bybit, GetTickersAsync already includes BestBid/BestAsk.
    /// </summary>
    public ValueTask<IEnumerable<TickerData>> GetBookTickersAsync() => GetTickersAsync();

    // Helper: Normalize symbol format (BTCUSDT -> BTC_USDT)
    public static string NormalizeSymbol(string symbol)
    {
        if (symbol.EndsWith("USDT"))
            return symbol.Replace("USDT", "_USDT");
        if (symbol.EndsWith("USDC"))
            return symbol.Replace("USDC", "_USDC");
        return symbol;
    }

    // Helper: Denormalize symbol (BTC_USDT -> BTCUSDT)
    public static string DenormalizeSymbol(string symbol) => symbol.Replace("_", "");
}

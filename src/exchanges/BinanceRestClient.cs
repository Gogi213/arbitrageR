using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging;
using SpreadAggregator.Application.Abstractions;
using SpreadAggregator.Application.Diagnostics;
using SpreadAggregator.Domain.Entities;
using SpreadAggregator.Domain.ValueObjects;
using SpreadAggregator.Infrastructure.Pooling;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// WARM/COLD PATH - Binance REST API client.
/// Handles all HTTP interactions, signing, and symbol metadata caching.
/// </summary>
public sealed class BinanceRestClient
{
    private readonly HttpClient _httpClient;
    
    // API Credentials
    private string _apiKey = "";
    private string _apiSecret = "";
    
    // Symbol info cache (symbol -> SymbolInfo)
    private readonly ConcurrentDictionary<string, SymbolInfo> _symbolCache = new();
    
    public string ExchangeName => "Binance";

    public BinanceRestClient(HttpClient httpClient)
    {
        _httpClient = httpClient;
    }

    public void SetCredentials(string apiKey, string apiSecret)
    {
        _apiKey = apiKey;
        _apiSecret = apiSecret;
        TradeLogger.Log("CONFIG", $"Binance credentials set (key ends with ...{apiKey[^4..]})");
    }

    private string SignRequest(string queryString)
    {
        using var hmac = new HMACSHA256(Encoding.UTF8.GetBytes(_apiSecret));
        var hash = hmac.ComputeHash(Encoding.UTF8.GetBytes(queryString));
        return BitConverter.ToString(hash).Replace("-", "").ToLower();
    }

    public ValueTask<SymbolInfo?> GetSymbolInfoAsync(string symbol)
    {
        var normalizedSymbol = symbol.Contains("_") ? symbol : NormalizeSymbol(symbol);
        
        if (_symbolCache.TryGetValue(normalizedSymbol, out var cached))
        {
            return new ValueTask<SymbolInfo?>(cached);
        }
        
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

    public async ValueTask<IEnumerable<SymbolInfo>> GetSymbolsAsync()
    {
        var response = await _httpClient.GetAsync("/fapi/v1/exchangeInfo");
        response.EnsureSuccessStatusCode();

        var content = await response.Content.ReadAsStringAsync();
        using var doc = JsonDocument.Parse(content);
        var symbols = doc.RootElement.GetProperty("symbols");

        var result = new List<SymbolInfo>();
        foreach (var s in symbols.EnumerateArray())
        {
            if (s.GetProperty("status").GetString() == "TRADING" && s.GetProperty("quoteAsset").GetString() == "USDT")
            {
                decimal priceStep = 0.01m;
                decimal quantityStep = 0.001m;
                decimal minNotional = 5m;

                if (s.TryGetProperty("filters", out var filters))
                {
                    foreach (var filter in filters.EnumerateArray())
                    {
                        if (filter.TryGetProperty("filterType", out var filterType))
                        {
                            var filterTypeStr = filterType.GetString();
                            
                            if (filterTypeStr == "PRICE_FILTER" && filter.TryGetProperty("tickSize", out var tickSize))
                            {
                                if (decimal.TryParse(tickSize.GetString(), System.Globalization.NumberStyles.Float, 
                                    System.Globalization.CultureInfo.InvariantCulture, out var parsedTickSize))
                                {
                                    priceStep = parsedTickSize;
                                }
                            }
                            else if (filterTypeStr == "LOT_SIZE" && filter.TryGetProperty("stepSize", out var stepSize))
                            {
                                if (decimal.TryParse(stepSize.GetString(), System.Globalization.NumberStyles.Float, 
                                    System.Globalization.CultureInfo.InvariantCulture, out var parsedStepSize))
                                {
                                    quantityStep = parsedStepSize;
                                }
                            }
                            else if (filterTypeStr == "MIN_NOTIONAL" && filter.TryGetProperty("notional", out var notional))
                            {
                                if (decimal.TryParse(notional.GetString(), System.Globalization.NumberStyles.Float, 
                                    System.Globalization.CultureInfo.InvariantCulture, out var parsedNotional))
                                {
                                    minNotional = parsedNotional;
                                }
                            }
                        }
                    }
                }

                var symbolInfo = new SymbolInfo
                {
                    Exchange = ExchangeName,
                    Name = NormalizeSymbol(s.GetProperty("symbol").GetString())
                };
                symbolInfo.SetMinNotional(minNotional);
                symbolInfo.SetPriceStep(priceStep);
                symbolInfo.SetQuantityStep(quantityStep);
                result.Add(symbolInfo);
            }
        }
        return result;
    }

    public async Task<(bool success, long orderId, string error)> PlaceOrderAsync(
        string symbol, string side, decimal price, decimal quantity)
    {
        try
        {
            var binanceSymbol = DenormalizeSymbol(symbol);
            var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            
            var symbolInfo = await GetSymbolInfoAsync(symbol);
            if (symbolInfo == null)
                return (false, 0, $"Symbol not found: {symbol}");
            
            price = RoundPrice(price, symbolInfo);
            quantity = RoundQuantity(quantity, symbolInfo);
            
            var priceStr = price.ToString(System.Globalization.CultureInfo.InvariantCulture);
            var qtyStr = quantity.ToString(System.Globalization.CultureInfo.InvariantCulture);
            
            var queryString = $"symbol={binanceSymbol}&side={side}&type=LIMIT&timeInForce=GTX&price={priceStr}&quantity={qtyStr}&timestamp={timestamp}";
            var signature = SignRequest(queryString);
            queryString += $"&signature={signature}";
            
            var request = new HttpRequestMessage(HttpMethod.Post, $"/fapi/v1/order?{queryString}");
            request.Headers.Add("X-MBX-APIKEY", _apiKey);
            
            var response = await _httpClient.SendAsync(request);
            var content = await response.Content.ReadAsStringAsync();
            
            if (response.IsSuccessStatusCode)
            {
                using var doc = JsonDocument.Parse(content);
                var orderId = doc.RootElement.GetProperty("orderId").GetInt64();
                TradeLogger.LogOrderPlaced(symbol, side, price, quantity, orderId);
                return (true, orderId, "");
            }
            else
            {
                TradeLogger.LogError("PlaceOrder", content);
                return (false, 0, content);
            }
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("PlaceOrder", ex.Message);
            return (false, 0, ex.Message);
        }
    }

    public async Task<(bool success, string error)> ModifyOrderAsync(
        string symbol, long orderId, string side, decimal newPrice, decimal quantity)
    {
        try
        {
            var binanceSymbol = DenormalizeSymbol(symbol);
            var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            
            var symbolInfo = await GetSymbolInfoAsync(symbol);
            if (symbolInfo == null)
                return (false, $"Symbol not found: {symbol}");
            
            newPrice = RoundPrice(newPrice, symbolInfo);
            quantity = RoundQuantity(quantity, symbolInfo);
            
            var priceStr = newPrice.ToString(System.Globalization.CultureInfo.InvariantCulture);
            var qtyStr = quantity.ToString(System.Globalization.CultureInfo.InvariantCulture);
            
            var queryString = $"symbol={binanceSymbol}&orderId={orderId}&side={side}&price={priceStr}&quantity={qtyStr}&timestamp={timestamp}";
            var signature = SignRequest(queryString);
            queryString += $"&signature={signature}";
            
            var request = new HttpRequestMessage(HttpMethod.Put, $"/fapi/v1/order?{queryString}");
            request.Headers.Add("X-MBX-APIKEY", _apiKey);
            
            var response = await _httpClient.SendAsync(request);
            var content = await response.Content.ReadAsStringAsync();
            
            if (response.IsSuccessStatusCode)
            {
                TradeLogger.LogOrderModified(symbol, orderId, 0, newPrice);
                return (true, "");
            }
            else
            {
                TradeLogger.LogError("ModifyOrder", content);
                return (false, content);
            }
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("ModifyOrder", ex.Message);
            return (false, ex.Message);
        }
    }

    public async Task<(bool success, string error)> CancelOrderAsync(string symbol, long orderId)
    {
        try
        {
            var binanceSymbol = DenormalizeSymbol(symbol);
            var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            
            var queryString = $"symbol={binanceSymbol}&orderId={orderId}&timestamp={timestamp}";
            var signature = SignRequest(queryString);
            queryString += $"&signature={signature}";
            
            var request = new HttpRequestMessage(HttpMethod.Delete, $"/fapi/v1/order?{queryString}");
            request.Headers.Add("X-MBX-APIKEY", _apiKey);
            
            var response = await _httpClient.SendAsync(request);
            var content = await response.Content.ReadAsStringAsync();
            
            if (response.IsSuccessStatusCode)
            {
                TradeLogger.Log("CANCEL", $"{symbol} orderId={orderId}");
                return (true, "");
            }
            else
            {
                TradeLogger.LogError("CancelOrder", content);
                return (false, content);
            }
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("CancelOrder", ex.Message);
            return (false, ex.Message);
        }
    }

    public async Task<(bool success, OrderState state, string error)> GetOrderAsync(string symbol, long orderId)
    {
        try
        {
            var binanceSymbol = DenormalizeSymbol(symbol);
            var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

            var queryString = $"symbol={binanceSymbol}&orderId={orderId}&timestamp={timestamp}";
            var signature = SignRequest(queryString);
            queryString += $"&signature={signature}";

            var request = new HttpRequestMessage(HttpMethod.Get, $"/fapi/v1/order?{queryString}");
            request.Headers.Add("X-MBX-APIKEY", _apiKey);

            var response = await _httpClient.SendAsync(request);
            var content = await response.Content.ReadAsStringAsync();

            if (response.IsSuccessStatusCode)
            {
                using var doc = JsonDocument.Parse(content);
                var statusStr = doc.RootElement.GetProperty("status").GetString();
                
                return Enum.TryParse<OrderState>(statusStr, true, out var state) 
                    ? (true, state, "") 
                    : (true, OrderState.Unknown, "");
            }
            else
            {
                return (false, OrderState.Unknown, content);
            }
        }
        catch (Exception ex)
        {
            return (false, OrderState.Unknown, ex.Message);
        }
    }

    public async ValueTask<IEnumerable<TickerData>> GetTickersAsync()
    {
        var response = await _httpClient.GetAsync("/fapi/v1/ticker/24hr");
        response.EnsureSuccessStatusCode();

        var content = await response.Content.ReadAsStringAsync();
        using var doc = JsonDocument.Parse(content);

        var result = new List<TickerData>();
        foreach (var t in doc.RootElement.EnumerateArray())
        {
            var symbol = t.GetProperty("symbol").GetString();
            if (symbol.EndsWith("USDT"))
            {
                 decimal.TryParse(t.GetProperty("lastPrice").GetString(), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var price);
                 decimal.TryParse(t.GetProperty("quoteVolume").GetString(), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var quoteVol);
                 decimal.TryParse(t.GetProperty("priceChangePercent").GetString(), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var priceChange);

                result.Add(new TickerData
                {
                    Symbol = NormalizeSymbol(symbol),
                    LastPrice = price,
                    Volume24h = quoteVol,
                    QuoteVolume = quoteVol,
                    PriceChangePercent24h = priceChange,
                    Timestamp = DateTime.UtcNow
                });
            }
        }
        return result;
    }

    public ValueTask<IEnumerable<TickerData>> GetBookTickersAsync()
    {
         return new ValueTask<IEnumerable<TickerData>>(GetTickersAsync().AsTask());
    }

    public decimal RoundPrice(decimal price, SymbolInfo symbolInfo)
    {
        var tickSize = symbolInfo.PriceStep;
        if (tickSize == 0) return price;
        return Math.Round(price / tickSize) * tickSize;
    }

    public decimal RoundQuantity(decimal qty, SymbolInfo symbolInfo)
    {
        var stepSize = symbolInfo.QuantityStep;
        if (stepSize == 0) return qty;
        return Math.Round(qty / stepSize) * stepSize;
    }

    private static string NormalizeSymbol(string binanceSymbol)
    {
        if (binanceSymbol.EndsWith("USDT") && !binanceSymbol.Contains('_'))
        {
            var normalized = binanceSymbol.Insert(binanceSymbol.Length - 4, "_");
            return StringPool.Intern(normalized);
        }
        return StringPool.Intern(binanceSymbol);
    }

    private static string DenormalizeSymbol(string normalizedSymbol)
    {
        return normalizedSymbol.Replace("_", "");
    }
}

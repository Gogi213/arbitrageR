
using SpreadAggregator.Application.Diagnostics;
using SpreadAggregator.Domain.Entities;
using System.Net.Http.Json; // Assuming needed or just general Json
using System.Text.Json;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using SpreadAggregator.Application.Abstractions;

using SpreadAggregator.Infrastructure.Pooling;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

public class BinanceFuturesExchangeClient : IExchangeClient
{
    private readonly HttpClient _httpClient;
    private BinanceFuturesNativeWebSocketClient _nativeWebSocket;
    private BinanceFuturesNativeWebSocketClient _nativeBookTickerWebSocket; // Separate WS for bookTicker
    private readonly BinanceWebSocketHotPath _hotPath;
    private readonly BinanceRestClient _restClient; // NEW
    private const string BaseUrl = "https://fapi.binance.com";
    
    public string ExchangeName => "Binance";

    public BinanceFuturesExchangeClient(HttpClient httpClient)
    {
        _httpClient = httpClient;
        _httpClient.BaseAddress = new Uri(BaseUrl);
        _hotPath = new BinanceWebSocketHotPath();
        _restClient = new BinanceRestClient(httpClient); // Init Rest Client
        // Initialize WebSockets to satisfy CS8618
        _nativeWebSocket = new BinanceFuturesNativeWebSocketClient();
        _nativeBookTickerWebSocket = new BinanceFuturesNativeWebSocketClient();
    }
    
    /// <summary>
    /// Set API credentials for trading
    /// </summary>
    public void SetCredentials(string apiKey, string apiSecret)
    {
        _restClient.SetCredentials(apiKey, apiSecret);
    }
    
    /// <summary>
    /// Get symbol info from cache or fetch from exchange
    /// </summary>
    public ValueTask<SymbolInfo?> GetSymbolInfoAsync(string symbol)
    {
        return _restClient.GetSymbolInfoAsync(symbol);
    }
    
    /// <summary>
    /// Round price to symbol's tick size precision
    /// </summary>
    public decimal RoundPrice(decimal price, SymbolInfo symbolInfo)
    {
        return _restClient.RoundPrice(price, symbolInfo);
    }
    
    /// <summary>
    /// Round quantity to symbol's lot size precision
    /// </summary>
    public decimal RoundQuantity(decimal qty, SymbolInfo symbolInfo)
    {
        return _restClient.RoundQuantity(qty, symbolInfo);
    }
    
    /// <summary>
    /// Place a limit order
    /// </summary>
    public Task<(bool success, long orderId, string error)> PlaceOrderAsync(
        string symbol, string side, decimal price, decimal quantity)
    {
        return _restClient.PlaceOrderAsync(symbol, side, price, quantity);
    }
    
    /// <summary>
    /// Modify an existing order (PUT /fapi/v1/order)
    /// </summary>
    public Task<(bool success, string error)> ModifyOrderAsync(
        string symbol, long orderId, string side, decimal newPrice, decimal quantity)
    {
        return _restClient.ModifyOrderAsync(symbol, orderId, side, newPrice, quantity);
    }
    
    /// <summary>
    /// Cancel an order
    /// </summary>
    public Task<(bool success, string error)> CancelOrderAsync(string symbol, long orderId)
    {
        return _restClient.CancelOrderAsync(symbol, orderId);
    }
    
    /// <summary>
    /// Get single order status
    /// </summary>
    public Task<(bool success, OrderState state, string error)> GetOrderAsync(string symbol, long orderId)
    {
        return _restClient.GetOrderAsync(symbol, orderId);
    }
    
    private static string DenormalizeSymbol(string normalizedSymbol)
    {
        // BTC_USDT -> BTCUSDT
        return normalizedSymbol.Replace("_", "");
    }

    public ValueTask<IEnumerable<SymbolInfo>> GetSymbolsAsync()
    {
        return _restClient.GetSymbolsAsync();
    }

    public ValueTask<IEnumerable<TickerData>> GetTickersAsync()
    {
        return _restClient.GetTickersAsync();
    }

    public ValueTask<IEnumerable<TickerData>> GetBookTickersAsync()
    {
         return _restClient.GetBookTickersAsync();
    }


    public async ValueTask SubscribeToTradesAsync(IEnumerable<string> symbols, Func<TradeData, Task> onData)
    {
        // OPTIMIZATION: Avoid LINQ ToList()
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        
        DiagnosticLogger.Log("Binance", $"SubscribeToTradesAsync called with {symbolsList.Count} symbols");

        if (symbolsList.Count == 0)
        {
            DiagnosticLogger.Log("Binance", "No symbols to subscribe");
            return;
        }

        // SPRINT 25.11 FIX: Reuse existing socket instead of creating new on every call
        if (_nativeWebSocket == null)
        {
            _nativeWebSocket = new BinanceFuturesNativeWebSocketClient();
        }
        await _nativeWebSocket.ConnectAsync(); // Idempotent: returns immediately if already connected

        // Denormalize symbols for Binance API (BTC_USDT → BTCUSDT)
        // OPTIMIZATION: Avoid LINQ Select()
        var binanceSymbols = new List<string>(symbolsList.Count);
        for (int i = 0; i < symbolsList.Count; i++)
        {
            binanceSymbols.Add(DenormalizeSymbol(symbolsList[i]));
        }

        // HOTFIX v2: No lambda capture - delegate directly to hot path
        await _nativeWebSocket.SubscribeToTradesAsync(binanceSymbols, trade => _hotPath.ProcessTradeCallbackAsync(trade, onData).AsTask());

        DiagnosticLogger.Log("Binance", $"✅ Subscribed to {symbolsList.Count} symbols (Batched)");
    }

    public virtual async ValueTask SubscribeToBookTickersAsync(IEnumerable<string> symbols, Func<TickerData, Task> onData)
    {
        // OPTIMIZATION: Avoid LINQ ToList()
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        
        DiagnosticLogger.Log("Binance", $"SubscribeToBookTickersAsync called with {symbolsList.Count} symbols");

        if (symbolsList.Count == 0)
        {
            DiagnosticLogger.Log("Binance", "No symbols to subscribe for bookTicker");
            return;
        }

        // SPRINT 25.11 FIX: Reuse existing socket instead of creating new on every call
        if (_nativeBookTickerWebSocket == null)
        {
            _nativeBookTickerWebSocket = new BinanceFuturesNativeWebSocketClient();
        }
        await _nativeBookTickerWebSocket.ConnectAsync(); // Idempotent: returns immediately if already connected

        // Denormalize symbols for Binance API (BTC_USDT → BTCUSDT)
        // OPTIMIZATION: Avoid LINQ Select()
        var binanceSymbols = new List<string>(symbolsList.Count);
        for (int i = 0; i < symbolsList.Count; i++)
        {
            binanceSymbols.Add(DenormalizeSymbol(symbolsList[i]));
        }

        // HOTFIX v2: No lambda capture - delegate directly to hot path
        await _nativeBookTickerWebSocket.SubscribeToBookTickersAsync(binanceSymbols, ticker => _hotPath.ProcessBookTickerCallbackAsync(ticker, onData).AsTask());

        DiagnosticLogger.Log("Binance", $"✅ Subscribed to bookTicker for {symbolsList.Count} symbols");
    }

    public async ValueTask StopAsync()
    {
         if (_nativeWebSocket != null) 
             await _nativeWebSocket.DisconnectAsync();
    }
}


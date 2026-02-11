using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Net.Http;
using System.Threading.Tasks;
using SpreadAggregator.Application.Abstractions;
using SpreadAggregator.Application.Diagnostics;
using SpreadAggregator.Domain.Entities;
using SpreadAggregator.Infrastructure.Pooling;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// Bybit Futures Exchange Client - Thin coordinator delegating to specialized components.
/// HOT PATH: BybitFuturesNativeWebSocketClient (WebSocket processing)
/// WARM PATH: BybitRestClient (REST API calls for metadata/tickers)
/// SPRINT 15 HOTFIX: Added symbol normalization cache to eliminate String.Replace allocations.
/// </summary>
public class BybitFuturesExchangeClient : IExchangeClient
{
    public string ExchangeName => "Bybit";

    // Shared symbol cache (synchronized between REST client and this coordinator)
    private readonly ConcurrentDictionary<string, SymbolInfo> _symbolCache = new();
    
    // HOTFIX: Symbol normalization cache (zero-alloc string interning)
    // Maps: BTCUSDT -> BTC_USDT (normalized), BTC_USDT -> BTCUSDT (denormalized)
    private readonly ConcurrentDictionary<string, string> _normalizedSymbolCache = new();
    private readonly ConcurrentDictionary<string, string> _denormalizedSymbolCache = new();

    private readonly HttpClient _httpClient;
    private readonly BybitFuturesNativeWebSocketClient _nativeWebSocket;
    private BybitFuturesNativeWebSocketClient? _nativeBookTickerWebSocket;
    
    // SPRINT 15: Extracted warm/cold path to BybitRestClient
    private readonly BybitRestClient _restClient;
    
    // HOTFIX: Lambda capture state (avoid closure allocations)
    private long _tradesMessageCount = 0;
    private long _tradesLastLogTicks = 0;
    private long _tickersMessageCount = 0;
    private long _tickersLastLogTicks = 0;

    public BybitFuturesExchangeClient(HttpClient httpClient, BybitFuturesNativeWebSocketClient nativeWebSocket)
    {
        _httpClient = httpClient;
        _nativeWebSocket = nativeWebSocket;
        
        // SPRINT 15: Initialize REST client with shared cache
        _restClient = new BybitRestClient(httpClient, _symbolCache);
    }

    // SPRINT 15: Delegate to BybitRestClient (WARM PATH)
    public ValueTask<IEnumerable<SymbolInfo>> GetSymbolsAsync() => _restClient.GetSymbolsAsync();
    public ValueTask<SymbolInfo?> GetSymbolInfoAsync(string symbol) => _restClient.GetSymbolInfoAsync(symbol);
    public ValueTask<IEnumerable<TickerData>> GetTickersAsync() => _restClient.GetTickersAsync();
    public ValueTask<IEnumerable<TickerData>> GetBookTickersAsync() => _restClient.GetBookTickersAsync();

    /// <summary>
    /// HOTFIX: Zero-allocation symbol normalization with string interning cache.
    /// BTCUSDT -> BTC_USDT
    /// </summary>
    private string NormalizeSymbolCached(string symbol)
    {
        return _normalizedSymbolCache.GetOrAdd(symbol, s =>
        {
            if (s.EndsWith("USDT"))
                return s.Substring(0, s.Length - 4) + "_USDT";
            if (s.EndsWith("USDC"))
                return s.Substring(0, s.Length - 4) + "_USDC";
            return s;
        });
    }

    /// <summary>
    /// HOTFIX: Zero-allocation symbol denormalization with string interning cache.
    /// BTC_USDT -> BTCUSDT
    /// </summary>
    private string DenormalizeSymbolCached(string symbol)
    {
        return _denormalizedSymbolCache.GetOrAdd(symbol, s => s.Replace("_", ""));
    }

    /// <summary>
    /// Subscribe to real-time trade updates via native WebSocket.
    /// HOTFIX: Uses cached normalization and instance method to avoid lambda capture allocations.
    /// </summary>
    public async ValueTask SubscribeToTradesAsync(IEnumerable<string> symbols, Func<TradeData, Task> onData)
    {
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        int symbolCount = symbolsList.Count;
        
        DiagnosticLogger.Log("Bybit", $"SubscribeToTradesAsync called with {symbolCount} symbols");

        await _nativeWebSocket.ConnectAsync();

        // Denormalize symbols (BTC_USDT -> BTCUSDT) - uses cache
        var bybitSymbols = new List<string>(symbolCount);
        for (int i = 0; i < symbolCount; i++)
        {
            bybitSymbols.Add(DenormalizeSymbolCached(symbolsList[i]));
        }

        // Reset state for this subscription
        _tradesMessageCount = 0;
        _tradesLastLogTicks = Stopwatch.GetTimestamp();
        
        // Store callback to avoid lambda capture
        var userCallback = onData;

        await _nativeWebSocket.SubscribeToTradesAsync(bybitSymbols, trade => HandleTradeUpdate(trade, userCallback));
        
        DiagnosticLogger.Log("Bybit", $"✅ Subscribed to {symbolCount} symbols (Batched)");
    }

    /// <summary>
    /// HOTFIX v2: TRUE ZERO ALLOC - No async state machine in hot path.
    /// Fast path for synchronous callbacks (IsCompleted), slow path for async.
    /// Uses TradeDataPool for zero allocations.
    /// </summary>
    private Task HandleTradeUpdate(TradeData trade, Func<TradeData, Task> onData)
    {
        // Periodic logging (every 5 seconds) - COLD PATH
        var elapsedMs = (Stopwatch.GetTimestamp() - _tradesLastLogTicks) * 1000 / Stopwatch.Frequency;
        if (elapsedMs >= 5000)
        {
            _tradesLastLogTicks = Stopwatch.GetTimestamp();
            DiagnosticLogger.Log("Bybit", $"Trades active. Count={_tradesMessageCount}. Sample: {trade.Symbol} {trade.Price}");
            _tradesMessageCount = 0;
        }
        _tradesMessageCount++;

        // HOT PATH: Use TradeDataPool for zero allocations
        var normalizedTrade = TradeDataPool.Get();
        normalizedTrade.Exchange = trade.Exchange;
        normalizedTrade.Symbol = NormalizeSymbolCached(trade.Symbol); // USES CACHE (zero alloc after first hit)
        normalizedTrade.PriceTicks = trade.PriceTicks;
        normalizedTrade.QuantityTicks = trade.QuantityTicks;
        normalizedTrade.Side = trade.Side;
        normalizedTrade.TimestampMicros = trade.TimestampMicros;
        
        // ZERO ALLOC FAST PATH: If callback is synchronous or already completed
        var task = onData(normalizedTrade);
        if (task.IsCompleted)
        {
            TradeDataPool.Return(normalizedTrade);
            return task.IsCompletedSuccessfully ? Task.CompletedTask : task;
        }
        
        // SLOW PATH: Async callback (only allocates state machine if needed)
        return HandleTradeUpdateSlowPath(task, normalizedTrade);
    }

    /// <summary>
    /// HOTFIX v2: Async slow path for trade updates (only used if callback is truly async).
    /// </summary>
    private async Task HandleTradeUpdateSlowPath(Task task, TradeData normalizedTrade)
    {
        try
        {
            await task;
        }
        finally
        {
            TradeDataPool.Return(normalizedTrade);
        }
    }

    /// <summary>
    /// Subscribe to real-time book ticker updates via native WebSocket.
    /// HOTFIX: Uses TickerDataPool and cached normalization for zero allocations.
    /// </summary>
    public async ValueTask SubscribeToBookTickersAsync(IEnumerable<string> symbols, Func<TickerData, Task> onData)
    {
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        int symbolCount = symbolsList.Count;
        
        DiagnosticLogger.Log("Bybit", $"SubscribeToBookTickersAsync called with {symbolCount} symbols");

        if (symbolCount == 0)
        {
            DiagnosticLogger.Log("Bybit", "No symbols to subscribe for bookTicker");
            return;
        }

        // Create separate WebSocket for bookTicker
        _nativeBookTickerWebSocket = new BybitFuturesNativeWebSocketClient();
        await _nativeBookTickerWebSocket.ConnectAsync();

        // Denormalize symbols - uses cache
        var bybitSymbols = new List<string>(symbolCount);
        for (int i = 0; i < symbolCount; i++)
        {
            bybitSymbols.Add(DenormalizeSymbolCached(symbolsList[i]));
        }

        // Reset state for this subscription
        _tickersMessageCount = 0;
        _tickersLastLogTicks = Stopwatch.GetTimestamp();
        
        // Store callback to avoid lambda capture
        var userCallback = onData;

        await _nativeBookTickerWebSocket.SubscribeToBookTickersAsync(bybitSymbols, ticker => HandleBookTickerUpdate(ticker, userCallback));

        DiagnosticLogger.Log("Bybit", $"✅ Subscribed to bookTicker for {symbolCount} symbols");
    }

    /// <summary>
    /// HOTFIX v2: TRUE ZERO ALLOC - No async state machine in hot path.
    /// Fast path for synchronous callbacks (IsCompleted), slow path for async.
    /// Uses TickerDataPool for zero allocations.
    /// </summary>
    private Task HandleBookTickerUpdate(TickerData ticker, Func<TickerData, Task> onData)
    {
        // Periodic logging (every 5 seconds) - COLD PATH
        var elapsedMs = (Stopwatch.GetTimestamp() - _tickersLastLogTicks) * 1000 / Stopwatch.Frequency;
        if (elapsedMs >= 5000)
        {
            _tickersLastLogTicks = Stopwatch.GetTimestamp();
            DiagnosticLogger.Log("Bybit", $"BookTickers active. Count={_tickersMessageCount}. Sample: {ticker.Symbol} Bid={ticker.BestBid} Ask={ticker.BestAsk}");
            _tickersMessageCount = 0;
        }
        _tickersMessageCount++;

        var normalizedTicker = TickerDataPool.Get();
        
        // USES CACHE (zero alloc after first hit)
        normalizedTicker.Symbol = NormalizeSymbolCached(ticker.Symbol);
        normalizedTicker.BestBidTicks = ticker.BestBidTicks;
        normalizedTicker.BestAskTicks = ticker.BestAskTicks;
        normalizedTicker.TimestampTicks = ticker.TimestampTicks;
        
        // ZERO ALLOC FAST PATH: If callback is synchronous or already completed
        Task task;
        try
        {
            task = onData(normalizedTicker);
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("Bybit", $"Error in bookTicker callback: {ex.Message}");
            TickerDataPool.Return(normalizedTicker);
            return Task.CompletedTask;
        }
        
        if (task.IsCompleted)
        {
            TickerDataPool.Return(normalizedTicker);
            return task.IsCompletedSuccessfully ? Task.CompletedTask : task;
        }
        
        // SLOW PATH: Async callback (only allocates state machine if needed)
        return HandleBookTickerUpdateSlowPath(task, normalizedTicker);
    }

    /// <summary>
    /// HOTFIX v2: Async slow path for book ticker updates (only used if callback is truly async).
    /// </summary>
    private async Task HandleBookTickerUpdateSlowPath(Task task, TickerData normalizedTicker)
    {
        try
        {
            await task;
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("Bybit", $"Error in bookTicker callback: {ex.Message}");
        }
        finally
        {
            TickerDataPool.Return(normalizedTicker);
        }
    }
    
    public async ValueTask StopAsync()
    {
        if (_nativeWebSocket != null) 
             await _nativeWebSocket.DisconnectAsync();
    }
}

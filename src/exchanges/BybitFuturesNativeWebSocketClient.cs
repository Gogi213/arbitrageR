using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using SpreadAggregator.Application.Abstractions;
using SpreadAggregator.Domain.Entities;
using SpreadAggregator.Domain.ValueObjects;
using SpreadAggregator.Infrastructure.Services.Exchanges.Base;

using SpreadAggregator.Infrastructure.Pooling;
using SpreadAggregator.Infrastructure.Logging;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// Native WebSocket client for Bybit Futures (V5)
/// SPRINT 16: TRUE ZERO ALLOC - Symbol cache + Fast/Slow path pattern
/// </summary>
public class BybitFuturesNativeWebSocketClient : NativeWebSocketClientBase
{
    protected override string WebSocketEndpoint => "wss://stream.bybit.com/v5/public/linear";

    // Callbacks for Trade Data (key: symbol string in UPPERCASE matching REST/WS format)
    private readonly ConcurrentDictionary<string, Func<TradeData, Task>> _symbolCallbacks = new();
    private readonly ConcurrentDictionary<string, Func<TickerData, Task>> _bookTickerCallbacks = new();
    
    // SPRINT 16: Symbol cache for zero-alloc string interning (ReadOnlySpan<byte> -> string)
    private readonly ZeroAllocSymbolCache _symbolCache = new();
    
    private int _requestId = 0;
    private Task? _pingTask;
    private CancellationTokenSource? _pingCts;
    
    // HFT: ThreadLocal buffer to avoid List<> allocation on every message
    // WARN: ThreadStatic is dangerous with async/await. Replaced with per-instance reusable buffer for now
    // Since we are single-threaded per connection receive loop, a simple field is fine?
    // ReceiveLoop is awaited... 
    // WebSocketHotPathBase.OnMessage is called synchronously from ReceiveLoop.
    // So we are safe to use a field if only one ReceiveLoop runs at a time.
    // NativeWebSocketClientBase ensures single ReceiveLoop task.
    private readonly List<(string symbol, long priceTicks, long qtyTicks, string side, long ts)> _tradesBuffer = new(16);

    /// <summary>
    /// SPRINT 16: Zero-alloc symbol lookup using pre-cached symbols.
    /// Symbols are cached during subscription, so hot path just converts Utf8 to string ONCE.
    /// After first hit, string is interned and reused.
    /// </summary>
    private string GetOrCacheSymbol(ReadOnlySpan<byte> utf8Bytes)
    {
        return _symbolCache.GetOrAdd(utf8Bytes);
    }

    protected override async Task OnConnectedAsync(CancellationToken cancellationToken)
    {
        // Console.WriteLine("[BybitFuturesNative] Starting Ping loop...");
        _pingCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        _pingTask = PingLoopAsync(_pingCts.Token);

        // If we have existing callbacks (reconnection), re-subscribe
        await ResubscribeAllAsync(cancellationToken);
    }

    private async Task PingLoopAsync(CancellationToken token)
    {
        // Bybit requires ping every 20s. We'll do 15s to be safe.
        // Format: {"op": "ping"}
        // Response: {"op": "pong", "args": [ "timestamp" ]} (but we don't strictly need to parse pong if connection stays alive)
        while (!token.IsCancellationRequested)
        {
            try
            {
                await Task.Delay(TimeSpan.FromSeconds(15), token);
                await SendMessageAsync("{\"op\":\"ping\"}", token);
                // Console.WriteLine("[BybitFuturesNative] Ping sent");
            }
            catch (OperationCanceledException) { break; }
            catch (Exception ex)
            {
                // Ping error usually means connection issue, loop will break and trigger reconnect
                // Use standard extension LogWarning from Microsoft.Extensions.Logging if specific one is missing, 
                // OR adapt to use LogError which exists
                _logger.LogError("Bybit", "Ping loop error", ex);
                break; 
            }
        }
    }

    private async Task ResubscribeAllAsync(CancellationToken token)
    {
        var symbols = _symbolCallbacks.Keys.ToList();
        if (symbols.Count == 0) return;

        // Console.WriteLine($"[BybitFuturesNative] Re-subscribing to {symbols.Count} symbols...");
        
        // Chunk logic using same SubscribeToTradesAsync method
        // But we need to avoid "re-adding" to dictionary.
        // We can just call the internal subscription sender logic.
        
        // Let's just use the Public subscribe method logic but extracted.
        // Max 10 args per request.
        
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        int count = symbolsList.Count;
        
        for (int i = 0; i < count; i += 10)
        {
             int chunkSize = Math.Min(10, count - i);
             var args = new string[chunkSize];
             for (int j = 0; j < chunkSize; j++)
             {
                 args[j] = $"publicTrade.{symbolsList[i + j]}";
             }

             var msg = JsonSerializer.Serialize(new
             {
                 op = "subscribe",
                 args = args,
                 req_id = Interlocked.Increment(ref _requestId).ToString()
             });
             await SendMessageAsync(msg, token);
             await Task.Delay(50, token); // Rate limit safety
        }
    }

    /// <summary>
    /// Subscribe to trades for multiple symbols (Batched 10 per request).
    /// </summary>
    public async Task SubscribeToTradesAsync(IEnumerable<string> symbols, Func<TradeData, Task> onTrade, CancellationToken cancellationToken = default)
    {
        // OPTIMIZATION: Zero-Allocation iteration
        IList<string> symbolsList = symbols as IList<string> ?? symbols.ToArray();
        if (symbolsList.Count == 0) return;

        // Store callbacks
        foreach (var s in symbolsList)
        {
            // SPRINT 16: Pre-cache symbol to avoid GetString() in hot path
            _symbolCache.GetOrAdd(Encoding.UTF8.GetBytes(s));
            _symbolCallbacks[s] = onTrade;
        }

        // Send subscriptions in chunks of 10
        int count = symbolsList.Count;
        for (int i = 0; i < count; i += 10)
        {
            int chunkSize = Math.Min(10, count - i);
            var args = new string[chunkSize];
            for (int j = 0; j < chunkSize; j++)
            {
                args[j] = $"publicTrade.{symbolsList[i + j]}";
            }

            var msg = JsonSerializer.Serialize(new
            {
                op = "subscribe",
                args = args,
                req_id = Interlocked.Increment(ref _requestId).ToString()
            });

            await SendMessageAsync(msg, cancellationToken);
        }
    }

    /// <summary>
    /// Unsubscribe from trades for multiple symbols.
    /// </summary>
    public async Task UnsubscribeFromTradesAsync(IEnumerable<string> symbols, CancellationToken cancellationToken = default)
    {
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        if (symbolsList.Count == 0) return;

        foreach (var s in symbolsList)
        {
            _symbolCallbacks.TryRemove(s, out _);
        }

        int count = symbolsList.Count;
        for (int i = 0; i < count; i += 10)
        {
             int chunkSize = Math.Min(10, count - i);
             var args = new string[chunkSize];
             for (int j = 0; j < chunkSize; j++)
             {
                 args[j] = $"publicTrade.{symbolsList[i + j]}";
             }

             var msg = JsonSerializer.Serialize(new
             {
                 op = "unsubscribe",
                 args = args,
                 req_id = Interlocked.Increment(ref _requestId).ToString()
             });
             await SendMessageAsync(msg, cancellationToken);
        }
    }

    /// <summary>
    /// Subscribe to tickers channel for real-time bid/ask updates (Batched 10 per request).
    /// Bybit V5 tickers.{symbol} channel includes bid1Price/ask1Price with 100ms push frequency.
    /// </summary>
    public virtual async Task SubscribeToBookTickersAsync(IEnumerable<string> symbols, Func<TickerData, Task> onBookTicker, CancellationToken cancellationToken = default)
    {
        // SPRINT 16: Zero-Allocation iteration - remove LINQ
        IList<string> symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        if (symbolsList.Count == 0) return;

        // Store callbacks
        foreach (var s in symbolsList)
        {
            // SPRINT 16: Pre-cache symbol to avoid GetString() in hot path
            _symbolCache.GetOrAdd(Encoding.UTF8.GetBytes(s));
            _bookTickerCallbacks[s] = onBookTicker;
        }

        // Send subscriptions in chunks of 10
        int count = symbolsList.Count;
        for (int i = 0; i < count; i += 10)
        {
            int chunkSize = Math.Min(10, count - i);
            var args = new string[chunkSize];
            for (int j = 0; j < chunkSize; j++)
            {
                args[j] = $"tickers.{symbolsList[i + j]}";
            }

            var msg = JsonSerializer.Serialize(new
            {
                op = "subscribe",
                args = args,
                req_id = Interlocked.Increment(ref _requestId).ToString()
            });

            await SendMessageAsync(msg, cancellationToken);
        }
    }

    /// <summary>
    /// Unsubscribe from book tickers.
    /// </summary>
    public async Task UnsubscribeFromBookTickersAsync(IEnumerable<string> symbols, CancellationToken cancellationToken = default)
    {
        var symbolsList = symbols as IList<string> ?? new List<string>(symbols);
        if (symbolsList.Count == 0) return;

        foreach (var s in symbolsList)
        {
            _bookTickerCallbacks.TryRemove(s, out _);
        }

        int count = symbolsList.Count;
        for (int i = 0; i < count; i += 10)
        {
             int chunkSize = Math.Min(10, count - i);
             var args = new string[chunkSize];
             for (int j = 0; j < chunkSize; j++)
             {
                 args[j] = $"tickers.{symbolsList[i + j]}";
             }

             var msg = JsonSerializer.Serialize(new
             {
                 op = "unsubscribe",
                 args = args,
                 req_id = Interlocked.Increment(ref _requestId).ToString()
             });

             await SendMessageAsync(msg, cancellationToken);
        }
    }

    /// <summary>
    /// Parse Bybit V5 messages (Zero-Copy with Utf8JsonReader).
    /// Implements WebSocketHotPathBase.OnMessage.
    /// </summary>
    protected override void OnMessage(ReadOnlySpan<byte> message, System.Net.WebSockets.WebSocketMessageType messageType)
    {
        if (messageType != System.Net.WebSockets.WebSocketMessageType.Text) return;

        try
        {
            // Fast check for common messages (op:pong, success:true)
            // Use message span directly
            if (message.IndexOf("\"op\":\"pong\""u8) >= 0) return;
            if (message.IndexOf("\"success\":true"u8) >= 0) return;
            
            var reader = new Utf8JsonReader(message);
            
            string? topic = null;
            string? op = null;
            
            // Ticker data fields
            string? tickerSymbol = null;
            // SPRINT-HOTPATH-001: Parse directly to ticks (zero overhead)
            long bid1PriceTicks = 0;
            long ask1PriceTicks = 0;
            long tickerTs = 0;
            
            // HFT: Reuse instance buffer instead of allocating new List on every message
            var trades = _tradesBuffer;
            trades.Clear();
            
            while (reader.Read())
            {
                if (reader.TokenType == JsonTokenType.PropertyName)
                {
                    if (reader.ValueTextEquals("op"u8))
                    {
                        reader.Read();
                        // SPRINT 16: Use cached symbol (but 'op' is not a symbol, skip cache)
                        op = reader.GetString();
                        // Handled by fast check above
                    }
                    else if (reader.ValueTextEquals("success"u8))
                    {
                        // Subscription confirmation - skip
                        return;
                    }
                    else if (reader.ValueTextEquals("topic"u8))
                    {
                        reader.Read();
                        // SPRINT 16: Cache topic string (zero alloc after first hit)
                        topic = GetOrCacheSymbol(reader.ValueSpan);
                    }
                    else if (reader.ValueTextEquals("data"u8))
                    {
                        reader.Read();
                        
                        // Handle ticker data (object)
                        // PHASE 4-R FIX: Structure-based detection - StartObject = ticker data
                        if (reader.TokenType == JsonTokenType.StartObject)
                        {
                            while (reader.Read() && reader.TokenType != JsonTokenType.EndObject)
                            {
                                if (reader.TokenType == JsonTokenType.PropertyName)
                                {
                                    if (reader.ValueTextEquals("symbol"u8))
                                    {
                                        reader.Read();
                                        // SPRINT 16: Cache symbol (zero alloc after first hit)
                                        tickerSymbol = GetOrCacheSymbol(reader.ValueSpan);
                                    }
                                    else if (reader.ValueTextEquals("bid1Price"u8))
                                    {
                                        reader.Read();
                                        if (reader.TokenType == JsonTokenType.String)
                                        {
                                            // SPRINT-HOTPATH-001: Parse directly to ticks (zero overhead)
                                            FixedPoint8.TryParseUtf8(reader.ValueSpan, out bid1PriceTicks);
                                        }
                                    }
                                    else if (reader.ValueTextEquals("ask1Price"u8))
                                    {
                                        reader.Read();
                                        if (reader.TokenType == JsonTokenType.String)
                                        {
                                            // SPRINT-HOTPATH-001: Parse directly to ticks (zero overhead)
                                            FixedPoint8.TryParseUtf8(reader.ValueSpan, out ask1PriceTicks);
                                        }
                                    }
                                }
                            }
                        }
                        // Handle trade data (array)
                        // PHASE 4-R FIX: Structure-based detection - StartArray = trade data
                        else if (reader.TokenType == JsonTokenType.StartArray)
                        {
                            while (reader.Read() && reader.TokenType != JsonTokenType.EndArray)
                            {
                                if (reader.TokenType == JsonTokenType.StartObject)
                                {
                                    string? symbol = null;
                                    string? side = null;
                                    long priceTicks = 0;
                                    long qtyTicks = 0;
                                    long ts = 0;
                                    
                                    while (reader.Read() && reader.TokenType != JsonTokenType.EndObject)
                                    {
                                        if (reader.TokenType == JsonTokenType.PropertyName)
                                        {
                                            if (reader.ValueTextEquals("s"u8))
                                            {
                                                reader.Read();
                                                // SPRINT 16: Cache symbol (zero alloc after first hit)
                                                symbol = GetOrCacheSymbol(reader.ValueSpan);
                                            }
                                            else if (reader.ValueTextEquals("S"u8))
                                            {
                                                reader.Read();
                                                // SPRINT 16: Cache side string (Buy/Sell reused)
                                                side = GetOrCacheSymbol(reader.ValueSpan);
                                            }
                                            else if (reader.ValueTextEquals("p"u8))
                                            {
                                                reader.Read();
                                                if (reader.TokenType == JsonTokenType.String)
                                                {
                                                    FixedPoint8.TryParseUtf8(reader.ValueSpan, out priceTicks);
                                                }
                                            }
                                            else if (reader.ValueTextEquals("v"u8))
                                            {
                                                reader.Read();
                                                if (reader.TokenType == JsonTokenType.String)
                                                {
                                                    FixedPoint8.TryParseUtf8(reader.ValueSpan, out qtyTicks);
                                                }
                                            }
                                            else if (reader.ValueTextEquals("T"u8))
                                            {
                                                reader.Read();
                                                ts = reader.GetInt64();
                                            }
                                        }
                                    }
                                    
                                    if (!string.IsNullOrEmpty(symbol) && !string.IsNullOrEmpty(side) && priceTicks > 0)
                                    {
                                        trades.Add((symbol, priceTicks, qtyTicks, side, ts));
                                    }
                                }
                            }
                        }
                    }
                    else if (reader.ValueTextEquals("ts"u8))
                    {
                        reader.Read();
                        tickerTs = reader.GetInt64();
                    }
                }
            }
            
            // Process parsed data and invoke callbacks
            
            // Handle ticker callback
            // PHASE 4-R FIX: Removed isTickerData flag, using tickerSymbol presence instead
            // SPRINT-HOTPATH-001: Check ticks directly (zero overhead)
            if (!string.IsNullOrEmpty(tickerSymbol) && bid1PriceTicks > 0 && ask1PriceTicks > 0)
            {
                if (_bookTickerCallbacks.TryGetValue(tickerSymbol, out var tickerCallback))
                {
                    // SPRINT-ALLOC-3-PHASE6: Use ObjectPool for Zero-Allocation
                    var tickerData = TickerDataPool.Get();
                    
                    // SPRINT 16: Already cached symbol (zero alloc)
                    tickerData.Symbol = tickerSymbol; // Already from cache
                    tickerData.BestBidTicks = bid1PriceTicks;
                    tickerData.BestAskTicks = ask1PriceTicks;
                    tickerData.TimestampTicks = tickerTs * 10000; // ms to ticks (1ms = 10000 ticks)
                    
                    // X-RAY: Log BookTicker
                    SpreadAggregator.Application.Diagnostics.XRayLogger.LogBookTicker("Bybit", tickerData);
                    
                    // SPRINT 16: Fast/Slow path - TRUE ZERO ALLOC
                    var task = tickerCallback(tickerData);
                    if (task.IsCompleted)
                    {
                        TickerDataPool.Return(tickerData);
                        // return; // Done
                    }
                    else
                    {
                        // Slow path: async callback
                        // Fire and forget, ensuring cleanup
                        _ = ProcessTickerSlowPath(task, tickerData);
                    }
                }
            }
            
            // Handle trade callbacks
            if (trades.Count > 0)
            {
                // SPRINT 16: Fast/Slow path for trades
                // Fire and forget
                _ = ProcessTrades(trades);
            }
            
            // return;
        }
        catch (Exception ex)
        {
            _logger.LogError("BybitFuturesNative", "Error processing message", ex);
        }
    }
    
    /// <summary>
    /// SPRINT 16: Slow path for ticker callback (only if truly async)
    /// </summary>
    private async Task ProcessTickerSlowPath(Task task, TickerData tickerData)
    {
        try
        {
            await task;
        }
        finally
        {
            TickerDataPool.Return(tickerData);
        }
    }
    
    /// <summary>
    /// SPRINT 16: Fast/Slow path for trade processing - TRUE ZERO ALLOC
    /// </summary>
    private Task ProcessTrades(List<(string symbol, long priceTicks, long qtyTicks, string side, long ts)> trades)
    {
        // Fast path: Check if all callbacks are sync
        Task? pendingTask = null;
        
        for (int i = 0; i < trades.Count; i++)
        {
            var (symbol, priceTicks, qtyTicks, side, ts) = trades[i];
            
            if (_symbolCallbacks.TryGetValue(symbol, out var callback))
            {
                // SPRINT-ALLOC-3-PHASE6: ObjectPool removed for safety (consistency with Binance)
                // Prevents Data Corruption / Race Conditions if callback buffers the object.
                var tradeData = new TradeData();
                
                tradeData.Exchange = "Bybit";
                tradeData.Symbol = symbol; // Already cached
                tradeData.PriceTicks = priceTicks;
                tradeData.QuantityTicks = qtyTicks;
                tradeData.Side = side; // Already cached
                tradeData.TimestampMicros = ts * 1000; // ms to micros

                // X-RAY
                SpreadAggregator.Application.Diagnostics.XRayLogger.LogTrade("Bybit", tradeData);
                
                var task = callback(tradeData);
                
                // Fire-and-forget logic requires awaiting if not completed synchronously
                if (!task.IsCompleted)
                {
                    // Slow path: Need async processing
                    if (pendingTask == null)
                    {
                        pendingTask = task; // Just await the task, no need for helper since no return to pool
                    }
                    else
                    {
                        pendingTask = Task.WhenAll(pendingTask, task);
                    }
                }
            }
        }
        
        return pendingTask ?? Task.CompletedTask;
    }
}


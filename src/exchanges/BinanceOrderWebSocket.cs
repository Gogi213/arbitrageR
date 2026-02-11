using System.Net.WebSockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using SpreadAggregator.Application.Diagnostics;
using SpreadAggregator.Domain.Entities;
using System.Collections.Concurrent;
using System.Globalization;
using System.Buffers;
using Microsoft.Extensions.Logging.Abstractions;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// Binance Futures WebSocket API for order management (HFT).
/// Uses wss://ws-fapi.binance.com for low-latency order operations.
/// </summary>
public class BinanceOrderWebSocket : IDisposable
{
    private const string WS_ENDPOINT = "wss://ws-fapi.binance.com/ws-fapi/v1";
    private const string WS_TESTNET = "wss://testnet.binancefuture.com/ws-fapi/v1";
    
    private ClientWebSocket? _ws;
    private string _apiKey = "";
    private string _apiSecret = "";
    private bool _useTestnet = true;
    
    // Request/response tracking
    private int _requestId = 0;
    private readonly ConcurrentDictionary<string, TaskCompletionSource<JsonDocument>> _pendingRequests = new();
    
    // Symbol info cache
    private readonly ConcurrentDictionary<string, SymbolInfo> _symbolCache = new();
    
    // Connection state
    private volatile bool _isConnected = false;
    private volatile bool _reconnecting = false;
    private CancellationTokenSource? _cts;
    private Task? _receiveTask;
    
    // Reconnection config
    private const int MAX_RECONNECT_ATTEMPTS = 10;
    private static readonly int[] BACKOFF_MS = { 1000, 2000, 4000, 8000, 16000, 30000 };
    
    // Cached HMAC for HFT (avoid allocation per request)
    private HMACSHA256? _hmac;

    // --- HOT PATH COMPONENTS (Phase 6) ---
    private readonly OrderWebSocketHotPath _hotPath;
    private readonly OrderEventProcessor _processor;
    private readonly OrderStateManager _stateManager;
    private readonly bool _enableHotPath = true; // Feature flag
    
    public bool IsConnected => _isConnected;

    public BinanceOrderWebSocket()
    {
        // Composition Root for Hot Path components
        // In a full DI scenario, these would be injected, but keeping existing constructor for compat
        _stateManager = new OrderStateManager();
        _processor = new OrderEventProcessor(NullLogger<OrderEventProcessor>.Instance);
        _hotPath = new OrderWebSocketHotPath();

        // Wire up routing
        // Route all statuses to processor and state manager
        // We could optimize by only routing specific statuses if needed
        foreach (OrderStatus status in Enum.GetValues(typeof(OrderStatus)))
        {
            _hotPath.RegisterHandler(status, (in OrderEvent evt) => 
            {
                _stateManager.TrackOrder(in evt);
                _processor.ProcessExecutionReport(in evt);
            });
        }
    }
    
    public void SetCredentials(string apiKey, string apiSecret, bool useTestnet = true)
    {
        _apiKey = apiKey;
        _apiSecret = apiSecret;
        _useTestnet = useTestnet;
        TradeLogger.Log("WS", $"Credentials set (key ends with ...{apiKey.Substring(Math.Max(0, apiKey.Length - 4))})");
    }
    
    public async Task ConnectAsync()
    {
        if (_isConnected) return;
        
        _ws = new ClientWebSocket();
        _ws.Options.KeepAliveInterval = TimeSpan.FromSeconds(5); // HFT: Aggressive KeepAlive
        _cts = new CancellationTokenSource();
        
        var endpoint = _useTestnet ? WS_TESTNET : WS_ENDPOINT;
        TradeLogger.Log("WS", $"Connecting to {endpoint}...");
        
        await _ws.ConnectAsync(new Uri(endpoint), _cts.Token);
        _isConnected = true;
        
        // Start receive loop
        _receiveTask = ReceiveLoopAsync(_cts.Token);
        
        TradeLogger.Log("WS", "✅ Connected");
    }
    
    /// <summary>
    /// Place a limit order via WebSocket
    /// </summary>
    public async Task<(bool success, long orderId, string error)> PlaceOrderAsync(
        string symbol, string side, decimal price, decimal quantity, bool reduceOnly = false)
    {
        var symbolInfo = _symbolCache.GetValueOrDefault(symbol);
        if (symbolInfo != null)
        {
            price = RoundPrice(price, symbolInfo);
            quantity = RoundQuantity(quantity, symbolInfo);
        }
        
        var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var binanceSymbol = symbol.Replace("_", "");
        
        // Build params for signature (alphabetically sorted)
        var signParams = new SortedDictionary<string, string>
        {
            ["apiKey"] = _apiKey,
            ["price"] = price.ToString(CultureInfo.InvariantCulture),
            ["quantity"] = quantity.ToString(CultureInfo.InvariantCulture),
            ["recvWindow"] = "5000",
            ["side"] = side,
            ["symbol"] = binanceSymbol,
            ["timeInForce"] = "GTC",
            ["timestamp"] = timestamp.ToString(),
            ["type"] = "LIMIT"
        };
        
        var queryString = string.Join("&", signParams.Select(kv => $"{kv.Key}={kv.Value}"));
        var signature = SignHmac(queryString);
        
        var requestId = Guid.NewGuid().ToString();
        
        // Build request with optional reduceOnly
        object requestParams;
        if (reduceOnly)
        {
            requestParams = new
            {
                apiKey = _apiKey,
                symbol = binanceSymbol,
                side = side,
                type = "LIMIT",
                timeInForce = "GTC",
                price = price.ToString(CultureInfo.InvariantCulture),
                quantity = quantity.ToString(CultureInfo.InvariantCulture),
                reduceOnly = "true",
                recvWindow = 5000,
                timestamp = timestamp,
                signature = SignHmac(queryString + "&reduceOnly=true")
            };
        }
        else
        {
            requestParams = new
            {
                apiKey = _apiKey,
                symbol = binanceSymbol,
                side = side,
                type = "LIMIT",
                timeInForce = "GTC",
                price = price.ToString(CultureInfo.InvariantCulture),
                quantity = quantity.ToString(CultureInfo.InvariantCulture),
                recvWindow = 5000,
                timestamp = timestamp,
                signature = signature
            };
        }
        
        var request = new
        {
            id = requestId,
            method = "order.place",
            @params = requestParams
        };
        
        try
        {
            var response = await SendRequestAsync(requestId, request);
            
            if (response.RootElement.TryGetProperty("result", out var result))
            {
                var orderId = result.GetProperty("orderId").GetInt64();
                TradeLogger.LogOrderPlaced(symbol, side, price, quantity, orderId);
                return (true, orderId, "");
            }
            
            if (response.RootElement.TryGetProperty("error", out var error))
            {
                var errorMsg = error.GetProperty("msg").GetString() ?? "Unknown error";
                TradeLogger.LogError("WS_PLACE", errorMsg);
                return (false, 0, errorMsg);
            }
            
            return (false, 0, "Unknown response format");
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("WS_PLACE", ex.Message);
            return (false, 0, ex.Message);
        }
    }
    
    /// <summary>
    /// Modify an order via WebSocket
    /// </summary>
    public async Task<(bool success, string error)> ModifyOrderAsync(
        string symbol, long orderId, string side, decimal newPrice, decimal quantity,
        System.Collections.Generic.Dictionary<string, object>? auditData = null,
        bool reduceOnly = false)
    {
        var symbolInfo = _symbolCache.GetValueOrDefault(symbol);
        if (symbolInfo != null)
        {
            newPrice = RoundPrice(newPrice, symbolInfo);
            quantity = RoundQuantity(quantity, symbolInfo);
        }
        
        var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var binanceSymbol = symbol.Replace("_", "");
        
        // Build params for signature
        var signParams = new SortedDictionary<string, string>
        {
            ["apiKey"] = _apiKey,
            ["orderId"] = orderId.ToString(),
            ["origType"] = "LIMIT",
            ["price"] = newPrice.ToString(CultureInfo.InvariantCulture),
            ["priceMatch"] = "NONE",
            ["quantity"] = quantity.ToString(CultureInfo.InvariantCulture),
            ["side"] = side,
            ["symbol"] = binanceSymbol,
            ["timestamp"] = timestamp.ToString()
        };
        
        // Add reduceOnly if true (must be part of signature)
        if (reduceOnly)
        {
            signParams["reduceOnly"] = "true";
        }
        
        var queryString = string.Join("&", signParams.Select(kv => $"{kv.Key}={kv.Value}"));
        var signature = SignHmac(queryString);
        
        var requestId = Guid.NewGuid().ToString();
        
        // Build request parameters
        // Dynamic object creation to handle optional reduceOnly
        object paramsObj;
        
        if (reduceOnly)
        {
             paramsObj = new
            {
                apiKey = _apiKey,
                symbol = binanceSymbol,
                orderId = orderId,
                side = side,
                origType = "LIMIT",
                priceMatch = "NONE",
                price = newPrice.ToString(CultureInfo.InvariantCulture),
                quantity = quantity.ToString(CultureInfo.InvariantCulture),
                reduceOnly = "true",
                timestamp = timestamp,
                signature = signature
            };
        }
        else
        {
            paramsObj = new
            {
                apiKey = _apiKey,
                symbol = binanceSymbol,
                orderId = orderId,
                side = side,
                origType = "LIMIT",
                priceMatch = "NONE",
                price = newPrice.ToString(CultureInfo.InvariantCulture),
                quantity = quantity.ToString(CultureInfo.InvariantCulture),
                timestamp = timestamp,
                signature = signature
            };
        }

        var request = new
        {
            id = requestId,
            method = "order.modify",
            @params = paramsObj
        };
        
        try
        {
            var response = await SendRequestAsync(requestId, request);
            
            if (response.RootElement.TryGetProperty("result", out _))
            {
                TradeLogger.LogOrderModified(symbol, orderId, 0, newPrice, auditData);
                if (reduceOnly) TradeLogger.Log("API", $"🔒 Modified #{orderId} with ReduceOnly=true");
                return (true, "");
            }
            
            if (response.RootElement.TryGetProperty("error", out var error))
            {
                var errorMsg = error.GetProperty("msg").GetString() ?? "Unknown error";
                return (false, errorMsg);
            }
            
            return (false, "Unknown response format");
        }
        catch (Exception ex)
        {
            return (false, ex.Message);
        }
    }

    /// <summary>
    /// Cancel an order via WebSocket
    /// </summary>
    public async Task CancelOrderAsync(string symbol, long orderId)
    {
        var timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var binanceSymbol = symbol.Replace("_", "");
        
        // Build params for signature
        var signParams = new SortedDictionary<string, string>
        {
            ["apiKey"] = _apiKey,
            ["orderId"] = orderId.ToString(),
            ["symbol"] = binanceSymbol,
            ["timestamp"] = timestamp.ToString()
        };
        
        var queryString = string.Join("&", signParams.Select(kv => $"{kv.Key}={kv.Value}"));
        var signature = SignHmac(queryString);
        
        var requestId = Guid.NewGuid().ToString();
        var request = new
        {
            id = requestId,
            method = "order.cancel",
            @params = new
            {
                apiKey = _apiKey,
                symbol = binanceSymbol,
                orderId = orderId,
                timestamp = timestamp,
                signature = signature
            }
        };
        
        try
        {
            var response = await SendRequestAsync(requestId, request);
            
            if (response.RootElement.TryGetProperty("result", out _))
            {
                TradeLogger.Log("WS_CANCEL", $"Canceled {symbol} #{orderId}");
                return;
            }
            
            if (response.RootElement.TryGetProperty("error", out var error))
            {
                var errorMsg = error.GetProperty("msg").GetString() ?? "Unknown error";
                TradeLogger.LogError("WS_CANCEL", errorMsg);
            }
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("WS_CANCEL", ex.Message);
        }
    }
    
    public void CacheSymbolInfo(string symbol, SymbolInfo info)
    {
        _symbolCache[symbol] = info;
    }
    
    private async Task<JsonDocument> SendRequestAsync(string requestId, object request)
    {
        if (!_isConnected || _ws == null)
        {
            throw new InvalidOperationException("WebSocket not connected");
        }
        
        var json = JsonSerializer.Serialize(request);
        var bytes = Encoding.UTF8.GetBytes(json);
        
        // Track request (no re-parsing needed - requestId passed explicitly)
        var tcs = new TaskCompletionSource<JsonDocument>();
        _pendingRequests[requestId] = tcs;
        
        try
        {
            await _ws.SendAsync(new ArraySegment<byte>(bytes), WebSocketMessageType.Text, true, _cts!.Token);
            
            // Wait for response with timeout (cleaner pattern)
            return await tcs.Task.WaitAsync(TimeSpan.FromSeconds(5));
        }
        catch (TimeoutException)
        {
            throw new TimeoutException("Request timed out");
        }
        finally
        {
            _pendingRequests.TryRemove(requestId, out _);
        }
    }
    
    private async Task ReceiveLoopAsync(CancellationToken ct)
    {
        var buffer = ArrayPool<byte>.Shared.Rent(8192);
        var messageBuffer = new MemoryStream();
        bool shouldReconnect = false;
        
        try
        {
            while (!ct.IsCancellationRequested && _ws?.State == WebSocketState.Open)
            {
                try
                {
                    messageBuffer.SetLength(0);
                    WebSocketReceiveResult result;
                    
                    do
                    {
                        result = await _ws.ReceiveAsync(new ArraySegment<byte>(buffer), ct);
                        messageBuffer.Write(buffer, 0, result.Count);
                    } while (!result.EndOfMessage);
                    
                    if (result.MessageType == WebSocketMessageType.Text)
                    {
                        // HOT PATH: Process bytes directly first (Phase 6)
                        // This avoids allocating a string for every execution report
                        var messageSpan = new ReadOnlySpan<byte>(messageBuffer.GetBuffer(), 0, (int)messageBuffer.Length);
                        
                        if (_enableHotPath && _hotPath.TryParseOrderEvent(messageSpan, out var evt))
                        {
                            // It's an execution report! Fast path.
                            _hotPath.RouteEvent(in evt);
                        }
                        else if (_hotPath.TryParseResponseId(messageSpan, out var requestId))
                        {
                            // It's a response to a request (has "id")
                            // We need to parse it fully to get the result
                            // This path allocates, but it's user-initiated (low frequency)
                            if (requestId != null && _pendingRequests.TryRemove(requestId, out var tcs))
                            {
                                try 
                                {
                                    var json = Encoding.UTF8.GetString(messageBuffer.GetBuffer(), 0, (int)messageBuffer.Length);
                                    var doc = JsonDocument.Parse(json);
                                    tcs.SetResult(doc);
                                }
                                catch (Exception ex)
                                {
                                    tcs.SetException(ex);
                                }
                            }
                        }
                        else
                        {
                            // Unknown message or keep-alive?
                            // Avoid parsing unless debug logging is on?
                            // For now, we assume if it's not OrderEvent and not Response, we can ignore or log sample
                            // TradeLogger.Log("WS_MSG", "Ignored message"); 
                        }
                    }
                    else if (result.MessageType == WebSocketMessageType.Close)
                    {
                        _isConnected = false;
                        TradeLogger.Log("WS", "Connection closed by server");
                        shouldReconnect = true;
                        break;
                    }
                }
                catch (OperationCanceledException)
                {
                    break; // Graceful shutdown, no reconnect
                }
                catch (WebSocketException ex)
                {
                    TradeLogger.LogError("WS_RECV", $"WebSocket error: {ex.Message}");
                    _isConnected = false;
                    shouldReconnect = true;
                    break;
                }
                catch (Exception ex)
                {
                    TradeLogger.LogError("WS_RECV", ex.Message);
                }
            }
            
            // Check if we exited due to connection drop (not cancellation)
            if (!ct.IsCancellationRequested && !_isConnected)
            {
                shouldReconnect = true;
            }
        }
        finally
        {
            ArrayPool<byte>.Shared.Return(buffer);
        }
        
        // Trigger reconnect outside of finally block
        if (shouldReconnect && !ct.IsCancellationRequested)
        {
            _ = ReconnectAsync();
        }
    }
    
    /// <summary>
    /// Auto-reconnect with exponential backoff
    /// </summary>
    private async Task ReconnectAsync()
    {
        if (_reconnecting) return;
        _reconnecting = true;
        
        for (int attempt = 0; attempt < MAX_RECONNECT_ATTEMPTS; attempt++)
        {
            if (_cts?.IsCancellationRequested == true) break;
            
            var delayMs = BACKOFF_MS[Math.Min(attempt, BACKOFF_MS.Length - 1)];
            TradeLogger.Log("WS", $"Reconnecting in {delayMs}ms (attempt {attempt + 1}/{MAX_RECONNECT_ATTEMPTS})");
            
            await Task.Delay(delayMs);
            
            try
            {
                _ws?.Dispose();
                _ws = new ClientWebSocket();
                _ws.Options.KeepAliveInterval = TimeSpan.FromSeconds(5); // HFT: Aggressive KeepAlive
                var endpoint = _useTestnet ? WS_TESTNET : WS_ENDPOINT;
                await _ws.ConnectAsync(new Uri(endpoint), _cts!.Token);
                _isConnected = true;
                _reconnecting = false;
                
                // Restart receive loop
                _receiveTask = ReceiveLoopAsync(_cts.Token);
                TradeLogger.Log("WS", "✅ Reconnected successfully");
                return;
            }
            catch (Exception ex)
            {
                TradeLogger.LogError("WS_RECONNECT", $"Attempt {attempt + 1} failed: {ex.Message}");
            }
        }
        
        _reconnecting = false;
        TradeLogger.LogError("WS", $"Failed to reconnect after {MAX_RECONNECT_ATTEMPTS} attempts");
    }
    
    internal void ProcessMessage(string json)
    {
        try
        {
            var doc = JsonDocument.Parse(json);
            
            if (doc.RootElement.TryGetProperty("id", out var idProp))
            {
                var id = idProp.GetString();
                if (id != null && _pendingRequests.TryRemove(id, out var tcs))
                {
                    tcs.SetResult(doc);
                    return;
                }
            }
            
            // Other messages (e.g., stream data)
            TradeLogger.Log("WS_MSG", json.Length > 200 ? json[..200] + "..." : json);
        }
        catch (Exception ex)
        {
            TradeLogger.LogError("WS_PARSE", ex.Message);
        }
    }
    
    private string SignHmac(string queryString)
    {
        // Lazy init HMAC (cached for HFT performance)
        _hmac ??= new HMACSHA256(Encoding.UTF8.GetBytes(_apiSecret));
        var hash = _hmac.ComputeHash(Encoding.UTF8.GetBytes(queryString));
        return BitConverter.ToString(hash).Replace("-", "").ToLowerInvariant();
    }
    
    private static decimal RoundPrice(decimal price, SymbolInfo symbolInfo)
    {
        var tickSize = symbolInfo.PriceStep;
        return Math.Round(price / tickSize) * tickSize;
    }
    
    private static decimal RoundQuantity(decimal qty, SymbolInfo symbolInfo)
    {
        var stepSize = symbolInfo.QuantityStep;
        return Math.Round(qty / stepSize) * stepSize;
    }
    
    public void Dispose()
    {
        _cts?.Cancel();
        _ws?.Dispose();
        _cts?.Dispose();
    }
}

using System;
using System.Buffers;
using System.Collections.Concurrent;
using System.IO;
using System.Net.WebSockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using SpreadAggregator.Application.Abstractions;
using SpreadAggregator.Application.Diagnostics;
using SpreadAggregator.Domain.Entities;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// HFT/Zero-Alloc WebSocket client for Bybit V5 Trade Stream.
/// Supports connection, authentication, and order entry.
/// </summary>
public sealed class BybitOrderWebSocket : IDisposable
{
    private const string WS_ENDPOINT = "wss://stream.bybit.com/v5/trade";
    private const string WS_TESTNET = "wss://stream-testnet.bybit.com/v5/trade";
    
    private ClientWebSocket? _ws;
    private string _apiKey = "";
    private string _apiSecret = ""; // Kept as string for HMAC init, but used via Span
    private bool _useTestnet = true;
    
    // Cached crypto for HFT (avoid per-request allocation)
    private HMACSHA256? _hmac;
    private readonly byte[] _hmacBuffer = new byte[32]; // SHA256 output size
    private readonly char[] _hexBuffer = new char[64];  // Hex string output size
    
    // Buffers (Pooled/Pinned)
    private readonly ArrayBufferWriter<byte> _sendBuffer = new(4096);
    private readonly byte[] _recvBuffer = new byte[65536]; // 64KB receive buffer
    
    // State
    private volatile bool _isConnected = false;
    private CancellationTokenSource? _cts;
    private Task? _receiveTask;
    
    private readonly ILogger<BybitOrderWebSocket> _logger;
    
    // Request tracking
    private readonly ConcurrentDictionary<string, TaskCompletionSource<bool>> _pendingAuth = new();
    
    // Symbol info cache for step size / tick size
    private readonly ConcurrentDictionary<string, SymbolInfo> _symbolCache = new();

    public bool IsConnected => _isConnected;

    public BybitOrderWebSocket(ILogger<BybitOrderWebSocket>? logger = null)
    {
        _logger = logger ?? NullLogger<BybitOrderWebSocket>.Instance;
    }

    public void SetCredentials(string apiKey, string apiSecret, bool useTestnet = true)
    {
        _apiKey = apiKey;
        _apiSecret = apiSecret;
        _useTestnet = useTestnet;
        _hmac?.Dispose();
        _hmac = new HMACSHA256(Encoding.UTF8.GetBytes(apiSecret));
    }

    public async Task ConnectAsync()
    {
        if (_isConnected) return;
        
        _ws = new ClientWebSocket();
        _ws.Options.KeepAliveInterval = TimeSpan.FromSeconds(20); // Bybit recommendation: 20s
        _cts = new CancellationTokenSource();
        
        var endpoint = _useTestnet ? WS_TESTNET : WS_ENDPOINT;
        _logger.LogInformation("Connecting to Bybit Trade Stream: {Endpoint}", endpoint);
        
        await _ws.ConnectAsync(new Uri(endpoint), _cts.Token);
        _isConnected = true;
        
        // Start receive loop
        _receiveTask = ReceiveLoopAsync(_cts.Token);
        
        // Authenticate immediately
        await AuthenticateAsync();
        
        _logger.LogInformation("✅ Connected and Authenticated to Bybit");
    }
    
    public async Task AuthenticateAsync()
    {
        if (string.IsNullOrEmpty(_apiKey) || string.IsNullOrEmpty(_apiSecret))
            throw new InvalidOperationException("Credentials not set");

        var expires = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() + 10000; // 10s valid window
        
        // Zero-Alloc Payload Generation
        _sendBuffer.Clear();
        using (var writer = new Utf8JsonWriter(_sendBuffer))
        {
            WriteAuthPayload(writer, _apiKey, _apiSecret, expires, _hmac, _hmacBuffer, _hexBuffer);
        }
        
        // Send
        await _ws.SendAsync(_sendBuffer.WrittenMemory, WebSocketMessageType.Text, true, _cts!.Token);
        
        // Wait for response (simplified for now - assume success or throw on error in receive loop)
        // TODO: Implement proper request/response correlation for Auth
    }

    /// <summary>
    /// Writes the Auth payload directly to the Utf8JsonWriter (Zero Alloc).
    /// </summary>
    internal static void WriteAuthPayload(
        Utf8JsonWriter writer, 
        ReadOnlySpan<char> apiKey, 
        ReadOnlySpan<char> apiSecret, 
        long expires,
        HMACSHA256? hmac = null,
        Span<byte> hmacBuffer = default,
        Span<char> hexBuffer = default)
    {
        // Calculate Signature: HMAC_SHA256(secret, "GET/realtime" + expires)
        // Note: For unit testing static method, we might need to create HMAC here if null
        bool disposeHmac = false;
        if (hmac == null)
        {
            hmac = new HMACSHA256(Encoding.UTF8.GetBytes(apiSecret.ToString()));
            disposeHmac = true;
            hmacBuffer = new byte[32];
            hexBuffer = new char[64];
        }

        try
        {
            // Compute Hash into buffer
            // 1. "GET/realtime"
            // 2. expires (long to string)
            
            // Allocation-free approach:
            // Need a buffer for the data to sign: "GET/realtime" (12 chars) + expires (13 digits) = 25 chars
            Span<byte> dataToSign = stackalloc byte[32]; 
            int bytesWritten = Encoding.UTF8.GetBytes("GET/realtime".AsSpan(), dataToSign);
            
            // Format expires directly into the buffer after "GET/realtime"
            if (System.Buffers.Text.Utf8Formatter.TryFormat(expires, dataToSign.Slice(bytesWritten), out int expiresBytes))
            {
                bytesWritten += expiresBytes;
            }
            
            // Compute HMAC
            hmac.TryComputeHash(dataToSign.Slice(0, bytesWritten), hmacBuffer, out _);
            
            // Convert to Hex
            // BitConverter is slow/allocating. Custom loop is faster.
            // Or use Convert.ToHexString (Net 5+)
            // Since we target zero-alloc, let's write directly to hexBuffer
            ToHex(hmacBuffer, hexBuffer);

            // Write JSON
            writer.WriteStartObject();
            writer.WriteString("op", "auth");
            writer.WriteStartArray("args");
            writer.WriteStringValue(apiKey);
            writer.WriteNumberValue(expires);
            writer.WriteStringValue(hexBuffer); // Write Hex String
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        finally
        {
            if (disposeHmac) hmac.Dispose();
        }
    }
    
    private static void ToHex(ReadOnlySpan<byte> bytes, Span<char> chars)
    {
        for (int i = 0; i < bytes.Length; i++)
        {
            byte b = bytes[i];
            chars[i * 2] = GetHexValue(b >> 4);
            chars[i * 2 + 1] = GetHexValue(b & 0xF);
        }
    }

    private static char GetHexValue(int i)
    {
        return (char)(i < 10 ? i + '0' : i - 10 + 'a'); // Lowercase hex
    }

    private async Task ReceiveLoopAsync(CancellationToken ct)
    {
        var buffer = new ArraySegment<byte>(_recvBuffer);
        
        try
        {
            while (!ct.IsCancellationRequested && _ws?.State == WebSocketState.Open)
            {
                var result = await _ws.ReceiveAsync(buffer, ct);
                
                if (result.MessageType == WebSocketMessageType.Close)
                {
                    _logger.LogWarning("Bybit connection closed by server");
                    break;
                }
                
                if (result.Count > 0)
                {
                    // Process message (Zero Alloc Parsing)
                    ProcessMessage(new ReadOnlySpan<byte>(_recvBuffer, 0, result.Count));
                }
            }
        }
        catch (OperationCanceledException) { }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Bybit Receive Error");
        }
        finally
        {
            _isConnected = false;
        }
    }
    
    private void ProcessMessage(ReadOnlySpan<byte> message)
    {
        // Simple log for now (Sprint 37.1 focuses on Auth)
        // In Sprint 37.2 we parse order responses
        if (_logger.IsEnabled(LogLevel.Debug))
        {
            var str = Encoding.UTF8.GetString(message);
            _logger.LogDebug("RECV: {Msg}", str);
        }
    }

    /// <summary>
    /// Places a market order via WebSocket (Zero-Alloc Hot Path).
    /// </summary>
    public async Task<(bool success, string orderId, string error)> PlaceMarketOrderAsync(
        string symbol,
        string side,
        long quantityTicks)
    {
        if (!_isConnected || _ws == null)
            return (false, "", "WebSocket not connected");

        var requestId = GenerateRequestId();
        var tcs = new TaskCompletionSource<(bool success, string orderId, string error)>();
        _pendingOrders[requestId] = tcs;

        try
        {
            // Zero-Alloc Payload Generation
            _sendBuffer.Clear();
            using (var writer = new Utf8JsonWriter(_sendBuffer))
            {
                WriteMarketOrderPayload(writer, symbol, side, quantityTicks, requestId);
            }

            // Send
            await _ws.SendAsync(_sendBuffer.WrittenMemory, WebSocketMessageType.Text, true, _cts!.Token);

            // Wait for response with timeout
            using var ctsTimeout = new CancellationTokenSource(TimeSpan.FromSeconds(5));
            return await tcs.Task.WaitAsync(ctsTimeout.Token);
        }
        catch (TimeoutException)
        {
            _pendingOrders.TryRemove(requestId, out _);
            return (false, "", "Request timeout");
        }
        catch (Exception ex)
        {
            _pendingOrders.TryRemove(requestId, out _);
            return (false, "", ex.Message);
        }
    }

    /// <summary>
    /// Writes Market Order payload directly to Utf8JsonWriter (Zero Alloc).
    /// Format: {"op":"order.create","args":[{"category":"linear","symbol":"BTCUSDT","side":"Buy","orderType":"Market","qty":"1.0"}]}
    /// </summary>
    internal static void WriteMarketOrderPayload(
        Utf8JsonWriter writer,
        ReadOnlySpan<char> symbol,
        ReadOnlySpan<char> side,
        long quantityTicks,
        string requestId = "")
    {
        writer.WriteStartObject();
        
        if (!string.IsNullOrEmpty(requestId))
        {
            writer.WriteString("id", requestId);
        }
        
        writer.WriteString("op", "order.create");
        writer.WriteStartArray("args");
        writer.WriteStartObject();
        writer.WriteString("category", "linear");
        writer.WriteString("symbol", symbol);
        writer.WriteString("side", side);
        writer.WriteString("orderType", "Market");
        
        // Convert ticks to decimal string (Zero-Alloc)
        // quantityTicks is FixedPoint8 (10^8)
        Span<char> qtyBuffer = stackalloc char[32];
        var qtySpan = FormatFixedPoint8(quantityTicks, qtyBuffer);
        writer.WriteString("qty", qtySpan);
        
        writer.WriteEndObject();
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    /// <summary>
    /// Formats FixedPoint8 ticks as decimal string (e.g., 100000000 -> "1.00000000").
    /// Zero-allocation using Span.
    /// </summary>
    private static ReadOnlySpan<char> FormatFixedPoint8(long ticks, Span<char> buffer)
    {
        // Handle negative
        bool negative = ticks < 0;
        if (negative) ticks = -ticks;

        // Split into whole and fractional parts
        long whole = ticks / 100_000_000;
        long frac = ticks % 100_000_000;

        int pos = 0;
        if (negative)
        {
            buffer[pos++] = '-';
        }

        // Write whole part
        if (whole == 0)
        {
            buffer[pos++] = '0';
        }
        else
        {
            // Count digits in whole
            int wholeDigits = 0;
            long temp = whole;
            while (temp > 0)
            {
                wholeDigits++;
                temp /= 10;
            }

            // Write whole part backwards then reverse
            int startPos = pos;
            temp = whole;
            while (temp > 0)
            {
                buffer[pos++] = (char)('0' + (temp % 10));
                temp /= 10;
            }
            // Reverse the whole part digits
            int endPos = pos - 1;
            while (startPos < endPos)
            {
                (buffer[startPos], buffer[endPos]) = (buffer[endPos], buffer[startPos]);
                startPos++;
                endPos--;
            }
        }

        // Write decimal point
        buffer[pos++] = '.';

        // Write fractional part (always 8 digits, with leading zeros)
        for (int i = 7; i >= 0; i--)
        {
            buffer[pos + i] = (char)('0' + (frac % 10));
            frac /= 10;
        }
        pos += 8;

        return buffer.Slice(0, pos);
    }

    /// <summary>
    /// Generates unique request ID for WebSocket correlation.
    /// </summary>
    private string GenerateRequestId()
    {
        // Simple incrementing ID with prefix
        var id = Interlocked.Increment(ref _requestIdCounter);
        return $"bybit_{id}";
    }

    // Request tracking for orders
    private long _requestIdCounter = 0;
    private readonly ConcurrentDictionary<string, TaskCompletionSource<(bool success, string orderId, string error)>> _pendingOrders = new();

    public void CacheSymbolInfo(string symbol, SymbolInfo info)
    {
        _symbolCache[symbol] = info;
    }

    public void Dispose()
    {
        _cts?.Cancel();
        _ws?.Dispose();
        _hmac?.Dispose();
        _cts?.Dispose();
    }
}

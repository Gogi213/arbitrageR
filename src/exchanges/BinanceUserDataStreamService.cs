using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using SpreadAggregator.Application.Diagnostics;
using System.Globalization;

using SpreadAggregator.Domain.Entities;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// Binance Futures User Data Stream for real-time order/position updates.
/// Provides instant fill notifications instead of guessing from modify errors.
/// </summary>
public class BinanceUserDataStreamService : IDisposable
{
    private const string WS_BASE_URL = "wss://fstream.binance.com/ws/";
    
    private readonly HttpClient _httpClient;
    private readonly BinanceListenKeyManager _listenKeyManager;
    private readonly UserDataHotPath _hotPath;
    private readonly AccountStateManager _stateManager;
    
    private ClientWebSocket? _ws;
    private string _apiKey = "";
    private string _listenKey = "";
    
    private CancellationTokenSource? _cts;
    private Task? _receiveTask;
    private bool _isConnected = false;
    
    // Events for order updates
    public event Action<OrderFillEvent>? OnOrderFilled;
    public event Action<long, long>? OnOrderCanceled;  // symbolId, orderId - ZERO ALLOC
    
    public bool IsConnected => _isConnected;
    
    public BinanceUserDataStreamService(HttpClient httpClient)
    {
        _httpClient = httpClient;
        _listenKeyManager = new BinanceListenKeyManager(httpClient);
        _listenKeyManager.OnListenKeyChanged += OnListenKeyChanged;
        
        _hotPath = new UserDataHotPath();
        _stateManager = new AccountStateManager(_hotPath.GetSymbolById);
        
        // Wire up HotPath -> StateManager -> Service Events
        _hotPath.OnOrderUpdate += (evt) => _stateManager.ProcessOrderUpdate(in evt);
        _hotPath.OnBalanceUpdate += (evt) => _stateManager.ProcessBalanceUpdate(in evt);
        _hotPath.OnPositionUpdate += (evt) => _stateManager.ProcessPositionUpdate(in evt);
        
        // Wire up StateManager -> Service Events
        _stateManager.OnOrderFilled += (evt) => OnOrderFilled?.Invoke(evt);
        // ZERO ALLOC: Pass symbolId directly, no string resolution on hot path
        _stateManager.OnOrderCanceled += (symbolId, orderId) => OnOrderCanceled?.Invoke(symbolId, orderId);
    }
    
    /// <summary>
    /// Start the User Data Stream
    /// </summary>
    public async Task StartAsync(string apiKey)
    {
        _apiKey = apiKey;
        _cts = new CancellationTokenSource();
        
        // 1. Get ListenKey via Manager (Phase 2)
        _listenKey = await _listenKeyManager.StartAsync(apiKey);
        
        if (string.IsNullOrEmpty(_listenKey))
        {
            if (TradeLogger.IsEnabled)
                TradeLogger.LogError("UserDataStream", "Failed to get listenKey");
            return;
        }
        
        if (TradeLogger.IsEnabled)
            TradeLogger.Log("UserDataStream", "Got listenKey: ..." + _listenKey.Substring(_listenKey.Length - 8));
        
        // 2. Connect to WebSocket
        await ConnectWebSocketAsync();
        
        if (TradeLogger.IsEnabled)
            TradeLogger.Log("UserDataStream", "✅ Started");
    }

    private void OnListenKeyChanged(string newKey)
    {
         if (TradeLogger.IsEnabled)
             TradeLogger.Log("UserDataStream", "ListenKey changed/renewed: ..." + newKey.Substring(newKey.Length - 8));
         _listenKey = newKey;
         if (_isConnected)
         {
             _ = ReconnectAsync();
         }
    }
    
    /// <summary>
    /// Connect to the User Data Stream WebSocket
    /// </summary>
    private async Task ConnectWebSocketAsync()
    {
        _ws = new ClientWebSocket();
        
        var endpoint = $"{WS_BASE_URL}{_listenKey}";
        if (TradeLogger.IsEnabled)
            TradeLogger.Log("UserDataStream", "Connecting to WebSocket...");
        
        await _ws.ConnectAsync(new Uri(endpoint), _cts!.Token);
        _isConnected = true;
        
        // Start receive loop
        _receiveTask = ReceiveLoopAsync(_cts.Token);
        
        if (TradeLogger.IsEnabled)
            TradeLogger.Log("UserDataStream", "✅ WebSocket connected");
    }
    
    /// <summary>
    /// Main receive loop for WebSocket messages
    /// </summary>
    private async Task ReceiveLoopAsync(CancellationToken ct)
    {
        var buffer = new byte[8192];
        var messageBuffer = new MemoryStream(); // Reusable buffer accumulator
        
        while (!ct.IsCancellationRequested && _ws?.State == WebSocketState.Open)
        {
            try
            {
                messageBuffer.SetLength(0); // Reset buffer pointer, keep capacity
                WebSocketReceiveResult result;
                
                do
                {
                    result = await _ws.ReceiveAsync(new ArraySegment<byte>(buffer), ct);
                    messageBuffer.Write(buffer, 0, result.Count);
                } while (!result.EndOfMessage);
                
                if (result.MessageType == WebSocketMessageType.Text)
                {
                    // Zero Alloc: Pass the span of the memory stream's buffer directly
                    if (messageBuffer.TryGetBuffer(out ArraySegment<byte> segment))
                    {
                        _hotPath.ProcessMessage(segment.AsSpan(0, (int)messageBuffer.Length));
                    }
                    else 
                    {
                        // Fallback if GetBuffer fails (unlikely for MemoryStream)
                        _hotPath.ProcessMessage(messageBuffer.ToArray());
                    }
                }
                else if (result.MessageType == WebSocketMessageType.Close)
                {
                    _isConnected = false;
                    if (TradeLogger.IsEnabled)
                        TradeLogger.Log("UserDataStream", "Connection closed by server, reconnecting...");
                    await ReconnectAsync();
                    break;
                }
            }
            catch (WebSocketException ex)
            {
                _isConnected = false;
                if (TradeLogger.IsEnabled)
                    TradeLogger.LogError("UserDataStream", "WebSocket error: " + ex.Message);
                await ReconnectAsync();
                break;
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex)
            {
                if (TradeLogger.IsEnabled)
                    TradeLogger.LogError("UserDataStream", "Receive error: " + ex.Message);
            }
        }
    }
    
    /// <summary>
    /// Reconnect after disconnect
    /// </summary>
    private async Task ReconnectAsync()
    {
        if (_cts?.IsCancellationRequested == true) return;
        
        int attempt = 0;
        while (!_cts!.IsCancellationRequested)
        {
            attempt++;
            var delay = Math.Min(30000, 1000 * (1 << attempt)); // Exponential backoff
            
            if (TradeLogger.IsEnabled)
                TradeLogger.Log("UserDataStream", "Reconnecting in " + delay + "ms (attempt " + attempt + ")...");
            await Task.Delay(delay, _cts.Token);
            
            try
            {
                // Get new ListenKey via Manager
                _listenKey = await _listenKeyManager.StartAsync(_apiKey);
                
                if (string.IsNullOrEmpty(_listenKey))
                {
                    continue;
                }
                
                // Dispose old WebSocket
                _ws?.Dispose();
                
                // Connect new WebSocket
                await ConnectWebSocketAsync();
                
                if (TradeLogger.IsEnabled)
                    TradeLogger.Log("UserDataStream", "✅ Reconnected successfully");
                return;
            }
            catch (Exception ex)
            {
                if (TradeLogger.IsEnabled)
                    TradeLogger.LogError("UserDataStream", "Reconnect failed: " + ex.Message);
            }
        }
    }
    
    public async Task StopAsync()
    {
        _cts?.Cancel();
        
        if (_ws?.State == WebSocketState.Open)
        {
            await _ws.CloseAsync(WebSocketCloseStatus.NormalClosure, "Stopping", CancellationToken.None);
        }
        
        await _listenKeyManager.StopAsync();
        
        _isConnected = false;
        if (TradeLogger.IsEnabled)
            TradeLogger.Log("UserDataStream", "Stopped");
    }
    
    public void Dispose()
    {
        _cts?.Cancel();
        _ws?.Dispose();
        _cts?.Dispose();
        _listenKeyManager?.Dispose();
    }
}

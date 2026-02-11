using System.Text.Json;
using SpreadAggregator.Application.Diagnostics;
using Microsoft.Extensions.Logging;

namespace SpreadAggregator.Infrastructure.Services.Exchanges;

/// <summary>
/// Manages Binance Futures User Data Stream ListenKey via REST API.
/// Handles creation, keep-alive (PUT), and lifecycle management.
/// </summary>
public class BinanceListenKeyManager : IDisposable
{
    private const string REST_BASE_URL = "https://fapi.binance.com";
    private const int KEEPALIVE_INTERVAL_MS = 30 * 60 * 1000; // 30 minutes
    
    private readonly HttpClient _httpClient;
    private readonly ILogger<BinanceListenKeyManager>? _logger;
    private string _apiKey = "";
    private string _currentListenKey = "";
    private CancellationTokenSource? _keepAliveCts;
    private Task? _keepAliveTask;
    
    // Event when ListenKey is refreshed or changes
    public event Action<string>? OnListenKeyChanged;

    public string CurrentListenKey => _currentListenKey;

    public BinanceListenKeyManager(HttpClient httpClient, ILogger<BinanceListenKeyManager>? logger = null)
    {
        _httpClient = httpClient;
        _logger = logger;
    }

    /// <summary>
    /// Initialize with API Key and get first ListenKey
    /// </summary>
    public async Task<string> StartAsync(string apiKey)
    {
        _apiKey = apiKey;
        _currentListenKey = await CreateListenKeyAsync();
        
        if (!string.IsNullOrEmpty(_currentListenKey))
        {
            StartAutoRenew();
        }
        
        return _currentListenKey;
    }

    public async Task StopAsync()
    {
        _keepAliveCts?.Cancel();
        if (_keepAliveTask != null)
        {
            try 
            {
                await _keepAliveTask;
            }
            catch (OperationCanceledException) { }
        }
        _currentListenKey = "";
    }

    private void StartAutoRenew()
    {
        _keepAliveCts?.Cancel();
        _keepAliveCts = new CancellationTokenSource();
        _keepAliveTask = KeepAliveLoopAsync(_keepAliveCts.Token);
    }

    private async Task<string> CreateListenKeyAsync()
    {
        try
        {
            var request = new HttpRequestMessage(HttpMethod.Post, $"{REST_BASE_URL}/fapi/v1/listenKey");
            request.Headers.Add("X-MBX-APIKEY", _apiKey);
            
            var response = await _httpClient.SendAsync(request);
            var content = await response.Content.ReadAsStringAsync();
            
            if (response.IsSuccessStatusCode)
            {
                using var doc = JsonDocument.Parse(content);
                var key = doc.RootElement.GetProperty("listenKey").GetString() ?? "";
                Log($"Got listenKey: ...{ShowLast(key, 8)}");
                return key;
            }
            else
            {
                LogError($"CreateListenKey failed: {content}");
                return "";
            }
        }
        catch (Exception ex)
        {
            LogError($"CreateListenKey exception: {ex.Message}");
            return "";
        }
    }

    private async Task KeepAliveListenKeyAsync()
    {
        if (string.IsNullOrEmpty(_currentListenKey)) return;

        try
        {
            var request = new HttpRequestMessage(HttpMethod.Put, $"{REST_BASE_URL}/fapi/v1/listenKey");
            request.Headers.Add("X-MBX-APIKEY", _apiKey);
            
            // Binance usually doesn't require ListenKey in body for PUT on fapi/v1/listenKey if using API Key header,
            // but docs say "Keepalive a user data stream to prevent a time out."
            // It implicitly refreshes the stream associated with the API Key or might need parameter?
            // Checking Binance Futures API docs: 
            // PUT /fapi/v1/listenKey
            // Parameters: NONE (it refreshes the key associated with the API Key? No, that can't be right for multiple streams)
            // Wait, standard Binance Spot uses "listenKey" parameter. 
            // Futures docs say: "Keepalive a user data stream to prevent a time out. User data stream will close after 60 minutes. It's recommended to send a ping about every 30 minutes."
            // It creates a listenKey. The same endpoint without params creates it.
            // Actually, for PUT, it often requires no params if it's just refreshing the *session* context, 
            // BUT standard practice is often passing it.
            // Let's check the original implementation.
            // Original: new HttpRequestMessage(HttpMethod.Put, $"{REST_BASE_URL}/fapi/v1/listenKey");
            // Original added API Key header. No body.
            // So we stick to that.

            var response = await _httpClient.SendAsync(request);
            
            if (response.IsSuccessStatusCode)
            {
                Log("ListenKey refreshed");
            }
            else
            {
                var content = await response.Content.ReadAsStringAsync();
                LogError($"KeepAlive failed: {content}");
                
                // If 401 or similar, maybe key expired?
                // Try to create new one
                Log("Attempting to re-create ListenKey...");
                var newKey = await CreateListenKeyAsync();
                if (!string.IsNullOrEmpty(newKey))
                {
                    _currentListenKey = newKey;
                    OnListenKeyChanged?.Invoke(newKey);
                }
            }
        }
        catch (Exception ex)
        {
            LogError($"KeepAlive exception: {ex.Message}");
        }
    }

    private async Task KeepAliveLoopAsync(CancellationToken ct)
    {
        Log("Starting KeepAlive loop");
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await Task.Delay(KEEPALIVE_INTERVAL_MS, ct);
                await KeepAliveListenKeyAsync();
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex)
            {
                 LogError($"KeepAlive loop error: {ex.Message}");
            }
        }
        Log("KeepAlive loop stopped");
    }

    private void Log(string message)
    {
        if (_logger != null) _logger.LogInformation($"[BinanceListenKeyManager] {message}");
        else TradeLogger.Log("UserDataStream", message);
    }

    private void LogError(string message)
    {
        if (_logger != null) _logger.LogError($"[BinanceListenKeyManager] {message}");
        else TradeLogger.LogError("UserDataStream", message);
    }
    
    private static string ShowLast(string s, int count)
    {
        if (string.IsNullOrEmpty(s)) return "";
        if (s.Length <= count) return s;
        return s[^count..];
    }

    public void Dispose()
    {
        _keepAliveCts?.Cancel();
        _keepAliveCts?.Dispose();
    }
}

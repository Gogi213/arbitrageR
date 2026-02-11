using System.Threading.Tasks;

namespace SpreadAggregator.Application.Abstractions;

/// <summary>
/// Defines the contract for a WebSocket server.
/// NOTE: This abstraction is necessary for proper layered architecture (Application -> Infrastructure boundary).
/// </summary>
public interface IWebSocketServer
{
    /// <summary>
    /// Starts the WebSocket server.
    /// </summary>
    void Start();

    /// <summary>
    /// Event triggered when a new client connects.
    /// </summary>
    event Action<Guid> OnClientConnected;

    /// <summary>
    /// Sends a message to a specific client.
    /// </summary>
    Task SendAsync(Guid clientId, string message);

    /// <summary>
    /// Sends a binary message to a specific client.
    /// </summary>
    Task SendAsync(Guid clientId, byte[] message);

    /// <summary>
    /// Broadcasts a message to all connected real-time clients.
    /// </summary>
    /// <param name="message">The message to send.</param>
    Task BroadcastRealtimeAsync(string message);

    /// <summary>
    /// Broadcasts a binary message to all connected real-time clients.
    /// Used for high-performance MessagePack updates.
    /// </summary>
    /// <param name="message">The binary message to send.</param>
    Task BroadcastRealtimeAsync(byte[] message);
    
    /// <summary>
    /// Gets the number of currently connected clients.
    /// </summary>
    int ClientCount { get; }
}

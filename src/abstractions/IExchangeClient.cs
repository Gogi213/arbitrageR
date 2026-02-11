using SpreadAggregator.Domain.Entities;
using System;
using System.Collections.Generic;
using System.Threading.Tasks;

namespace SpreadAggregator.Application.Abstractions;

/// <summary>
/// Defines the contract for an exchange client.
/// </summary>
public interface IExchangeClient
{
    /// <summary>
    /// Gets the name of the exchange.
    /// </summary>
    string ExchangeName { get; }

    /// <summary>
    /// Gets detailed information for all available symbols on the exchange.
    /// </summary>
    ValueTask<IEnumerable<SymbolInfo>> GetSymbolsAsync();

    /// <summary>
    /// Gets symbol info for a specific symbol from cache or fetches from exchange.
    /// Uses ValueTask for zero-allocation cache hits.
    /// </summary>
    /// <param name="symbol">The symbol to lookup (normalized or exchange format).</param>
    /// <returns>SymbolInfo if found, null otherwise.</returns>
    ValueTask<SymbolInfo?> GetSymbolInfoAsync(string symbol);

    /// <summary>
    /// Gets tickers for all symbols.
    /// </summary>
    /// <returns>A list of tickers.</returns>
    ValueTask<IEnumerable<TickerData>> GetTickersAsync();

    /// <summary>
    /// Gets book ticker data (BestBid/BestAsk) for all symbols.
    /// Used for deviation analysis and mid-price calculations.
    /// </summary>
    /// <returns>A list of book tickers with BestBid/BestAsk.</returns>
    ValueTask<IEnumerable<TickerData>> GetBookTickersAsync();

    /// <summary>
    /// Subscribes to trade updates for a list of symbols.
    /// </summary>
    /// <param name="symbols">The symbols to subscribe to.</param>
    /// <param name="onData">The action to perform when new trade data arrives.</param>
    ValueTask SubscribeToTradesAsync(IEnumerable<string> symbols, Func<TradeData, Task> onData);

    /// <summary>
    /// Subscribes to real-time book ticker updates (BestBid/BestAsk) for specified symbols.
    /// Replaces REST polling with WebSocket streaming for sub-100ms latency.
    /// </summary>
    /// <param name="symbols">The symbols to subscribe to (normalized format with underscores).</param>
    /// <param name="onData">The action to perform when book ticker data arrives.</param>
    ValueTask SubscribeToBookTickersAsync(IEnumerable<string> symbols, Func<TickerData, Task> onData);

    /// <summary>
    /// Stops all active subscriptions and closes connections.
    /// </summary>
    ValueTask StopAsync();
}

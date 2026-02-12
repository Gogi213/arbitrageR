using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Configuration;
using SpreadAggregator.Application.Services;
using SpreadAggregator.Application.Services.Audit;
using SpreadAggregator.Application.Abstractions;
using SpreadAggregator.Presentation.HotPath;
using SpreadAggregator.Presentation.Logic;
using SpreadAggregator.Presentation.Persistence;
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Globalization;
using System.IO;

namespace SpreadAggregator.Presentation;

/// <summary>
/// ThresholdTrackerService - Main Coordinator.
/// REFACTORED (Sprint 23): God Object -> Coordinator Pattern.
/// Delegates responsibilities to:
/// - HotPath (Zero Alloc State)
/// - Logic (Business Rules)
/// - Persistence (Cold Path I/O)
/// </summary>
public class ThresholdTrackerService : BackgroundService, IPulseProvider
{
    private readonly RatioService _ratioService;
    private readonly ILogger<ThresholdTrackerService> _logger;
    private readonly DashboardAuditLogger _auditLogger;
    private readonly string _logsBasePath;

    // Components
    private readonly ThresholdLogic _logic;
    private readonly ThresholdPersistence _persistence;

    // State Containers (Thread-Safe)
    // Symbol -> PairIdx -> HotPath
    private readonly ConcurrentDictionary<string, ThresholdHotPath[]> _hotPaths = new();
    
    // HOTFIX: Symbol -> ZeroCrossingTracker for BinByb (pairIdx=0)
    private readonly ConcurrentDictionary<string, ZeroCrossingTracker> _zeroTrackers = new();
    
    // Symbol -> PairIdx -> History (Warm Path storage)
    private readonly ConcurrentDictionary<string, ConcurrentDictionary<int, List<(DateTime timestamp, double val)>>> _touchHistory = new();
    private readonly ConcurrentDictionary<string, ConcurrentDictionary<int, List<(DateTime timestamp, double val)>>> _spreadHistory = new();
    
    // Symbol -> Total hits counter (Atomic)
    private readonly ConcurrentDictionary<string, long> _symbolTotalHits = new();
    
    // Symbol -> List of historical snapshots for trend calculation (timestamp, totalHits)
    private readonly ConcurrentDictionary<string, List<(DateTime timestamp, long totalHits)>> _activityHistory = new();
    
    // Symbol -> HalfLifeAnalyzer for BinByb spread (pairIdx=0)
    private readonly ConcurrentDictionary<string, HalfLifeAnalyzer> _halfLifeAnalyzers = new();
    
    // Configuration
    private const double THRESHOLD_PCT = 0.0020; // Kept at 0.20% for legacy pairs
    private const double RESET_PCT = 0.0005;
    private const int UPDATE_INTERVAL_MS = 2000;
    
    // HOTFIX: Inactive symbol filter threshold
    private const double INACTIVE_FILTER_THRESHOLD_PCT = 0.001; // 0.1%
    
    // HOTFIX: History retention (5 minutes)
    private const int HISTORY_RETENTION_MINUTES = 5;
    
    // Feature Flag
    private readonly bool _useNewHotPath;

    public ThresholdTrackerService(
        RatioService ratioService,
        ILogger<ThresholdTrackerService> logger,
        ILogger<ThresholdPersistence> persistenceLogger,
        ILogger<DashboardAuditLogger> auditLogger,
        IConfiguration configuration)
    {
        _ratioService = ratioService ?? throw new ArgumentNullException(nameof(ratioService));
        _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        _auditLogger = new DashboardAuditLogger(auditLogger);

        _logsBasePath = "logs"; // Default
        _useNewHotPath = configuration.GetValue("ThresholdTracker:UseNewHotPath", true);

        // Initialize Components
        _logic = new ThresholdLogic();
        _persistence = new ThresholdPersistence(
            persistenceLogger,
            Path.Combine(_logsBasePath, "threshold_state.json"));

        Directory.CreateDirectory(_logsBasePath);
    }

    public override Task StartAsync(CancellationToken cancellationToken)
    {
        _logger.LogInformation("[ThresholdTracker] Starting... NewHotPath={UseNew}", _useNewHotPath);
        _ratioService.OnRatioUpdated += HandleRatioUpdate;
        return base.StartAsync(cancellationToken);
    }

    public override Task StopAsync(CancellationToken cancellationToken)
    {
        _ratioService.OnRatioUpdated -= HandleRatioUpdate;
        return base.StopAsync(cancellationToken);
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        while (!stoppingToken.IsCancellationRequested)
        {
            try
            {
                await Task.Delay(UPDATE_INTERVAL_MS, stoppingToken);
                if (_useNewHotPath)
                {
                    // Dashboard updates moved to separate method or component if needed
                    // For now, minimal loop
                }
            }
            catch (OperationCanceledException) { break; }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error in tracker loop");
            }
        }
    }

    private void HandleRatioUpdate(RatioService.RatioSnapshot snapshot)
    {
        if (!_useNewHotPath) 
        {
            // KILL SWITCH ACTIVE: UseNewHotPath=false disables the service
            // Legacy implementation has been removed.
            return;
        }

        try
        {
            var symbol = snapshot.Symbol;
            var hotPaths = _hotPaths.GetOrAdd(symbol, _ => 
            {
                var arr = new ThresholdHotPath[6];
                for(int i=0; i<6; i++) arr[i] = new ThresholdHotPath(THRESHOLD_PCT, RESET_PCT);
                return arr;
            });

            for (int pairIdx = 0; pairIdx < 6; pairIdx++)
            {
                // 1. Hot Path Execution (Zero Alloc)
                var (hl, signal) = _logic.GetSignalData(snapshot.Point, pairIdx);
                
                // HOTFIX: Use ZeroCrossingTracker for pairIdx=0 (BinByb)
                if (pairIdx == 0)
                {
                    var tracker = _zeroTrackers.GetOrAdd(symbol, _ => new ZeroCrossingTracker());
                    tracker.ProcessSignal(signal);
                }
                else
                {
                    // Legacy ThresholdHotPath for other pairs
                    if (hotPaths[pairIdx].CheckThreshold(signal, out var direction))
                    {
                        RecordEvent(symbol, pairIdx, signal);
                    }
                }
                
                // Track spread for logic (Warm Path)
                double rawSpread = _logic.GetRawSpread(snapshot.Point, pairIdx);
                RecordSpread(symbol, pairIdx, rawSpread);
            }
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error processing tick for {Symbol}", snapshot.Symbol);
        }
    }

    private void RecordEvent(string symbol, int pairIdx, double signal)
    {
        var now = DateTime.UtcNow;
        var histories = _touchHistory.GetOrAdd(symbol, _ => new ConcurrentDictionary<int, List<(DateTime, double)>>());
        var history = histories.GetOrAdd(pairIdx, _ => new List<(DateTime, double)>());
        
        lock (history)
        {
            history.Add((now, signal));
            
            // HOTFIX: Cleanup old entries (prevent memory leak)
            while (history.Count > 0 && (now - history[0].Item1).TotalMinutes > HISTORY_RETENTION_MINUTES)
            {
                history.RemoveAt(0);
            }
        }
        
        // Atomic increment of total hits (O(1))
        long currentTotal = _symbolTotalHits.AddOrUpdate(symbol, 1, (_, count) => count + 1);
        
        // Update activity history for trend calculation
        var actHistory = _activityHistory.GetOrAdd(symbol, _ => new List<(DateTime, long)>());
        lock (actHistory)
        {
            // O(1) append instead of O(N) aggregation
            actHistory.Add((now, currentTotal));
            
            // Cleanup
            if (actHistory.Count > 0 && (now - actHistory[0].timestamp).TotalMinutes > 5)
            {
                actHistory.RemoveAt(0);
            }
        }
    }

    private void RecordSpread(string symbol, int pairIdx, double spread)
    {
        var now = DateTime.UtcNow;
        var histories = _spreadHistory.GetOrAdd(symbol, _ => new ConcurrentDictionary<int, List<(DateTime, double)>>());
        var history = histories.GetOrAdd(pairIdx, _ => new List<(DateTime, double)>());
        
        lock (history)
        {
            history.Add((now, spread));
            
            // HOTFIX: Cleanup old entries (prevent memory leak)
            while (history.Count > 0 && (now - history[0].Item1).TotalMinutes > HISTORY_RETENTION_MINUTES)
            {
                history.RemoveAt(0);
            }
        }
        
        // Feed HalfLifeAnalyzer for BinByb (pairIdx=0)
        if (pairIdx == 0)
        {
            var analyzer = _halfLifeAnalyzers.GetOrAdd(symbol, _ => new HalfLifeAnalyzer());
            analyzer.AddSample(spread, ((DateTimeOffset)now).ToUnixTimeMilliseconds());
        }
    }

    public (bool IsReady, double AvgPct) GetPulse(string symbol)
    {
        if (!_touchHistory.TryGetValue(symbol, out var pairHistories)) return (false, 0);
        
        var now = DateTime.UtcNow;
        
        // Use atomic counter for Heat (O(1))
        _symbolTotalHits.TryGetValue(symbol, out long totalHits);
        
        // Get activity history for "Trend"
        var actHistory = _activityHistory.GetOrAdd(symbol, _ => new List<(DateTime, long)>());
        List<(DateTime, long)> historySnapshot;
        
        lock(actHistory)
        {
            historySnapshot = actHistory.ToList();
        }
        
        // Calculate Pulse using Logic
        var (heat, trend, desc) = _logic.CalculatePulse(totalHits, null, historySnapshot, now);
        
        // Calculate average % (still iterates pairs, can be optimized later)
        double totalPct = 0;
        int count = 0;
        
        foreach (var history in pairHistories.Values)
        {
            lock(history)
            {
                 foreach(var (ts, val) in history)
                 {
                     if ((now - ts).TotalMinutes <= 5)
                     {
                         totalPct += Math.Abs(val);
                         count++;
                     }
                 }
            }
        }

        if (count < 3) return (false, 0); // Min samples
        
        return (true, totalPct / count);
    }

    public class SymbolStats
    {
        public string Symbol { get; set; } = "";
        public double CurrentSpread { get; set; }
        public double SpreadRange { get; set; }
        public bool IsSpreadNA { get; set; }
        public int Hits { get; set; }
        public double EstHalfLife { get; set; }
    }
    
    public List<SymbolStats> GetStats()
    {
        var stats = new List<SymbolStats>();
        var now = DateTime.UtcNow;
        
        // Return all symbols that have HalfLife data
        foreach (var kvp in _halfLifeAnalyzers)
        {
            var symbol = kvp.Key;
            var analyzer = kvp.Value;
            
            var rawHalfLife = analyzer.Analyze().HalfLifeSec;
            var estHalfLife = Math.Min(rawHalfLife, 600.0);
            if (double.IsInfinity(estHalfLife) || double.IsNaN(estHalfLife)) estHalfLife = 600.0;
            
            // PHASE 36.1: Get current spread from BinByb (pairIdx=0) - single source
            double currentSpread = 0;
            double spreadRange = 0;
            bool isSpreadNA = true;
            bool hasSignificantSpread = false; // HOTFIX: Filter inactive symbols
            
            if (_spreadHistory.TryGetValue(symbol, out var spreadHistories))
            {
                if (spreadHistories.TryGetValue(0, out var binBybHistory))
                {
                    lock (binBybHistory)
                    {
                        // PHASE 36.1: Current spread - last value
                        if (binBybHistory.Count > 0)
                        {
                            currentSpread = binBybHistory[binBybHistory.Count - 1].val;
                        }
                        
                        // PHASE 36.2: Calculate range over 2 minutes (HOTFIX: reuse existing method)
                        var (range, na) = _logic.CalculateSpreadRange(binBybHistory, now);
                        spreadRange = range;
                        isSpreadNA = na;
                        
                        // HOTFIX: Check if any spread in last 2 minutes exceeds threshold
                        // Optimized: iterate from end (most recent) backwards
                        var cutoff = now.AddMinutes(-2);
                        for (int i = binBybHistory.Count - 1; i >= 0; i--)
                        {
                            var (ts, val) = binBybHistory[i];
                            if (ts < cutoff) break; // Time-sorted, stop early
                            if (Math.Abs(val) >= INACTIVE_FILTER_THRESHOLD_PCT)
                            {
                                hasSignificantSpread = true;
                                break;
                            }
                        }
                    }
                }
            }
            
            // HOTFIX: Skip symbols without significant spread activity
            if (!hasSignificantSpread)
                continue;
            
            // HOTFIX: Get hit count from ZeroCrossingTracker (zero-crossing hysteresis)
            int hits = 0;
            if (_zeroTrackers.TryGetValue(symbol, out var tracker))
            {
                hits = (int)tracker.HitCount;
            }
            
            stats.Add(new SymbolStats
            {
                Symbol = symbol,
                CurrentSpread = currentSpread,
                SpreadRange = spreadRange,
                IsSpreadNA = isSpreadNA,
                Hits = hits,
                EstHalfLife = estHalfLife
            });
        }

        // Sort: Primary by Hits DESC, Secondary by SpreadRange DESC
        stats.Sort((a, b) =>
        {
            int hitsCmp = b.Hits.CompareTo(a.Hits); // DESC
            if (hitsCmp != 0) return hitsCmp;
            return b.SpreadRange.CompareTo(a.SpreadRange); // DESC
        });
        return stats;
    }

    private void LogAuditData(List<SymbolStats> stats, DateTime timestamp)
    {
        // Simplified - no audit logging needed for now
    }
}

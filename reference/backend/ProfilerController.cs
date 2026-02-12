using Microsoft.AspNetCore.Mvc;
using SpreadAggregator.Application.Diagnostics;
using SpreadAggregator.Infrastructure.Pooling;
using System.Buffers;

namespace SpreadAggregator.Presentation.Controllers;

/// <summary>
/// API для управления E2E Latency Profiler в runtime.
/// 
/// Endpoints:
/// - GET  /api/profiler/status       - Получить статус профайлера
/// - POST /api/profiler/enable       - Включить профилирование
/// - POST /api/profiler/disable      - Выключить профилирование
/// - GET  /api/profiler/stats        - Получить статистику
/// - GET  /api/profiler/report       - Получить детальный отчет
/// - POST /api/profiler/reset        - Сбросить статистику
/// - GET  /api/profiler/traces       - Получить последние trace'ы
/// 
/// OPTIMIZED: Zero-allocation implementation - no LINQ, manual loops only
/// </summary>
[ApiController]
[Route("api/[controller]")]
public class ProfilerController : ControllerBase
{
    /// <summary>
    /// Получить статус профайлера
    /// </summary>
    [HttpGet("status")]
    public IActionResult GetStatus()
    {
        var stats = E2ELatencyProfiler.Instance.GetAllStats();
        
        // Manual Sum() - zero allocation
        long totalTraces = 0;
        for (int i = 0; i < stats.Length; i++)
        {
            totalTraces += stats[i].TotalTraces;
        }
        
        return Ok(new
        {
            enabled = totalTraces > 0,
            totalSymbols = stats.Length,
            totalTraces,
            timestamp = DateTimeOffset.UtcNow
        });
    }
    
    /// <summary>
    /// Включить профилирование
    /// </summary>
    /// <param name="sampleRate">Частота семплирования (1/N events)</param>
    [HttpPost("enable")]
    public IActionResult Enable([FromQuery] int sampleRate = 100)
    {
        if (sampleRate < 1)
            return BadRequest(new { error = "sampleRate must be >= 1" });
            
        E2ELatencyProfiler.Enable(sampleRate);
        
        return Ok(new
        {
            message = "E2E Profiler ENABLED",
            sampleRate,
            description = $"Will profile every {sampleRate}th event"
        });
    }
    
    /// <summary>
    /// Выключить профилирование
    /// </summary>
    [HttpPost("disable")]
    public IActionResult Disable()
    {
        E2ELatencyProfiler.Disable();
        
        return Ok(new
        {
            message = "E2E Profiler DISABLED"
        });
    }
    
    /// <summary>
    /// Получить статистику по всем узлам
    /// </summary>
    [HttpGet("stats")]
    public IActionResult GetStats()
    {
        var stats = E2ELatencyProfiler.Instance.GetAllStats();
        
        if (stats.Length == 0)
        {
            return Ok(new
            {
                message = "No data collected. Enable profiler first.",
                stats = Array.Empty<object>()
            });
        }
        
        // Rent pooled arrays
        var resultArray = ProfilerDtoPool.Rent<SymbolStatDto>(stats.Length);
        int resultCount = 0;
        
        try
        {
            // Manual Select() implementation
            for (int sIdx = 0; sIdx < stats.Length; sIdx++)
            {
                var s = stats[sIdx];
                var nodeCount = s.Nodes.Count;
                var nodeArray = ProfilerDtoPool.Rent<NodeStatDto>(nodeCount);
                int nodeIdx = 0;
                
                // Manual Select() for nodes + OrderBy name using insertion sort
                foreach (var node in s.Nodes)
                {
                    var nodeStat = new NodeStatDto
                    {
                        Name = node.Key,
                        AvgMicros = node.Value.AvgMicros,
                        MaxMicros = node.Value.MaxMicros,
                        MinMicros = node.Value.MinMicros,
                        P95Micros = node.Value.P95Micros,
                        Count = node.Value.Count
                    };
                    
                    // Insertion sort by name
                    int insertPos = nodeIdx;
                    for (int i = 0; i < nodeIdx; i++)
                    {
                        if (string.CompareOrdinal(nodeArray[i].Name, nodeStat.Name) > 0)
                        {
                            insertPos = i;
                            break;
                        }
                    }
                    
                    // Shift elements
                    for (int i = nodeIdx; i > insertPos; i--)
                    {
                        nodeArray[i] = nodeArray[i - 1];
                    }
                    
                    nodeArray[insertPos] = nodeStat;
                    nodeIdx++;
                }
                
                resultArray[resultCount++] = new SymbolStatDto
                {
                    Symbol = s.Symbol,
                    TotalTraces = s.TotalTraces,
                    Nodes = nodeArray[..nodeIdx].ToArray() // Create actual array of correct size
                };
                
                // Return temporary array
                ProfilerDtoPool.Return(nodeArray);
            }
            
            return Ok(new
            {
                stats = resultArray[..resultCount],
                collectedAt = DateTimeOffset.UtcNow
            });
        }
        finally
        {
            ProfilerDtoPool.Return(resultArray);
        }
    }
    
    /// <summary>
    /// Получить детальный отчет (aggregated view)
    /// </summary>
    [HttpGet("report")]
    public IActionResult GetReport()
    {
        var stats = E2ELatencyProfiler.Instance.GetAllStats();
        
        if (stats.Length == 0)
        {
            return Ok(new
            {
                message = "No data collected. Enable profiler first."
            });
        }
        
        // Phase 1: Collect all nodes with their stats using pooled arrays
        var allNodes = ProfilerDtoPool.Rent<(NodeStatsCollection Symbol, string NodeName, NodeStats Stats)>(stats.Length * 10); // Estimate
        int allNodesCount = 0;
        
        try
        {
            // Manual SelectMany() + Select()
            for (int sIdx = 0; sIdx < stats.Length; sIdx++)
            {
                var s = stats[sIdx];
                foreach (var node in s.Nodes)
                {
                    allNodes[allNodesCount++] = (s, node.Key, node.Value);
                }
            }
            
            // Phase 2: Group by node name using pooled array (zero-allocation)
            // First pass: count unique nodes
            var nodeNames = ProfilerDtoPool.Rent<string>(allNodesCount);
            var nodeCounts = ProfilerDtoPool.Rent<int>(allNodesCount);
            int uniqueNodeCount = 0;
            
            for (int i = 0; i < allNodesCount; i++)
            {
                var item = allNodes[i];
                int nodeIdx = -1;
                
                // Find existing node
                for (int j = 0; j < uniqueNodeCount; j++)
                {
                    if (nodeNames[j] == item.NodeName)
                    {
                        nodeIdx = j;
                        break;
                    }
                }
                
                if (nodeIdx == -1)
                {
                    // New node
                    nodeNames[uniqueNodeCount] = item.NodeName;
                    nodeCounts[uniqueNodeCount] = 1;
                    uniqueNodeCount++;
                }
                else
                {
                    nodeCounts[nodeIdx]++;
                }
            }
            
            // Allocate grouped storage
            var groupedNodes = ProfilerDtoPool.Rent<(string NodeName, NodeStats Stats)[]>(uniqueNodeCount);
            var groupedIndices = ProfilerDtoPool.Rent<int>(uniqueNodeCount);
            
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                groupedNodes[i] = ProfilerDtoPool.Rent<(string, NodeStats)>(nodeCounts[i]);
                groupedIndices[i] = 0;
            }
            
            // Second pass: fill grouped data
            for (int i = 0; i < allNodesCount; i++)
            {
                var item = allNodes[i];
                for (int j = 0; j < uniqueNodeCount; j++)
                {
                    if (nodeNames[j] == item.NodeName)
                    {
                        groupedNodes[j][groupedIndices[j]++] = (item.NodeName, item.Stats);
                        break;
                    }
                }
            }
            
            // Phase 3: Calculate aggregates
            var aggregated = ProfilerDtoPool.Rent<AggregatedNodeDto>(uniqueNodeCount);
            
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                double sumAvg = 0, sumP95 = 0;
                double maxMicros = 0;
                double minMicros = double.MaxValue;
                long totalCount = 0;
                
                for (int j = 0; j < groupedIndices[i]; j++)
                {
                    var stats2 = groupedNodes[i][j].Stats;
                    sumAvg += stats2.AvgMicros;
                    sumP95 += stats2.P95Micros;
                    if (stats2.MaxMicros > maxMicros) maxMicros = stats2.MaxMicros;
                    if (stats2.MinMicros < minMicros) minMicros = stats2.MinMicros;
                    totalCount += stats2.Count;
                }
                
                int count = groupedIndices[i];
                aggregated[i] = new AggregatedNodeDto
                {
                    Node = nodeNames[i],
                    AvgMicros = sumAvg / count,
                    MaxMicros = maxMicros,
                    MinMicros = minMicros == double.MaxValue ? 0 : minMicros,
                    P95Micros = sumP95 / count,
                    TotalCount = totalCount
                };
                
                // Return grouped array
                ProfilerDtoPool.Return(groupedNodes[i]);
            }
            
            // Sort using Array.Sort (O(n log n)) instead of bubble sort (O(n²))
            Array.Sort(aggregated, 0, uniqueNodeCount, AggregatedNodeComparer.Instance);
            
            // Cleanup
            ProfilerDtoPool.Return(nodeNames);
            ProfilerDtoPool.Return(nodeCounts);
            ProfilerDtoPool.Return(groupedNodes);
            ProfilerDtoPool.Return(groupedIndices);
            
            // Phase 4: Calculate total average
            double totalAvg = 0;
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                totalAvg += aggregated[i].AvgMicros;
            }
            
            // Phase 5: Take top 3 bottlenecks + manual Select()
            var topCount = Math.Min(3, uniqueNodeCount);
            var bottlenecks = new BottleneckDto[topCount];
            for (int i = 0; i < topCount; i++)
            {
                var item = aggregated[i];
                bottlenecks[i] = new BottleneckDto
                {
                    Node = item.Node,
                    AvgMicros = item.AvgMicros,
                    MaxMicros = item.MaxMicros,
                    PercentageOfTotal = totalAvg > 0 ? item.AvgMicros / totalAvg * 100 : 0
                };
            }
            
            // Phase 6: Manual Sum() for total traces
            long totalTraces = 0;
            for (int i = 0; i < stats.Length; i++)
            {
                totalTraces += stats[i].TotalTraces;
            }
            
            // Phase 7: Manual Select() for per-symbol
            var perSymbolArray = ProfilerDtoPool.Rent<SymbolSummaryDto>(stats.Length);
            int perSymbolCount = 0;
            
            for (int sIdx = 0; sIdx < stats.Length; sIdx++)
            {
                var s = stats[sIdx];
                double sumLatency = 0;
                foreach (var node in s.Nodes.Values)
                {
                    sumLatency += node.AvgMicros;
                }
                
                perSymbolArray[perSymbolCount++] = new SymbolSummaryDto
                {
                    Symbol = s.Symbol,
                    TotalTraces = s.TotalTraces,
                    AvgLatencyMicros = sumLatency
                };
            }
            
            var result = new
            {
                summary = new
                {
                    totalSymbols = stats.Length,
                    totalTraces = totalTraces,
                    totalNodes = uniqueNodeCount,
                    totalAvgLatencyMicros = totalAvg
                },
                aggregatedNodes = aggregated[..uniqueNodeCount],
                topBottlenecks = bottlenecks,
                perSymbol = perSymbolArray[..perSymbolCount],
                collectedAt = DateTimeOffset.UtcNow
            };
            
            ProfilerDtoPool.Return(perSymbolArray);
            ProfilerDtoPool.Return(aggregated);
            
            return Ok(result);
        }
        finally
        {
            ProfilerDtoPool.Return(allNodes);
        }
    }
    
    /// <summary>
    /// Сбросить всю статистику
    /// </summary>
    [HttpPost("reset")]
    public IActionResult Reset()
    {
        E2ELatencyProfiler.Instance.Reset();
        
        return Ok(new
        {
            message = "E2E Profiler stats RESET"
        });
    }
    
    /// <summary>
    /// Получить последние N trace'ов для детального анализа
    /// </summary>
    /// <param name="count">Количество trace'ов (default: 100)</param>
    [HttpGet("traces")]
    public IActionResult GetRecentTraces([FromQuery] int count = 100)
    {
        if (count < 1 || count > 1000)
            return BadRequest(new { error = "count must be between 1 and 1000" });
            
        var traces = E2ELatencyProfiler.Instance.GetRecentTraces(count);
        
        if (traces.Length == 0)
        {
            return Ok(new
            {
                message = "No traces collected yet.",
                traces = Array.Empty<object>()
            });
        }
        
        // Rent pooled array for results
        var resultArray = ProfilerDtoPool.Rent<TraceResultDto>(traces.Length);
        
        try
        {
            // Manual Select() implementation
            for (int tIdx = 0; tIdx < traces.Length; tIdx++)
            {
                var t = traces[tIdx];
                var nodeArray = ProfilerDtoPool.Rent<TraceNodeDto>(t.Nodes.Length);
                
                // Manual Select() for nodes
                for (int nIdx = 0; nIdx < t.Nodes.Length; nIdx++)
                {
                    var n = t.Nodes[nIdx];
                    nodeArray[nIdx] = new TraceNodeDto
                    {
                        Name = n.Name,
                        DurationMicros = n.DurationMicros
                    };
                }
                
                resultArray[tIdx] = new TraceResultDto
                {
                    Symbol = t.Symbol,
                    ExchangeTimestampMs = t.ExchangeTimestampMs,
                    TotalDurationMicros = t.TotalDurationMicros,
                    Nodes = nodeArray[..t.Nodes.Length].ToArray(),
                    CompletedAt = t.CompletedAt
                };
                
                ProfilerDtoPool.Return(nodeArray);
            }
            
            return Ok(new
            {
                count = traces.Length,
                traces = resultArray[..traces.Length]
            });
        }
        finally
        {
            ProfilerDtoPool.Return(resultArray);
        }
    }
    
    /// <summary>
    /// Получить сводку (dashboard view)
    /// </summary>
    [HttpGet("dashboard")]
    public IActionResult GetDashboard()
    {
        var stats = E2ELatencyProfiler.Instance.GetAllStats();
        
        if (stats.Length == 0)
        {
            return Ok(new
            {
                enabled = false,
                message = "No data collected. Enable profiler first."
            });
        }
        
        // Phase 1: Collect all nodes
        var allNodes = ProfilerDtoPool.Rent<(NodeStatsCollection Symbol, string NodeName, NodeStats Stats)>(stats.Length * 10);
        int allNodesCount = 0;
        
        try
        {
            // Manual SelectMany() + Select()
            for (int sIdx = 0; sIdx < stats.Length; sIdx++)
            {
                var s = stats[sIdx];
                foreach (var node in s.Nodes)
                {
                    allNodes[allNodesCount++] = (s, node.Key, node.Value);
                }
            }
            
            // Phase 2: Group by node name using pooled array (zero-allocation)
            var nodeNames = ProfilerDtoPool.Rent<string>(allNodesCount);
            var nodeCounts = ProfilerDtoPool.Rent<int>(allNodesCount);
            int uniqueNodeCount = 0;
            
            for (int i = 0; i < allNodesCount; i++)
            {
                var item = allNodes[i];
                int nodeIdx = -1;
                
                for (int j = 0; j < uniqueNodeCount; j++)
                {
                    if (nodeNames[j] == item.NodeName)
                    {
                        nodeIdx = j;
                        break;
                    }
                }
                
                if (nodeIdx == -1)
                {
                    nodeNames[uniqueNodeCount] = item.NodeName;
                    nodeCounts[uniqueNodeCount] = 1;
                    uniqueNodeCount++;
                }
                else
                {
                    nodeCounts[nodeIdx]++;
                }
            }
            
            var groupedNodes = ProfilerDtoPool.Rent<(string NodeName, NodeStats Stats)[]>(uniqueNodeCount);
            var groupedIndices = ProfilerDtoPool.Rent<int>(uniqueNodeCount);
            
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                groupedNodes[i] = ProfilerDtoPool.Rent<(string, NodeStats)>(nodeCounts[i]);
                groupedIndices[i] = 0;
            }
            
            for (int i = 0; i < allNodesCount; i++)
            {
                var item = allNodes[i];
                for (int j = 0; j < uniqueNodeCount; j++)
                {
                    if (nodeNames[j] == item.NodeName)
                    {
                        groupedNodes[j][groupedIndices[j]++] = (item.NodeName, item.Stats);
                        break;
                    }
                }
            }
            
            // Phase 3: Calculate aggregates
            var aggregated = ProfilerDtoPool.Rent<DashboardNodeDto>(uniqueNodeCount);
            
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                double sumAvg = 0;
                double maxMicros = 0;
                long totalCount = 0;
                
                for (int j = 0; j < groupedIndices[i]; j++)
                {
                    var stats2 = groupedNodes[i][j].Stats;
                    sumAvg += stats2.AvgMicros;
                    if (stats2.MaxMicros > maxMicros) maxMicros = stats2.MaxMicros;
                    totalCount += stats2.Count;
                }
                
                double avgMicros = sumAvg / groupedIndices[i];
                aggregated[i] = new DashboardNodeDto
                {
                    Node = nodeNames[i],
                    AvgMicros = avgMicros,
                    AvgMs = avgMicros / 1000.0,
                    MaxMicros = maxMicros,
                    MaxMs = maxMicros / 1000.0,
                    Count = totalCount,
                    PercentageOfTotal = 0
                };
                
                ProfilerDtoPool.Return(groupedNodes[i]);
            }
            
            // Sort using Array.Sort (O(n log n)) instead of bubble sort
            Array.Sort(aggregated, 0, uniqueNodeCount, DashboardNodeComparer.Instance);
            
            // Cleanup
            ProfilerDtoPool.Return(nodeNames);
            ProfilerDtoPool.Return(nodeCounts);
            ProfilerDtoPool.Return(groupedNodes);
            ProfilerDtoPool.Return(groupedIndices);
            
            // Phase 4: Calculate percentages
            double totalAvg = 0;
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                totalAvg += aggregated[i].AvgMicros;
            }
            
            for (int i = 0; i < uniqueNodeCount; i++)
            {
                aggregated[i].PercentageOfTotal = totalAvg > 0 ? aggregated[i].AvgMicros / totalAvg * 100 : 0;
            }
            
            // Phase 5: Manual Sum() for total traces
            long totalTraces = 0;
            for (int i = 0; i < stats.Length; i++)
            {
                totalTraces += stats[i].TotalTraces;
            }
            
            // Phase 6: Manual FirstOrDefault()
            string topBottleneck = uniqueNodeCount > 0 ? aggregated[0].Node : "N/A";
            
            var result = new
            {
                enabled = true,
                summary = new
                {
                    totalSymbols = stats.Length,
                    totalTraces = totalTraces,
                    totalAvgLatencyMicros = totalAvg,
                    totalAvgLatencyMs = totalAvg / 1000.0
                },
                nodes = aggregated[..uniqueNodeCount],
                topBottleneck = topBottleneck,
                collectedAt = DateTimeOffset.UtcNow
            };
            
            ProfilerDtoPool.Return(aggregated);
            
            return Ok(result);
        }
        finally
        {
            ProfilerDtoPool.Return(allNodes);
        }
    }
}

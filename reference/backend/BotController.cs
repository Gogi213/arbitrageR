using Microsoft.AspNetCore.Mvc;
using SpreadAggregator.Infrastructure.Services;
using System.Linq;

namespace SpreadAggregator.Presentation.Controllers;

[ApiController]
[Route("api/bot")]
public class BotController : ControllerBase
{
    private readonly TradingBotManager _botManager;

    public BotController(TradingBotManager botManager)
    {
        _botManager = botManager;
    }

    [HttpPost("{symbol}/start")]
    public IActionResult Start(string symbol)
    {
        var tracker = _botManager.Trackers.FirstOrDefault(t => t.Symbol == symbol);
        if (tracker == null) return NotFound(new { error = $"Bot {symbol} not found" });

        tracker.StartTrading();
        return Ok(new { symbol, status = "STARTED", message = "Trading enabled" });
    }

    [HttpPost("{symbol}/stop")]
    public IActionResult Stop(string symbol)
    {
        var tracker = _botManager.Trackers.FirstOrDefault(t => t.Symbol == symbol);
        if (tracker == null) return NotFound(new { error = $"Bot {symbol} not found" });

        tracker.StopTrading();
        return Ok(new { symbol, status = "STOPPED", message = "Trading disabled" });
    }
    
    [HttpDelete("{symbol}")]
    public async Task<IActionResult> Delete(string symbol)
    {
        try
        {
            await _botManager.RemoveBotAsync(symbol);
            return Ok(new { symbol, status = "DELETED", message = "Bot removed" });
        }
        catch (KeyNotFoundException)
        {
            return NotFound(new { error = $"Bot {symbol} not found" });
        }
        catch (Exception ex)
        {
            return BadRequest(new { error = ex.Message });
        }
    }
    
    // SPRINT 33 Phase 7: Dynamic Spawning
    [HttpPost("spawn")]
    public async Task<IActionResult> Spawn([FromBody] SpawnRequest req)
    {
        if (string.IsNullOrEmpty(req.Symbol)) return BadRequest(new { error = "Symbol required" });
        try
        {
            await _botManager.SpawnBotBySymbolAsync(req.Symbol);
            return Ok(new { symbol = req.Symbol, status = "SPAWNED" });
        }
        catch (Exception ex)
        {
            return BadRequest(new { error = ex.Message });
        }
    }

    public record SpawnRequest(string Symbol);
}

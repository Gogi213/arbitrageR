export default {
    props: ['bots'],
    template: `
        <div class="stats-table-container">
            <table class="cyber-table">
                <thead>
                    <tr>
                        <th class="text-left">BOT</th>
                        <th class="text-center">POS</th>
                        <th class="text-right">BALANCE</th>
                        <th class="text-right">PnL</th>
                        <th class="text-right">PnL %</th>
                        <th class="text-right">uPnL</th>
                        <th class="text-right">TRADES</th>
                        <th class="text-right">WIN%</th>
                        <th class="text-right">SHARPE</th>
                        <th class="text-right">uMPP</th>
                        <th class="text-right">uMPU</th>
                        <th class="text-right">VOL</th>
                        <th class="text-right">COMM</th>
                        <th class="text-right">HL (s)</th>
                    </tr>
                </thead>
                <tbody>
                    <tr v-for="bot in bots" :key="bot.tracker?.symbol">
                        <td class="text-left font-mono font-bold">{{ bot.tracker?.symbol }}</td>
                        
                        <!-- Position -->
                        <td class="text-center">
                            <span v-if="bot.pos && bot.pos.q !== 0" 
                                  class="badge" 
                                  :class="bot.pos.s === 'LONG' ? 'bg-green' : 'bg-red'">
                                {{ bot.pos.s }} {{ bot.pos.q }}
                            </span>
                            <span v-else class="text-dim">FLAT</span>
                        </td>
                        
                        <td class="text-right">{{ fmtMoney(bot.balance) }}</td>
                        
                        <!-- PnL -->
                        <td class="text-right" :class="color(bot.pnl)">{{ fmtMoney(bot.pnl) }}</td>
                        <td class="text-right" :class="color(bot.pnl)">{{ fmtPct(bot.startCap ? bot.pnl/bot.startCap : 0) }}</td>
                        
                        <!-- uPnL -->
                        <td class="text-right" :class="color(bot.uPnl)">{{ fmtMoney(bot.uPnl) }}</td>
                        
                        <td class="text-right">{{ bot.trades }}</td>
                        <td class="text-right" :class="bot.winRate >= 0.5 ? 'text-green' : 'text-red'">{{ fmtPct(bot.winRate) }}</td>
                        <td class="text-right text-blue">{{ (bot.sharpe || 0).toFixed(2) }}</td>
                        
                        <!-- Max Profit/Drawdown -->
                        <td class="text-right text-green">+{{ fmtMoney(bot.mpp) }}</td>
                        <td class="text-right text-red">{{ fmtMoney(bot.mpu) }}</td>
                        
                        <!-- Vol/Comm -->
                        <td class="text-right">{{ fmtMoney(bot.volume) }}</td>
                        <td class="text-right text-dim">{{ fmtMoney(bot.commission) }}</td>
                        
                        <!-- Half-Life -->
                        <td class="text-right" :class="hlColor(bot.tracker?.halfLifeSec)">
                            {{ Math.round(bot.tracker?.halfLifeSec || 0) }}s
                        </td>
                    </tr>
                </tbody>
            </table>
        </div>
    `,
    methods: {
        fmtMoney(v) { return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(v || 0); },
        fmtPct(v) { return new Intl.NumberFormat('en-US', { style: 'percent', minimumFractionDigits: 2 }).format(v || 0); },
        color(v) { return (v || 0) >= 0 ? 'text-green' : 'text-red'; },
        hlColor(v) { 
            if (!v) return 'text-dim';
            if (v > 300) return 'text-red';
            if (v > 120) return 'text-gold';
            return 'text-green';
        }
    }
};

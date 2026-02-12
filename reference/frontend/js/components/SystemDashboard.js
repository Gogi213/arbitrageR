export default {
    props: ['store'],
    template: `
        <div class="dashboard-grid" style="grid-template-columns: repeat(3, 1fr); grid-template-rows: auto 1fr; gap: 10px; height: 100%; overflow: hidden; padding-bottom: 10px; box-sizing: border-box;">
            <!-- Row 1: Compact Metrics -->
            <div class="dash-card" style="padding: 10px; margin: 0;">
                <div class="dash-label" style="margin-bottom:4px;">ACTIVE BOTS</div>
                <div class="dash-value text-primary" style="font-size:20px;">{{ store.activeBotCount }} <span style="font-size:12px; color:#666;">/ {{ store.bots.length }}</span></div>
            </div>
            
            <div class="dash-card" style="padding: 10px; margin: 0;">
                <div class="dash-label" style="margin-bottom:4px;">OPPORTUNITIES</div>
                <div class="dash-value text-gold" style="font-size:20px;">{{ store.screener.length }}</div>
            </div>
            
             <div class="dash-card" style="padding: 10px; margin: 0;">
                <div class="dash-label" style="margin-bottom:4px;">GLOBAL EXPOSURE</div>
                <div class="dash-value text-blue" style="font-size:20px;">{{ fmtMoney(globalExposure) }}</div>
            </div>

            <!-- Row 2: Screener Table (Full Height) -->
            <div class="dash-card wide-card" style="grid-column: 1 / -1; display:flex; flex-direction:column; overflow:hidden; height:94%; margin: 0;">
                <div class="dash-label" style="padding:4px 10px; border-bottom:1px solid rgba(255,255,255,0.05); min-height: 20px;">MARKET SCREENER (ACTIVE OPPORTUNITIES)</div>
                <div style="flex:1; overflow-y:auto;">
                    <table class="cyber-table">
                        <thead style="position:sticky; top:0; background:var(--bg-card); z-index:1;">
                            <tr>
                                <th class="text-left">SYMBOL</th>
                                <th class="text-right">SPREAD</th>
                                <th class="text-right">RANGE (2m)</th>
                                <th class="text-right">HITS</th>
                                <th class="text-right">AVG HL</th>
                                <th class="text-right">ACTION</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="s in store.screener" :key="s.symbol">
                                <td class="text-left font-mono font-bold">{{ s.symbol }}</td>
                                <td class="text-right" :class="spreadColor(s.currentSpread)">{{ fmtPct(s.currentSpread) }}</td>
                                <td class="text-right" :class="rangeColor(s)">{{ fmtRange(s) }}</td>
                                <td class="text-right">{{ s.hits }}</td>
                                <td class="text-right">{{ s.estHalfLife?.toFixed(2) || 'N/A' }}</td>
                                <td class="text-right">
                                    <button class="btn btn-primary" @click="spawn(s.symbol)">+ BOT</button>
                                </td>
                            </tr>
                            <tr v-if="store.screener.length === 0">
                                <td colspan="6" class="text-center text-dim" style="padding:40px;">SCANNING MARKETS... NO SIGNALS YET</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    `,
    computed: {
        globalExposure() {
            return this.store.bots.reduce((sum, b) => sum + Math.abs((b.pos?.q || 0) * (b.tracker?.entryPrice || 0)), 0);
        },
        longExposure() {
            return this.store.bots.filter(b => b.pos?.s === 'LONG').reduce((sum, b) => sum + (b.pos?.q || 0) * (b.tracker?.entryPrice || 0), 0);
        },
        shortExposure() {
             return this.store.bots.filter(b => b.pos?.s === 'SELL').reduce((sum, b) => sum + (b.pos?.q || 0) * (b.tracker?.entryPrice || 0), 0);
        }
    },
    methods: {
        fmtMoney(v) { return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(v || 0); },
        fmtPct(v) { 
            return new Intl.NumberFormat('en-US', { 
                style: 'percent', 
                minimumFractionDigits: 2,
                maximumFractionDigits: 2
            }).format(v || 0); 
        },
        fmtRange(s) {
            if (s.isSpreadNA) return 'N/A';
            return (s.spreadRange * 100).toFixed(3) + '%';
        },
        spreadColor(spread) {
            const s = spread || 0;
            if (s < -0.0025) return 'text-green';  // < -0.25% = green (Binance cheaper)
            if (s > 0.0025) return 'text-red';     // > 0.25% = red (Binance expensive)
            return 'text-secondary';
        },
        rangeColor(s) {
            if (s.isSpreadNA) return 'text-dim';
            if (s.spreadRange > 0.005) return 'text-green';  // > 0.5% = good arbitrage
            if (s.spreadRange > 0.003) return 'text-gold';   // > 0.3% = medium
            return 'text-secondary';
        },
        spawn(symbol) {
            this.store.spawnBot(symbol);
        }
    }
};

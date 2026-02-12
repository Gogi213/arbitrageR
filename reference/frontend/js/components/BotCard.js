// Icons (Lucide)
const ICONS = {
    PLAY: `<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    STOP: `<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect></svg>`,
    TRASH: `<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>`,
    ALERT: `<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>`
};

export default {
    props: ['bot'],
    template: `
        <div class="bot-card" 
             :class="[borderClass, { 'selected': isSelected }]"
             @click="$emit('select', bot.tracker.symbol)">
             
            <!-- Header -->
            <div class="card-header">
                <div style="display:flex; flex-direction:column;">
                    <div class="symbol">{{ bot.tracker?.symbol }}</div>
                    <div class="status-badge" :class="statusColorClass" style="margin-top:4px; font-size:9px;">{{ statusText }}</div>
                </div>
                
                <!-- Quick Controls (Only visible on hover or if needed) -->
                <div class="card-controls" @click.stop>
                    <button v-if="!isTradingEnabled" class="btn btn-icon btn-primary" title="Start" @click="$emit('toggle', bot.tracker.symbol, 'start')" v-html="ICONS.PLAY"></button>
                    <button v-else class="btn btn-icon btn-danger" title="Stop" @click="$emit('toggle', bot.tracker.symbol, 'stop')" v-html="ICONS.STOP"></button>
                    
                    <button v-if="canDelete" class="btn btn-icon btn-ghost" title="Delete" @click="$emit('delete', bot.tracker.symbol)" v-html="ICONS.TRASH"></button>
                </div>
            </div>
            
            <!-- Main PnL (Odometer Style) -->
            <div style="text-align:right; margin-bottom:16px;">
                <div style="font-size:20px; font-weight:800;" :class="pnlClass">
                    {{ fmtMoney(bot.pnl) }}
                </div>
                <div style="font-size:10px; color:var(--text-secondary); display:flex; justify-content:flex-end; gap:8px;">
                    <span>uPnL: <span :class="uPnlClass">{{ fmtMoney(bot.uPnl) }}</span></span>
                    <span>{{ bot.trades || 0 }} TRADES</span>
                    <span v-if="bot.winRate !== undefined">WIN: {{ fmtPct(bot.winRate) }}</span>
                </div>
            </div>
            
            <!-- Metrics Grid -->
            <div class="metrics-grid">
                <!-- Spread -->
                <div>
                    <div class="metric-label">SPREAD</div>
                    <div class="metric-val" :class="spreadClass">{{ fmtPct(bot.tracker?.spreadPct) }}</div>
                </div>
                
                <!-- Half-Life Gauge -->
                <div style="flex:1; margin-left:12px;">
                    <div style="display:flex; justify-content:space-between; margin-bottom:4px;">
                        <span class="metric-label">HALF-LIFE</span>
                        <span class="metric-val" :class="hlClass">{{ Math.round(hlVal) }}s</span>
                    </div>
                    <div class="gauge-track">
                        <div class="gauge-fill" :style="hlStyle"></div>
                    </div>
                </div>
            </div>
            
            <!-- Structural Break Alert -->
            <div v-if="isBreak" class="break-alert">
                <span v-html="ICONS.ALERT" style="margin-right:6px;"></span>
                STRUCTURAL BREAK DETECTED
            </div>

        </div>
    `,
    setup() {
        return { ICONS };
    },
    computed: {
        isSelected() { return false; }, // Can be passed as prop later
        isTradingEnabled() { return this.bot.tracker?.isTradingEnabled; },
        isPaused() { return this.bot.tracker?.isPaused; },
        
        canDelete() {
            // Logic: Stopped + Flat + No Orders
            const isStopped = !this.isTradingEnabled;
            const isFlat = !this.bot.pos || this.bot.pos.q === 0;
            const noOrders = !this.bot.orders || this.bot.orders.length === 0;
            return isStopped && isFlat && noOrders;
        },
        
        borderClass() {
            if (!this.isTradingEnabled) return 'stopped-border';
            if (this.isPaused) return 'paused-border';
            return 'active-border';
        },
        statusText() {
            if (!this.isTradingEnabled) return 'STOPPED';
            if (this.isPaused) return 'PAUSED (RISK)';
            return 'ACTIVE';
        },
        statusColorClass() {
             if (!this.isTradingEnabled) return 'text-dim';
            if (this.isPaused) return 'text-gold';
            return 'text-green';
        },
        pnlClass() { return (this.bot.pnl || 0) >= 0 ? 'text-green' : 'text-red'; },
        uPnlClass() { return (this.bot.uPnl || 0) >= 0 ? 'text-green' : 'text-red'; },
        
        spreadClass() { 
            const s = (this.bot.tracker?.spreadPct || 0);
            return (s < -0.0025) ? 'text-green' : (s > 0.0025 ? 'text-red' : 'text-secondary');
        },
        
        // Half-Life Logic
        hlVal() { return this.bot.tracker?.halfLifeSec || 0; },
        isBreak() { return this.hlVal > 300; },
        hlClass() {
            if (this.hlVal > 300) return 'text-red';
            if (this.hlVal > 120) return 'text-gold';
            return 'text-green';
        },
        hlStyle() {
            // Visualize 0-300s range
            let pct = (this.hlVal / 300) * 100;
            if (pct > 100) pct = 100;
            
            let color = 'var(--neon-green)';
            if (this.hlVal > 120) color = 'var(--neon-gold)';
            if (this.hlVal > 300) color = 'var(--neon-red)';
            
            return {
                width: `${pct}%`,
                backgroundColor: color
            };
        }
    },
    methods: {
        fmtMoney(v) { return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(v || 0); },
        fmtPct(v) { return new Intl.NumberFormat('en-US', { style: 'percent', minimumFractionDigits: 2 }).format(v || 0); }
    }
};

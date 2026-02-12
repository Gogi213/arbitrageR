export default {
    props: ['bot'],
    template: `
        <div class="ladder-container">
            <div v-if="!hasData" style="text-align:center; padding:20px; color:var(--text-secondary);">
                WAITING FOR DATA...
            </div>
            <div v-else class="ladder-scroll">
                <div class="ladder-level" 
                     v-for="level in levels" 
                     :key="level.id"
                     :style="{ top: level.top + '%' }">
                     
                     <div class="level-marker" :class="level.type" :style="{ backgroundColor: level.bg, color: level.color }">
                        <span style="font-weight:700; font-size:9px; margin-right:4px;">{{ level.label }}</span>
                        <span style="font-family:'JetBrains Mono'; font-weight:600;">{{ level.price.toFixed(4) }}</span>
                     </div>
                     
                     <div class="level-line" :style="{ backgroundColor: level.color }"></div>
                </div>
            </div>
        </div>
    `,
    computed: {
        hasData() {
            return this.bot && this.bot.tracker && this.bot.tracker.fairValue > 0;
        },
        levels() {
            if (!this.hasData) return [];
            
            const t = this.bot.tracker;
            const items = [];
            
            // 1. Fair Value (Center)
            items.push({ price: t.fairValue, label: "FAIR", type: "fv", color: "var(--neon-blue)", bg: "rgba(41, 121, 255, 0.2)", z: 10 });
            
            // 2. Market
            if (t.marketAsk) items.push({ price: t.marketAsk, label: "ASK", type: "ask", color: "#ff5555", bg: "transparent", z: 5 });
            if (t.marketBid) items.push({ price: t.marketBid, label: "BID", type: "bid", color: "#55aa55", bg: "transparent", z: 5 });
            
            // 3. Our Orders
            // Sell Leg
            if (t.sellLeg && t.sellLeg.active) { // Only show active legs
                 items.push({ price: t.sellLeg.price, label: "SELL", type: "sell", color: "var(--neon-red)", bg: "rgba(255, 23, 68, 0.2)", z: 20 });
            }
            // Buy Leg
            if (t.buyLeg && t.buyLeg.active) {
                 items.push({ price: t.buyLeg.price, label: "BUY", type: "buy", color: "var(--neon-green)", bg: "rgba(0, 230, 118, 0.2)", z: 20 });
            }
            
            // 4. Entry Price
            if (t.entryPrice > 0) {
                items.push({ price: t.entryPrice, label: "ENTRY", type: "entry", color: "var(--neon-gold)", bg: "rgba(255, 215, 0, 0.2)", z: 15 });
            }

            // Calculate Positions (Zoom Logic)
            const center = t.fairValue;
            const ZOOM = 1500; // Sensitivity
            
            return items.map((item, idx) => {
                const diff = (item.price - center) / center;
                let topPct = 50 - (diff * ZOOM);
                
                // Clamping
                if (topPct < 2) topPct = 2;
                if (topPct > 98) topPct = 98;
                
                return {
                    id: idx,
                    ...item,
                    top: topPct
                };
            });
        }
    }
};

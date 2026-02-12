import { createApp } from 'vue';
import { store } from './store.js';
import BotCard from './components/BotCard.js';
import Ladder from './components/Ladder.js';
import StatsTable from './components/StatsTable.js';
import DebugConsole from './components/DebugConsole.js';
import LadderGrid from './components/LadderGrid.js';
import SystemDashboard from './components/SystemDashboard.js';

// --- Main App ---
const app = createApp({
    components: { BotCard, Ladder, StatsTable, DebugConsole, LadderGrid, SystemDashboard },
    setup() {
        return { store };
    },
    data() {
        return {
            currentTab: 'grid', // 'grid', 'table', 'debug'
            newBotSymbol: ''
        }
    },
    computed: {
        // Sort bots: Active first, then Paused, then Stopped
        sortedBots() {
            return [...store.bots].sort((a, b) => {
                const score = (bot) => {
                    if (bot.tracker?.isTradingEnabled && !bot.tracker?.isPaused) return 3;
                    if (bot.tracker?.isTradingEnabled && bot.tracker?.isPaused) return 2;
                    return 1;
                };
                return score(b) - score(a);
            });
        }
    },
    mounted() {
        // Start Polling
        this.poll();
        setInterval(this.poll, 100); // 10Hz
        
        // Remove loader
        const loader = document.getElementById('loader');
        if (loader) loader.style.display = 'none';
        
        store.log('System initialized. Ready to trade.', 'success');
    },
    methods: {
        async poll() {
            await store.fetchStats();
        },
        spawn() {
            if (!this.newBotSymbol) return;
            const s = this.newBotSymbol.toUpperCase();
            store.spawnBot(s);
            store.log(`Spawning bot ${s}...`);
            this.newBotSymbol = '';
        },
        quickSpawn(symbol) {
            store.spawnBot(symbol);
            store.log(`Quick spawn ${symbol}...`);
        },
        onSymbolInput(event) {
            let val = event.target.value;
            // Layout Fix (RU -> EN QWERTY)
            const map = {
                'й':'q', 'ц':'w', 'у':'e', 'к':'r', 'е':'t', 'н':'y', 'г':'u', 'ш':'i', 'щ':'o', 'з':'p', 'х':'[', 'ъ':']',
                'ф':'a', 'ы':'s', 'в':'d', 'а':'f', 'п':'g', 'р':'h', 'о':'j', 'л':'k', 'д':'l', 'ж':';', 'э':'\'',
                'я':'z', 'ч':'x', 'с':'c', 'м':'v', 'и':'b', 'т':'n', 'ь':'m', 'б':',', 'ю':'.'
            };
            
            val = val.split('').map(c => {
                const lower = c.toLowerCase();
                return map[lower] ? map[lower] : c;
            }).join('');
            
            // Force Upper & Clean
            this.newBotSymbol = val.toUpperCase().replace(/[^A-Z0-9_]/g, '');
        },
        fmtMoney(v) { return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(v || 0); }
    }
});

app.mount('#app');

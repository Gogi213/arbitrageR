import { reactive } from 'vue';

const API_URL = '/api/paper/stats';

// Define state first
const store = reactive({
    // --- State ---
    system: {
        lastUpdate: 0,
        isConnected: false,
        latencyMs: 0
    },
    
    bots: [],          // Raw bot data from API
    logs: [],          // System Logs
    history: [],       // PnL History for Charts
    screener: [],      // SPRINT 34: Screener Data
    selectedBotSymbol: null, // Currently inspected bot
    
    // --- Computed Helpers (Getters) ---
    get selectedBot() {
        if (!this.selectedBotSymbol) return null;
        return this.bots.find(b => b.tracker?.symbol === this.selectedBotSymbol);
    },
    
    get activeBotCount() {
        return this.bots.filter(b => b.tracker?.isTradingEnabled).length;
    },
    
    get totalPnL() {
        return this.bots.reduce((sum, b) => sum + (b.pnl || 0), 0);
    },

    get totalUnrealizedPnL() {
        return this.bots.reduce((sum, b) => sum + (b.uPnl || 0), 0);
    }
});

// Define Actions (Methods) attached to store
store.fetchStats = async function() {
    const start = performance.now();
    try {
        const res = await fetch(API_URL);
        if (!res.ok) throw new Error("API Error");
        
        const data = await res.json();
        
        // Update State
        store.bots = data;
        store.system.isConnected = true;
        store.system.lastUpdate = Date.now();
        store.system.latencyMs = Math.round(performance.now() - start);
        
        // Record History (Max 300 points)
        if (store.system.lastUpdate % 1000 < 500) { // Approx 1Hz
            store.history.push({
                ts: Date.now(),
                pnl: store.totalPnL,
                uPnl: store.totalUnrealizedPnL
            });
            if (store.history.length > 300) store.history.shift();
            
            // Poll Screener occasionally (1Hz)
            store.fetchScreener();
        }
        
    } catch (err) {
        console.error(err);
        store.system.isConnected = false;
    }
};

store.fetchScreener = async function() {
    try {
        const res = await fetch('/api/screener/stats');
        if (res.ok) {
            store.screener = await res.json();
        }
    } catch (e) {
        // Silent fail
    }
};

store.selectBot = function(symbol) {
    store.selectedBotSymbol = symbol;
};

store.spawnBot = async function(symbol) {
    if (!symbol) return;
    try {
        await fetch('/api/bot/spawn', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ symbol })
        });
        store.log(`Spawned ${symbol}`, 'success');
        await store.fetchStats(); // Immediate refresh
    } catch (e) {
        store.log(`Spawn failed: ${e.message}`, 'error');
    }
};

store.deleteBot = async function(symbol) {
    if (!confirm(`Permanently delete ${symbol}?`)) return;
    try {
        await fetch(`/api/bot/${symbol}`, { method: 'DELETE' });
        store.log(`Deleted ${symbol}`, 'success');
        await store.fetchStats();
    } catch (e) {
        store.log(`Delete failed: ${e.message}`, 'error');
    }
};

store.toggleBot = async function(symbol, action) { // action = 'start' | 'stop'
    try {
        await fetch(`/api/bot/${symbol}/${action}`, { method: 'POST' });
        store.log(`${action.toUpperCase()} ${symbol}`);
        await store.fetchStats();
    } catch (e) {
        store.log(`Action failed: ${e.message}`, 'error');
    }
};

store.log = function(msg, type='info') {
    const ts = new Date().toLocaleTimeString();
    store.logs.unshift({ ts, msg, type });
    if (store.logs.length > 100) store.logs.pop();
};

export { store };

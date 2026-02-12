export default {
    props: ['logs'],
    template: `
        <div class="console-container">
            <div v-for="(log, i) in logs" :key="i" class="console-line">
                <span class="text-dim">[{{ log.ts }}]</span>
                <span :class="typeColor(log.type)" style="margin-left:8px;">{{ log.msg }}</span>
            </div>
            <div v-if="logs.length === 0" class="text-dim" style="padding:20px;">System Ready. No logs yet.</div>
        </div>
    `,
    methods: {
        typeColor(type) {
            if (type === 'error') return 'text-red';
            if (type === 'success') return 'text-green';
            return 'text-primary';
        }
    }
};

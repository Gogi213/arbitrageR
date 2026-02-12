import Ladder from './Ladder.js';

export default {
    components: { Ladder },
    props: ['bots'],
    template: `
        <div class="ladder-grid-container">
            <div v-if="bots.length === 0" class="text-dim" style="text-align:center; padding:50px;">
                NO ACTIVE LADDERS
            </div>
            <div v-else class="grid-layout">
                <div v-for="bot in bots" :key="bot.tracker?.symbol" class="ladder-card-wrapper">
                    <div class="ladder-header">{{ bot.tracker?.symbol }}</div>
                    <ladder :bot="bot" class="grid-ladder-component" style="flex:1;"></ladder>
                </div>
            </div>
        </div>
    `,
};

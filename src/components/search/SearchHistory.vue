<template>
    <section class="search-history" v-if="history.length > 0">
        <div class="history-header">
            <h2 class="history-title">搜索历史</h2>
            <n-button
                text
                type="primary"
                class="clear-btn"
                @click="$emit('clear')"
            >
                清除历史
            </n-button>
        </div>
        <!-- M3 filter chip：一条芯片里是两个独立控件（重新搜索 / 删除），
             两者命中区域都是 48dp，且都能用键盘到达。 -->
        <ul class="history-tags">
            <li v-for="item in history" :key="item" class="history-item">
                <div class="history-chip">
                    <button
                        type="button"
                        class="chip-label"
                        :aria-label="`重新搜索 ${item}`"
                        @click="$emit('select', item)"
                    >
                        {{ item }}
                    </button>
                    <button
                        type="button"
                        class="chip-remove"
                        :aria-label="`删除历史记录 ${item}`"
                        @click="$emit('remove', item)"
                    >
                        <svg
                            viewBox="0 0 24 24"
                            width="18"
                            height="18"
                            aria-hidden="true"
                        >
                            <path
                                d="M6 6l12 12M18 6 6 18"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                            />
                        </svg>
                    </button>
                </div>
            </li>
        </ul>
    </section>
</template>

<script setup lang="ts">
import { NButton } from 'naive-ui'

defineProps<{
    history: string[]
}>()

defineEmits<{
    (e: 'select', keyword: string): void
    (e: 'remove', keyword: string): void
    (e: 'clear'): void
}>()
</script>

<style scoped>
.search-history {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-2);
    margin-bottom: var(--md-space-6);
}

.history-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-3);
    min-width: 0;
}

.history-title {
    margin: 0;
    font-size: var(--md-title-small);
    line-height: var(--md-title-small-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface-variant);
}

.clear-btn {
    flex-shrink: 0;
    padding: 0 var(--md-space-3);
    --n-height: var(--md-target-min);
}

.history-tags {
    display: flex;
    flex-wrap: wrap;
    gap: var(--md-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
}

.history-item {
    max-width: 100%;
    min-width: 0;
}

/* 描边芯片外形，高度 48dp；两个内部控件各自达到 48dp 命中区域。 */
.history-chip {
    display: inline-flex;
    align-items: stretch;
    max-width: 100%;
    min-height: var(--md-target-min);
    border: 1px solid var(--md-outline);
    border-radius: var(--md-shape-full);
    background-color: var(--md-surface);
}

.chip-label {
    flex: 1 1 auto;
    min-width: var(--md-target-min);
    min-height: var(--md-target-min);
    padding: 0 var(--md-space-3) 0 var(--md-space-4);
    border: none;
    border-radius: var(--md-shape-full) 0 0 var(--md-shape-full);
    background-color: transparent;
    color: var(--md-on-surface);
    font-family: inherit;
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    text-align: left;
    white-space: normal;
    overflow-wrap: anywhere;
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.chip-label:hover {
    background-color: var(--md-surface-container-high);
}

.chip-remove {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--md-target-min);
    min-height: var(--md-target-min);
    padding: 0;
    border: none;
    border-radius: 0 var(--md-shape-full) var(--md-shape-full) 0;
    background-color: transparent;
    color: var(--md-on-surface-variant);
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.chip-remove:hover {
    background-color: var(--md-surface-container-highest);
    color: var(--md-on-surface);
}

@media (prefers-reduced-motion: reduce) {
    .chip-label,
    .chip-remove {
        transition: none;
    }
}
</style>

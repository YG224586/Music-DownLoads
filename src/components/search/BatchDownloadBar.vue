<template>
    <div
        v-if="selectedCount > 0"
        class="batch-bar"
        role="region"
        aria-label="批量下载操作栏"
    >
        <span class="selected-text" aria-live="polite"
            >已选择 {{ selectedCount }} 首</span
        >
        <n-button
            type="primary"
            class="batch-download-btn"
            @click="$emit('batch-download')"
        >
            批量下载
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { NButton } from 'naive-ui'

defineProps<{
    selectedCount: number
}>()

defineEmits<{
    (e: 'batch-download'): void
}>()
</script>

<style scoped>
/* 上下文操作栏：粘在滚动区底部，圆角朝上，一个 filled 主操作。 */
.batch-bar {
    position: sticky;
    bottom: 0;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-2) var(--md-space-3);
    min-width: 0;
    margin-top: var(--md-space-6);
    padding: var(--md-space-3) var(--md-space-4);
    background-color: var(--md-surface-container-high);
    border-top: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-lg) var(--md-shape-lg) 0 0;
    box-shadow: var(--md-elevation-2);
}

.selected-text {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    min-width: 0;
}

.batch-download-btn.n-button {
    flex-shrink: 0;
}

@media (max-width: 599px) {
    .batch-bar {
        padding: var(--md-space-2) var(--md-space-3);
    }

    .batch-download-btn.n-button {
        flex: 1 1 auto;
    }
}
</style>

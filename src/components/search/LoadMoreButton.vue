<template>
    <div class="load-more-wrapper">
        <n-button
            class="load-more-btn"
            :loading="loading"
            :disabled="disabled"
            @click="$emit('click')"
        >
            {{ loading ? '加载中...' : '加载更多' }}
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { NButton } from 'naive-ui'

defineProps<{
    loading: boolean
    disabled: boolean
}>()

defineEmits<{
    (e: 'click'): void
}>()
</script>

<style scoped>
.load-more-wrapper {
    display: flex;
    justify-content: center;
    margin-top: var(--md-space-6);
}

/* M3 outlined 按钮：低权重下一步动作，仍保持 48dp 命中区域。 */
.load-more-btn.n-button {
    width: 100%;
    max-width: 320px;
    min-height: var(--md-target-min);
    --n-color: transparent;
    --n-color-hover: var(--md-surface-container-high);
    --n-color-pressed: var(--md-surface-container-highest);
    --n-color-focus: var(--md-surface-container-high);
    --n-text-color: var(--md-primary);
    --n-text-color-hover: var(--md-primary);
    --n-text-color-pressed: var(--md-primary);
    --n-text-color-focus: var(--md-primary);
    --n-border: 1px solid var(--md-outline);
    --n-border-hover: 1px solid var(--md-primary);
    --n-border-pressed: 1px solid var(--md-primary);
    --n-border-focus: 1px solid var(--md-primary);
    --n-border-radius: var(--md-shape-full);
}
</style>

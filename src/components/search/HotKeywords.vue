<template>
    <section v-if="loading" class="hot-loading" aria-live="polite">
        <n-spin size="small" />
        <span class="hot-loading-text">正在加载热搜…</span>
    </section>
    <section v-else-if="keywords.length > 0" class="hot-keywords">
        <h2 class="hot-header">热搜推荐</h2>
        <!-- M3 assist chip：一次点击即发起搜索，不做多选 -->
        <ul class="hot-list">
            <li v-for="word in keywords" :key="word">
                <n-tag
                    class="hot-tag"
                    role="button"
                    tabindex="0"
                    :aria-label="`搜索 ${word}`"
                    @click="$emit('select', word)"
                    @keydown.enter.prevent="$emit('select', word)"
                    @keydown.space.prevent="$emit('select', word)"
                >
                    {{ word }}
                </n-tag>
            </li>
        </ul>
    </section>
</template>

<script setup lang="ts">
import { NTag, NSpin } from 'naive-ui'

defineProps<{
    keywords: string[]
    loading: boolean
}>()

defineEmits<{
    (e: 'select', word: string): void
}>()
</script>

<style scoped>
.hot-keywords {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-2);
}

.hot-header {
    margin: 0;
    font-size: var(--md-title-small);
    line-height: var(--md-title-small-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface-variant);
}

.hot-list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--md-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
}

.hot-list li {
    max-width: 100%;
    min-width: 0;
}

.hot-tag.n-tag {
    box-sizing: border-box;
    height: auto;
    min-height: var(--md-target-min);
    padding: var(--md-space-1) var(--md-space-4);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-full);
    background-color: var(--md-surface-container-low);
    color: var(--md-on-surface);
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.hot-tag.n-tag:hover {
    background-color: var(--md-surface-container-high);
}

.hot-tag :deep(.n-tag__content) {
    min-width: 0;
    white-space: normal;
    overflow-wrap: anywhere;
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
}

.hot-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--md-space-2);
    padding: var(--md-space-4) 0;
}

.hot-loading-text {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
}

@media (prefers-reduced-motion: reduce) {
    .hot-tag.n-tag {
        transition: none;
    }
}
</style>

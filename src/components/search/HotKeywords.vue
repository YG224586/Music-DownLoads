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

/*
 * 行间距 16dp = 芯片上下各外扩 8dp 的命中区之和，相邻两行刚好拼满不重叠；
 * 列间距保持 8dp，横向用 pointer 精度，不需要额外补偿。
 */
.hot-list {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--md-space-2);
    row-gap: var(--md-space-4);
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
}

.hot-list li {
    max-width: 100%;
    min-width: 0;
}

/*
 * M3 chip：视觉高 32dp、圆角 8dp（不是胶囊）。命中区仍要 48dp，
 * 用 ::after 上下各外扩 9dp 撑到 48dp —— 事件照常冒泡到 n-tag 自身。
 * 注意外扩取 9dp 而非 8dp：绝对定位伪元素的包含块是**内边距盒**，
 * 1px 边框会被排除在外，8dp 只能得到 46dp（独立验证 P3-1 实测）。
 */
.hot-tag.n-tag {
    box-sizing: border-box;
    position: relative;
    height: 32px;
    padding: 0 var(--md-space-3);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-chip);
    background-color: var(--md-surface-container-low);
    color: var(--md-on-surface);
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.hot-tag.n-tag::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: -9px;
    bottom: -9px;
}

.hot-tag.n-tag:hover {
    background-color: var(--md-surface-container-high);
}

/* 芯片固定 32dp 高，文字必须单行，过长时省略号截断。 */
.hot-tag :deep(.n-tag__content) {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

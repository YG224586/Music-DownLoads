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
                        <span class="chip-text">{{ item }}</span>
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

/*
 * 行间距 16dp = 两个内部控件各自上下外扩 8dp 的命中区之和，相邻两行刚好拼满不重叠。
 */
.history-tags {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--md-space-2);
    row-gap: var(--md-space-4);
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
}

.history-item {
    max-width: 100%;
    min-width: 0;
}

/*
 * M3 chip：视觉高 32dp、圆角 8dp。box-sizing: border-box 让 1px 上下描边算进 32dp，
 * 否则内容盒 + 两侧描边会渲染成 34dp。容器本身不能挂 ::after（会盖住内部两个按钮、
 * 导致点不动），命中区由 .chip-label / .chip-remove 各自外扩 9dp 撑到 48dp。
 */
.history-chip {
    box-sizing: border-box;
    display: inline-flex;
    align-items: stretch;
    max-width: 100%;
    height: 32px;
    border: 1px solid var(--md-outline);
    border-radius: var(--md-shape-chip);
    background-color: var(--md-surface);
}

.chip-label {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1 1 auto;
    min-width: 32px;
    padding: 0 var(--md-space-3) 0 var(--md-space-4);
    border: none;
    border-radius: var(--md-shape-chip) 0 0 var(--md-shape-chip);
    background-color: transparent;
    color: var(--md-on-surface);
    font-family: inherit;
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

/*
 * 省略号必须落在内部 span 上：按钮自身要 overflow: visible，
 * 否则 ::after 撑出的命中区会一起被裁掉（实测裁剪后上下 4dp 已经点不到芯片）。
 */
.chip-text {
    display: block;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.chip-label::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: -9px;
    bottom: -9px;
}

.chip-label:hover {
    background-color: var(--md-surface-container-high);
}

.chip-remove {
    position: relative;
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    padding: 0;
    border: none;
    border-radius: 0 var(--md-shape-chip) var(--md-shape-chip) 0;
    background-color: transparent;
    color: var(--md-on-surface-variant);
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

/* 命中区只做上下外扩：向左右扩会盖住 .chip-label，反而抢走它的点击。 */
.chip-remove::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: -9px;
    bottom: -9px;
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

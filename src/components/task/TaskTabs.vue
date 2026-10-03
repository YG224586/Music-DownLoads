<template>
    <!-- 紧凑窗口：7 个筛选项在 390dp 下无法一次排开，改用可换行的 M3 过滤 chip，
         既不横向滚动也不裁切标签文字；选中态用 secondaryContainer 表达。 -->
    <div
        v-if="isNarrow"
        class="task-tabs-compact"
        role="tablist"
        aria-label="任务筛选"
    >
        <button
            v-for="tab in tabs"
            :key="tab.name"
            type="button"
            role="tab"
            class="task-filter-chip"
            :class="{ 'is-selected': tab.name === activeTab }"
            :aria-selected="tab.name === activeTab"
            @click="emit('update:activeTab', tab.name)"
        >
            {{ tab.label }}
        </button>
    </div>

    <!-- 更宽窗口：分段标签一次全部可见，形态与断点前的行为保持一致。 -->
    <n-tabs
        v-else
        :value="activeTab"
        :type="'segment'"
        size="medium"
        @update:value="emit('update:activeTab', $event)"
    >
        <n-tab-pane
            v-for="tab in tabs"
            :key="tab.name"
            :name="tab.name"
            :tab="tab.label"
        />
    </n-tabs>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { NTabs, NTabPane } from 'naive-ui'
import { useNarrowLayout } from '../../composables/useNarrowLayout'

// 与外壳断点一致：窄屏用 chip，宽屏用分段标签。
const isNarrow = useNarrowLayout()

export interface TabCounts {
    total: number
    waiting: number
    downloading: number
    paused: number
    completed: number
    interrupted: number
    error: number
}

const props = defineProps<{
    activeTab: string
    counts: TabCounts
}>()

const emit = defineEmits<{
    (e: 'update:activeTab', value: string): void
}>()

/** 标签顺序、名称与计数文案与改造前完全一致。 */
const tabs = computed(() => [
    { name: 'all', label: `全部 (${props.counts.total})` },
    { name: 'waiting', label: `等待中 (${props.counts.waiting})` },
    { name: 'downloading', label: `下载中 (${props.counts.downloading})` },
    { name: 'paused', label: `暂停 (${props.counts.paused})` },
    { name: 'completed', label: `已完成 (${props.counts.completed})` },
    { name: 'interrupted', label: `已中断 (${props.counts.interrupted})` },
    { name: 'error', label: `错误 (${props.counts.error})` },
])
</script>

<style scoped>
/* 7 个筛选项在 390dp 下无法一次排开：改为单行横向滚动（M3 scrollable tabs 模式），
   不再换行占掉两行高度，筛选行始终只有一行 48dp。 */
.task-tabs-compact {
    display: flex;
    flex-wrap: nowrap;
    gap: var(--md-space-2);
    min-width: 0;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
    -ms-overflow-style: none;
}

.task-tabs-compact::-webkit-scrollbar {
    display: none;
}

/* M3 过滤 chip：描边未选中 / secondaryContainer 选中，命中区域 48dp。 */
.task-filter-chip {
    flex: 0 0 auto;
    min-height: var(--md-target-min);
    padding: 0 var(--md-space-4);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-chip);
    background-color: transparent;
    color: var(--md-on-surface-variant);
    font-family: inherit;
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    font-weight: var(--md-weight-medium);
    white-space: nowrap;
    cursor: pointer;
    transition:
        background-color var(--md-duration-short) var(--md-easing-standard),
        color var(--md-duration-short) var(--md-easing-standard);
}

.task-filter-chip:hover {
    background-color: var(--md-surface-container-high);
}

.task-filter-chip.is-selected {
    background-color: var(--md-secondary-container);
    border-color: transparent;
    color: var(--md-on-secondary-container);
}

/* 横向滚动容器会裁掉外扩的焦点环，改为内描边以保证可见 */
.task-tabs-compact .task-filter-chip:focus-visible {
    outline-offset: -3px;
}
</style>

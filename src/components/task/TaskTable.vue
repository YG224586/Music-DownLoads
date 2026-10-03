<template>
    <MobileTaskList
        v-if="isMobile"
        :tasks="tasks"
        :selected-row-keys="selectedRowKeys"
        :is-android="isAndroid"
        @update:selected-row-keys="
            (keys) => emit('update:selectedRowKeys', keys)
        "
        @action="
            (action, taskId, extra) => emit('action', action, taskId, extra)
        "
    />
    <n-data-table
        v-else
        class="task-table"
        :columns="columns"
        :data="tasks"
        :row-key="(row: TaskRecord) => row.id"
        :scroll-x="780"
        table-layout="fixed"
        :checked-row-keys="selectedRowKeys"
        @update:checked-row-keys="handleCheckedRowKeys"
    >
        <!-- Naive 默认空态是英文 “No Data”，这里与紧凑列表保持同一文案 -->
        <template #empty>
            <div class="task-table-empty" role="status">
                <span class="task-table-empty-icon" v-html="EMPTY_ICON" />
                <p class="task-table-empty-text">暂无任务</p>
            </div>
        </template>
    </n-data-table>
</template>

<script setup lang="ts">
import { h, ref, onMounted, computed } from 'vue'
import { NDataTable, NProgress, NSpace, NEllipsis } from 'naive-ui'
import type { DataTableColumn, DataTableRowKey } from 'naive-ui'
import type { TaskRecord } from '../../types'
import { formatSpeed } from '../../utils/format'
import { renderActions } from './TaskRowActions'
import type { TaskAction, TaskActionExtra } from './TaskRowActions'
import MobileTaskList from './MobileTaskList.vue'
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { getRuntimePlatform } from '../../api/runtimeApi'

// 使用 ref 存储 Android 状态，替代原先的同步 UA 判断
// 原生平台信息由 API 层提供，在 onMounted 中异步获取并更新。
const isAndroid = ref(false)

// 响应式检测移动端：与导航共用断点，监听由 composable 随组件释放
const isMobile = useNarrowLayout()

onMounted(async () => {
    // 异步获取当前平台，设置 isAndroid
    try {
        const currentPlatform = await getRuntimePlatform()
        // Web 文件位于服务器，和 Android SAF 一样展示路径但不显示本机“打开位置”。
        isAndroid.value =
            currentPlatform === 'android' || currentPlatform === 'web'
    } catch (error) {
        console.warn('获取平台信息失败，默认按非 Android 处理', error)
        isAndroid.value = false
    }
})

const props = defineProps<{
    tasks: TaskRecord[]
    selectedRowKeys: string[]
}>()

// 紧凑与宽屏两种呈现共用同一份数据契约
void props

const emit = defineEmits<{
    (e: 'update:selectedRowKeys', keys: string[]): void
    // 增加第三个参数 extra，用于传递删除文件标志等
    (
        e: 'action',
        action: TaskAction,
        taskId: string,
        extra?: TaskActionExtra,
    ): void
}>()

function handleCheckedRowKeys(keys: DataTableRowKey[]) {
    emit('update:selectedRowKeys', keys.map(String))
}

/** 宽屏表格的空态图标：与紧凑列表同源，仅作视觉提示。 */
const EMPTY_ICON =
    '<svg viewBox="0 0 48 48" width="48" height="48" aria-hidden="true"><rect x="9" y="12" width="30" height="26" rx="4" fill="none" stroke="currentColor" stroke-width="2"/><path d="M9 20h30" fill="none" stroke="currentColor" stroke-width="2"/><path d="M24 25v7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="m20.6 28.6 3.4 3.4 3.4-3.4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>'

/**
 * 状态元信息：与紧凑列表使用同一套“图标 + 文字 + 容器色”规则，
 * 状态不因屏宽变化而只靠颜色表达。
 */
const STATUS_META: Record<
    string,
    { tone: string; label: string; icon: string }
> = {
    waiting: {
        tone: 'is-info',
        label: '等待中',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 8.2v4.1l2.9 1.8" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    },
    downloading: {
        tone: 'is-info',
        label: '下载中',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M12 4.5v10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="m8.2 10.8 3.8 3.8 3.8-3.8" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.5 19h13" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    },
    paused: {
        tone: 'is-warning',
        label: '暂停',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M9.6 5.5v13M14.4 5.5v13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>',
    },
    completed: {
        tone: 'is-success',
        label: '已完成',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="m5.5 12.5 4.5 4.5 8.5-10" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    },
    error: {
        tone: 'is-error',
        label: '错误',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 7.8v5.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><circle cx="12" cy="16.2" r="1.05" fill="currentColor"/></svg>',
    },
    processing: {
        tone: 'is-info',
        label: '处理中',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M12 4.5a7.5 7.5 0 1 1-7.4 6.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="M12 4.5v3.6M4.6 10.7h3.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    },
    interrupted: {
        tone: 'is-warning',
        label: '已中断',
        icon: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M7 4.5h10M7 19.5h10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="M8.6 4.5c0 4 6.8 5.4 6.8 9.4M15.4 4.5c0 4-6.8 5.4-6.8 9.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    },
}

/** 状态 chip：图标 + 文字，容器色成对使用。 */
function renderStatusChip(row: TaskRecord) {
    const meta = STATUS_META[row.status] || {
        tone: 'is-info',
        label: row.status,
        icon: '',
    }
    return h('span', { class: ['task-chip', meta.tone] }, [
        h('span', { class: 'task-chip-icon', innerHTML: meta.icon }),
        h('span', null, meta.label),
    ])
}

/**
 * 渲染进度列：进度条旁边始终有百分比文字，进度不只用颜色/长度表达。
 */
function renderProgress(row: TaskRecord) {
    const percent =
        row.fileSize > 0 ? Math.round((row.downloaded / row.fileSize) * 100) : 0

    // 已完成或处理中状态：显示100%
    if (row.status === 'completed' || row.status === 'processing') {
        return '100%'
    }

    // 中断状态由 Rust 在重启恢复时写入，恢复动作由用户主动发起。
    if (row.status === 'interrupted') {
        return '上次运行中断，等待恢复'
    }

    // 错误状态显示错误信息
    if (row.status === 'error') {
        return row.errorMsg || ''
    }

    // 等待中
    if (row.status === 'waiting') {
        return '-'
    }

    // 下载中或暂停：显示进度条、百分比与速度
    const children = [
        h(
            'div',
            {
                class: 'task-progress-row',
                role: 'progressbar',
                'aria-valuenow': percent,
                'aria-valuemin': 0,
                'aria-valuemax': 100,
                'aria-label': '下载进度',
            },
            [
                h('div', { class: 'task-progress-bar' }, [
                    h(NProgress, {
                        percentage: percent,
                        height: 8,
                        showIndicator: false,
                        borderRadius: 4,
                    }),
                ]),
                h('span', { class: 'task-progress-text' }, `${percent}%`),
            ],
        ),
    ]

    if (row.speed && row.speed > 0) {
        children.push(h('div', { class: 'task-speed' }, formatSpeed(row.speed)))
    }

    return h('div', null, children)
}

// Android/Web 文件路径列依赖异步平台检测，列定义必须是计算属性才能随检测结果更新。
const columns = computed<DataTableColumn<TaskRecord>[]>(() => [
    {
        type: 'selection',
        // 48dp：勾选框本身没有文字标签，命中区必须靠列宽补足
        width: 48,
        disabled: (row: TaskRecord) => row.status === 'downloading',
    },
    {
        title: '歌曲信息',
        key: 'song',
        // 文件路径并入本列第二行，去掉单独的长路径列后 900px 视口内不再需要横向滚动
        minWidth: 220,
        render(row: TaskRecord) {
            const lines = [
                h('div', { class: 'song-line' }, [
                    h(
                        'span',
                        { class: 'song-title' },
                        row.songTitle || '未知歌曲',
                    ),
                    h('span', { class: 'song-separator' }, ' - '),
                    h(
                        'span',
                        { class: 'song-artist' },
                        row.artist || '未知歌手',
                    ),
                ]),
            ]
            // Android 无法使用桌面文件管理器，因此直接展示文件路径；无路径的行不渲染占位符。
            if (isAndroid.value && row.filePath) {
                lines.push(
                    h('div', { class: 'song-filepath' }, [
                        h(
                            NEllipsis,
                            {
                                style: { maxWidth: '100%' },
                                expandTrigger: 'click',
                                lineClamp: 1,
                                tooltip: false, // 禁用 tooltip，改用点击展开
                            },
                            () => row.filePath,
                        ),
                    ]),
                )
            }
            return h('div', { class: 'song-info' }, lines)
        },
    },
    {
        title: '音质',
        key: 'quality',
        width: 80,
        render(row: TaskRecord) {
            return row.quality
        },
    },
    {
        title: '状态',
        key: 'status',
        width: 96,
        render(row: TaskRecord) {
            return renderStatusChip(row)
        },
    },
    {
        title: '进度',
        key: 'progress',
        width: 140,
        render(row: TaskRecord) {
            return renderProgress(row)
        },
    },
    {
        title: '操作',
        key: 'actions',
        width: 200,
        render(row: TaskRecord) {
            return h(
                NSpace,
                { justify: 'center', class: 'task-table-actions' },
                () =>
                    renderActions(row, {
                        // 显式传递第三个参数，确保 extra 不被丢弃
                        emit: (
                            action: TaskAction,
                            taskId: string,
                            extra?: TaskActionExtra,
                        ) => {
                            emit('action', action, taskId, extra)
                        },
                        isAndroid: isAndroid.value,
                    }),
            )
        },
    },
])

// props 直接透传给表格渲染函数使用，避免未使用告警
void props
</script>

<style scoped>
.task-table {
    min-width: 0;
    flex-shrink: 0;
}

/* 表格本体做成一张圆角 surface，与紧凑列表的容器形态一致。 */
.task-table :deep(.n-data-table-base-table) {
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-lg);
    overflow: hidden;
}

.task-table :deep(.n-data-table-th) {
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    font-weight: var(--md-weight-medium);
}

/* 固定列布局配合表格内部横向滚动，长内容在单元格内换行或省略 */
.task-table :deep(.n-data-table-td) {
    overflow-wrap: anywhere;
    font-size: var(--md-body-medium);
    vertical-align: middle;
}

/*
 * 选择列是纯勾选框（无文字标签），命中区必须 ≥48x48：
 * 首列去掉单元格左右内边距，勾选框在 48px 列宽内居中铺满。
 */
.task-table :deep(.n-data-table-th:first-child),
.task-table :deep(.n-data-table-td:first-child) {
    padding-left: 0;
    padding-right: 0;
}

.task-table :deep(.n-data-table-th:first-child .n-checkbox),
.task-table :deep(.n-data-table-td:first-child .n-checkbox) {
    justify-content: center;
    min-width: var(--md-target-min);
    min-height: var(--md-target-min);
}

/* 行内操作：48dp 命中区 + 最小宽度，避免 2 字标签被压成圆形按钮 */
.task-table :deep(.task-table-actions .n-button) {
    min-height: var(--md-target-min);
    min-width: 72px;
    border-radius: var(--md-shape-full);
    padding-left: var(--md-space-4);
    padding-right: var(--md-space-4);
}

/* 「更多」菜单触发器是纯图标圆形按钮：保持 48x48，不被上面的最小宽度/内边距撑开 */
.task-table :deep(.task-table-actions .n-button[aria-haspopup='menu']) {
    min-width: var(--md-target-min);
    padding-left: 0;
    padding-right: 0;
}

.task-table :deep(.task-progress-row) {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
    min-width: 0;
}

.task-table :deep(.task-progress-bar) {
    flex: 1;
    min-width: 0;
}

.task-table :deep(.task-progress-text) {
    flex-shrink: 0;
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    font-variant-numeric: tabular-nums;
}

.task-table :deep(.task-speed) {
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    margin-top: var(--md-space-1);
}

/* 列内容由表格的 render 回调创建，使用 deep 将样式限定在当前表格内 */
.task-table :deep(.song-info) {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
}

.task-table :deep(.song-line) {
    display: flex;
    flex-direction: row;
    align-items: baseline;
    flex-wrap: nowrap;
    min-width: 0;
}

.task-table :deep(.song-title) {
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    /* 允许收缩 */
    min-width: 0;
}

.task-table :deep(.song-separator) {
    margin: 0 4px;
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-small);
    flex-shrink: 0;
    /* 分隔符不收缩 */
}

.task-table :deep(.song-artist) {
    font-size: var(--md-body-small);
    color: var(--md-on-surface-variant);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
}

.task-table :deep(.song-filepath) {
    min-width: 0;
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
}

/* ---------- chip（与紧凑列表同一套语义色） ---------- */
.task-table :deep(.task-chip) {
    display: inline-flex;
    align-items: center;
    gap: var(--md-space-1);
    max-width: 100%;
    padding: 2px var(--md-space-2);
    border-radius: var(--md-shape-sm);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    font-weight: var(--md-weight-medium);
    white-space: nowrap;
}

.task-table :deep(.task-chip-icon) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
}

.task-table :deep(.task-chip.is-info) {
    background-color: var(--md-tertiary-container);
    color: var(--md-on-tertiary-container);
}

.task-table :deep(.task-chip.is-warning) {
    background-color: var(--md-warning-container);
    color: var(--md-on-warning-container);
}

.task-table :deep(.task-chip.is-success) {
    background-color: var(--md-success-container);
    color: var(--md-on-success-container);
}

.task-table :deep(.task-chip.is-error) {
    background-color: var(--md-error-container);
    color: var(--md-on-error-container);
}

/* ---------- 空态 ---------- */
.task-table :deep(.task-table-empty) {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--md-space-3);
    padding: var(--md-space-8) var(--md-space-4);
    color: var(--md-on-surface-variant);
}

.task-table :deep(.task-table-empty-icon) {
    display: inline-flex;
}

.task-table :deep(.task-table-empty-text) {
    margin: 0;
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
}
</style>

<template>
    <!-- 该组件使用 render 函数渲染，故 template 为空 -->
</template>

<script lang="ts">
import { defineComponent, h, type PropType } from 'vue'
import { NCheckbox, NProgress, NEllipsis } from 'naive-ui'
import type { TaskRecord } from '../../types'
import { formatSpeed } from '../../utils/format'
import { renderActions } from './TaskRowActions'
import type { TaskAction, TaskActionExtra } from './TaskRowActions'

/**
 * 状态图标：状态不能只用颜色表达，chip 内始终"图标 + 文字"成对出现。
 * 16dp，跟随 chip 文字色（currentColor）。
 */
const STATUS_ICONS: Record<string, string> = {
    waiting:
        '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 8.2v4.1l2.9 1.8" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    downloading:
        '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M12 4.5v10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="m8.2 10.8 3.8 3.8 3.8-3.8" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.5 19h13" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    paused: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M9.6 5.5v13M14.4 5.5v13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>',
    completed:
        '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="m5.5 12.5 4.5 4.5 8.5-10" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    error: '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 7.8v5.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><circle cx="12" cy="16.2" r="1.05" fill="currentColor"/></svg>',
    processing:
        '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M12 4.5a7.5 7.5 0 1 1-7.4 6.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="M12 4.5v3.6M4.6 10.7h3.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
    interrupted:
        '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M7 4.5h10M7 19.5h10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="M8.6 4.5c0 4 6.8 5.4 6.8 9.4M15.4 4.5c0 4-6.8 5.4-6.8 9.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><path d="M9.4 19.5c0-1.5.9-2.4 2.6-3.6 1.7 1.2 2.6 2.1 2.6 3.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
}

/** 空态图标：与列表内容同层，只做视觉提示（aria-hidden）。 */
const EMPTY_ICON =
    '<svg viewBox="0 0 48 48" width="48" height="48" aria-hidden="true"><rect x="9" y="12" width="30" height="26" rx="4" fill="none" stroke="currentColor" stroke-width="2"/><path d="M9 20h30" fill="none" stroke="currentColor" stroke-width="2"/><path d="M24 25v7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="m20.6 28.6 3.4 3.4 3.4-3.4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>'

export default defineComponent({
    name: 'MobileTaskList',
    props: {
        tasks: {
            type: Array as PropType<TaskRecord[]>,
            required: true,
        },
        selectedRowKeys: {
            type: Array as PropType<string[]>,
            required: true,
        },
        isAndroid: {
            type: Boolean,
            default: false,
        },
    },
    emits: ['update:selectedRowKeys', 'action'],
    setup(props, { emit }) {
        // 进度百分比
        function progressPercent(task: TaskRecord): number {
            if (task.status === 'completed' || task.status === 'processing')
                return 100
            if (task.fileSize > 0) {
                return Math.min(
                    100,
                    Math.round((task.downloaded / task.fileSize) * 100),
                )
            }
            return 0
        }

        // 状态文案与容器色角色；文案与改造前一致。
        const statusMap: Record<string, { tone: string; label: string }> = {
            waiting: { tone: 'info', label: '等待中' },
            downloading: { tone: 'info', label: '下载中' },
            paused: { tone: 'warning', label: '暂停' },
            completed: { tone: 'success', label: '已完成' },
            error: { tone: 'error', label: '错误' },
            processing: { tone: 'info', label: '处理中' },
            interrupted: { tone: 'warning', label: '已中断' },
        }

        // 操作按钮 VNode 数组（行为完全沿用 TaskRowActions）
        function actionNodes(task: TaskRecord) {
            return renderActions(task, {
                emit: (
                    action: TaskAction,
                    taskId: string,
                    extra?: TaskActionExtra,
                ) => {
                    emit('action', action, taskId, extra)
                },
                isAndroid: props.isAndroid,
            })
        }

        // 处理多选切换
        function toggleSelection(taskId: string, checked: boolean) {
            const newKeys = new Set(props.selectedRowKeys)
            if (checked) {
                newKeys.add(taskId)
            } else {
                newKeys.delete(taskId)
            }
            emit('update:selectedRowKeys', Array.from(newKeys))
        }

        /** 状态 chip：图标 + 文字，容器色成对使用保证 4.5:1 以上对比度。 */
        function renderStatusChip(task: TaskRecord) {
            const status = statusMap[task.status] || {
                tone: 'info',
                label: task.status,
            }
            return h('span', { class: ['task-chip', `is-${status.tone}`] }, [
                h('span', {
                    class: 'task-chip-icon',
                    innerHTML: STATUS_ICONS[task.status] || '',
                }),
                h('span', null, status.label),
            ])
        }

        /** 音质标签：中性描边 chip，避免与状态色抢注意力。 */
        function renderQualityChip(task: TaskRecord) {
            if (!task.quality) return null
            return h(
                'span',
                { class: ['task-chip', 'is-outline'] },
                task.quality,
            )
        }

        /** 进度区：进度条 + 百分比文字（进度不只用颜色/长度表达）。 */
        function renderProgress(task: TaskRecord) {
            const percent = progressPercent(task)
            const speedText =
                task.status === 'downloading' && task.speed && task.speed > 0
                    ? formatSpeed(task.speed)
                    : ''
            return h('div', { class: 'task-row-progress' }, [
                h(
                    'div',
                    {
                        class: 'task-row-progress-bar',
                        role: 'progressbar',
                        'aria-valuenow': percent,
                        'aria-valuemin': 0,
                        'aria-valuemax': 100,
                        'aria-label': '下载进度',
                    },
                    [
                        h(NProgress, {
                            percentage: percent,
                            height: 8,
                            showIndicator: false,
                            borderRadius: 4,
                        }),
                    ],
                ),
                h(
                    'span',
                    { class: 'task-row-progress-text' },
                    speedText ? `${percent}% · ${speedText}` : `${percent}%`,
                ),
            ])
        }

        /** 失败/中断：图标 + 明确原因，不能只靠颜色。 */
        function renderStatusMessage(task: TaskRecord) {
            const isError = task.status === 'error'
            return h(
                'div',
                {
                    class: [
                        'task-row-message',
                        isError ? 'is-error' : 'is-interrupted',
                    ],
                    role: 'status',
                },
                [
                    h('span', {
                        class: 'task-chip-icon',
                        innerHTML:
                            STATUS_ICONS[isError ? 'error' : 'interrupted'],
                    }),
                    h(
                        'span',
                        { class: 'task-row-message-text' },
                        isError
                            ? task.errorMsg || '下载失败'
                            : '上次运行中断，等待恢复',
                    ),
                ],
            )
        }

        // 构建单个任务列表项（list item，而非把每行都包成卡片）
        function renderTaskRow(task: TaskRecord) {
            const checked = props.selectedRowKeys.includes(task.id)
            const disabled = task.status === 'downloading'
            const qualityChip = renderQualityChip(task)

            // 操作按钮：无可用操作（处理中）时不渲染空容器
            const nodes = actionNodes(task)

            const children: any[] = [
                // 头部：复选框 + 歌曲信息 + 操作（操作并入标题行，一屏能看到更多任务）
                h('div', { class: 'task-row-head' }, [
                    h(NCheckbox, {
                        checked,
                        disabled,
                        'aria-label': `选择任务：${task.songTitle || '未知歌曲'}`,
                        'onUpdate:checked': (val: boolean) =>
                            toggleSelection(task.id, val),
                    }),
                    h('div', { class: 'task-row-text' }, [
                        h(
                            'div',
                            {
                                class: 'task-row-title',
                                title: task.songTitle || '未知歌曲',
                            },
                            task.songTitle || '未知歌曲',
                        ),
                        // 副标题行：歌手 + 音质 chip 同行，避免为单个 chip 多占一行
                        h(
                            'div',
                            { class: 'task-row-sub' },
                            [
                                h(
                                    'span',
                                    { class: 'task-row-artist' },
                                    task.artist || '未知歌手',
                                ),
                                qualityChip,
                            ].filter((node) => node !== null),
                        ),
                    ]),
                    nodes.length > 0
                        ? h('div', { class: 'task-row-actions' }, nodes)
                        : null,
                ]),

                // 状态行：状态 chip + 进度或失败原因同行；状态不只靠颜色，且带数值文字。
                h('div', { class: 'task-row-status' }, [
                    renderStatusChip(task),
                    task.status === 'error' || task.status === 'interrupted'
                        ? renderStatusMessage(task)
                        : renderProgress(task),
                ]),
            ]

            // 仅安卓/Web 运行时显示文件路径；没有路径的行不渲染占位符
            if (props.isAndroid && task.filePath) {
                children.push(
                    h('div', { class: 'task-row-filepath' }, [
                        h(
                            NEllipsis,
                            {
                                style: {
                                    fontSize: 'var(--md-body-small)',
                                    maxWidth: '100%',
                                },
                                expandTrigger: 'click',
                                lineClamp: 1,
                                tooltip: false,
                            },
                            () => task.filePath,
                        ),
                    ]),
                )
            }

            return h(
                'div',
                {
                    class: ['task-row', { 'is-selected': checked }],
                    key: task.id,
                },
                children.filter((child) => child !== null),
            )
        }

        /** 空态：图标 + 文案（原文案不变），不再是纯文本居中。 */
        function renderEmpty() {
            return h('div', { class: 'task-empty', role: 'status' }, [
                h('span', {
                    class: 'task-empty-icon',
                    innerHTML: EMPTY_ICON,
                }),
                h('p', { class: 'task-empty-text' }, '暂无任务'),
            ])
        }

        // 返回渲染函数
        return () => {
            if (props.tasks.length === 0) {
                return renderEmpty()
            }
            return h(
                'div',
                { class: 'task-row-list' },
                props.tasks.map((task) => renderTaskRow(task)),
            )
        }
    },
})
</script>

<style scoped>
/* 列表整体是一张 surface（M3 list），行之间用分隔线，而不是每行一张卡片。 */
.task-row-list {
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
    background-color: var(--md-surface-container-low);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-lg);
}

.task-row {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-2);
    min-width: 0;
    /* 行内 4dp 节奏：纵向收紧到 8dp，一屏可容纳 4 行以上任务 */
    padding: var(--md-space-2) var(--md-space-4);
    border-left: 3px solid transparent;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.task-row + .task-row {
    border-top: 1px solid var(--md-outline-variant);
}

/* 选中态：secondaryContainer 状态层 + 主色指示条，键盘/触屏都能看出选中。 */
.task-row.is-selected {
    background-color: var(--md-secondary-container);
    border-left-color: var(--md-primary);
}

.task-row-head {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
    min-width: 0;
}

.task-row-head :deep(.n-checkbox) {
    flex-shrink: 0;
    /* 勾选是行内唯一的纯图标型控件，命中区补足到 48x48 */
    justify-content: center;
    min-width: var(--md-target-min);
    min-height: var(--md-target-min);
}

.task-row-text {
    flex: 1;
    min-width: 0;
}

.task-row-title {
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    /* 单行省略；不使用固定高度，放大字号不会裁切 */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.task-row-artist {
    flex: 1;
    min-width: 0;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* 副标题行：歌手占满剩余宽度，音质 chip 靠右且不被压缩 */
.task-row-sub {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: var(--md-space-2);
    min-width: 0;
}

/* ---------- chip ---------- */
.task-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--md-space-1);
    flex-shrink: 0;
    padding: 2px var(--md-space-2);
    border: 1px solid transparent;
    border-radius: var(--md-shape-sm);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    font-weight: var(--md-weight-medium);
    white-space: nowrap;
}

.task-chip-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
}

.task-chip.is-info {
    background-color: var(--md-tertiary-container);
    color: var(--md-on-tertiary-container);
}

.task-chip.is-warning {
    background-color: var(--md-warning-container);
    color: var(--md-on-warning-container);
}

.task-chip.is-success {
    background-color: var(--md-success-container);
    color: var(--md-on-success-container);
}

.task-chip.is-error {
    background-color: var(--md-error-container);
    color: var(--md-on-error-container);
}

.task-chip.is-outline {
    border-color: var(--md-outline-variant);
    color: var(--md-on-surface-variant);
}

/* ---------- 进度 ---------- */
.task-row-progress {
    display: flex;
    align-items: center;
    gap: var(--md-space-3);
    min-width: 0;
}

.task-row-progress-bar {
    flex: 1;
    min-width: 0;
}

.task-row-progress-text {
    flex-shrink: 0;
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    font-variant-numeric: tabular-nums;
}

/* ---------- 终止状态说明 ---------- */
.task-row-message {
    display: flex;
    align-items: flex-start;
    gap: var(--md-space-2);
    padding: var(--md-space-2) var(--md-space-3);
    border-radius: var(--md-shape-sm);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
}

.task-row-message.is-error {
    background-color: var(--md-error-container);
    color: var(--md-on-error-container);
}

.task-row-message.is-interrupted {
    background-color: var(--md-warning-container);
    color: var(--md-on-warning-container);
}

.task-row-message-text {
    min-width: 0;
    overflow-wrap: anywhere;
}

/* ---------- 文件路径 ---------- */
.task-row-filepath {
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
}

/* ---------- 状态行 ---------- */
/* 状态 chip 与进度/原因同行：状态表达只有一处，且不只靠颜色。 */
.task-row-status {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
    min-width: 0;
}

.task-row-status .task-row-progress {
    flex: 1;
    min-width: 0;
}

.task-row-status .task-row-message {
    flex: 1;
    min-width: 0;
}

/* ---------- 操作 ---------- */
/* 操作并入标题行右侧：不再单独占一行，行高由 3 行降到 2 行。 */
.task-row-actions {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    gap: var(--md-space-1);
}

/* 触控目标：行内按钮不小于 48dp，且给 2 字标签留出按钮宽度（避免被压成圆形）。 */
.task-row-actions :deep(.n-button) {
    min-height: var(--md-target-min);
    min-width: 72px;
    padding-left: var(--md-space-4);
    padding-right: var(--md-space-4);
}

/* 「更多」菜单触发器是纯图标圆形按钮：保持 48x48，不被上面的最小宽度/内边距撑开 */
.task-row-actions :deep(.n-button[aria-haspopup='menu']) {
    min-width: var(--md-target-min);
    padding-left: 0;
    padding-right: 0;
}

/* ---------- 空态 ---------- */
.task-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--md-space-3);
    padding: var(--md-space-10) var(--md-space-4);
    min-width: 0;
}

.task-empty-icon {
    display: inline-flex;
    color: var(--md-on-surface-variant);
}

.task-empty-text {
    margin: 0;
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface-variant);
}

/* 极窄屏兜底：状态行允许换行，进度/原因占满整行，避免被挤成不可读的窄条 */
@media (max-width: 359px) {
    .task-row-status {
        flex-wrap: wrap;
    }

    .task-row-status .task-row-progress,
    .task-row-status .task-row-message {
        flex: 1 1 100%;
    }
}
</style>

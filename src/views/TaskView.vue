<template>
    <div class="task-view">
        <TaskTabs v-model:activeTab="activeTab" :counts="tabCounts" />

        <!-- 批量操作栏：按当前标签页显示可用的一键操作 -->
        <section v-if="showToolbar" class="task-toolbar" aria-label="批量操作">
            <!-- 中断恢复与错误重试按任务状态分别选择，实际规则交给 Rust。 -->
            <n-button
                v-if="showRetryAll"
                class="task-toolbar-primary"
                type="primary"
                :loading="retryingAll"
                :disabled="retryingAll"
                @click="handleRetryAll"
            >
                {{
                    activeTab === 'interrupted'
                        ? '恢复全部中断任务'
                        : '全部重试'
                }}
                （{{
                    activeTab === 'interrupted'
                        ? tabCounts.interrupted
                        : tabCounts.error
                }}）
            </n-button>
            <p v-if="showRetryAll" class="task-toolbar-hint">
                会依次重新入队，实际同时下载数量由“最大并发数”决定
            </p>

            <!-- 清除类动作不可逆，窄屏默认收纳在展开区里，避免误触 -->
            <div
                v-show="clearActionsVisible"
                id="task-toolbar-clear"
                class="task-toolbar-clear"
            >
                <!-- 清除所有已下载（已完成）的任务记录 -->
                <n-popconfirm
                    v-if="canClearCompleted"
                    :style="{ maxWidth: 'calc(100vw - 32px)' }"
                    @positive-click="handleClearCompleted"
                >
                    <template #trigger>
                        <n-button
                            type="warning"
                            secondary
                            :loading="clearing"
                            :disabled="clearing"
                        >
                            清除所有已下载的任务（{{ tabCounts.completed }}）
                        </n-button>
                    </template>
                    <n-space vertical :size="8" class="task-toolbar-confirm">
                        <span
                            >确定清除
                            {{ tabCounts.completed }}
                            个已下载的任务记录吗？</span
                        >
                        <n-checkbox v-model:checked="deleteFileForCompleted">
                            同时删除磁盘上已下载的文件（不可恢复）
                        </n-checkbox>
                    </n-space>
                </n-popconfirm>

                <!-- 清除所有历史任务（含进行中的任务，会被取消） -->
                <n-popconfirm
                    v-if="canClearAll"
                    :style="{ maxWidth: 'calc(100vw - 32px)' }"
                    @positive-click="handleClearAll"
                >
                    <template #trigger>
                        <n-button
                            type="error"
                            secondary
                            :loading="clearing"
                            :disabled="clearing"
                        >
                            清除所有历史任务（{{ tabCounts.total }}）
                        </n-button>
                    </template>
                    <n-space vertical :size="8" class="task-toolbar-confirm">
                        <span
                            >确定清除全部
                            {{ tabCounts.total }} 个任务记录吗？</span
                        >
                        <span
                            v-if="activeTaskCount > 0"
                            class="task-toolbar-warn"
                        >
                            其中
                            {{ activeTaskCount }}
                            个任务正在进行（等待/下载/暂停/处理中），会被一并取消。
                        </span>
                        <n-checkbox v-model:checked="deleteFileForAll">
                            同时删除磁盘上的文件（不可恢复）
                        </n-checkbox>
                    </n-space>
                </n-popconfirm>
            </div>

            <!-- 窄屏渐进披露：清除动作先收纳，点击后展开 -->
            <n-button
                v-if="isNarrow && hasClearActions"
                class="task-toolbar-disclosure"
                quaternary
                :aria-expanded="clearActionsOpen"
                aria-controls="task-toolbar-clear"
                @click="clearActionsOpen = !clearActionsOpen"
            >
                {{ clearActionsOpen ? '收起清除操作' : '更多清除操作' }}
                <span
                    class="task-toolbar-chevron"
                    :class="{ 'is-open': clearActionsOpen }"
                    aria-hidden="true"
                    v-html="CHEVRON_ICON"
                />
            </n-button>
        </section>

        <TaskTable
            :tasks="pagedTasks"
            :selectedRowKeys="selectedRowKeys"
            @update:selectedRowKeys="selectedRowKeys = $event"
            @action="handleAction"
        />

        <!-- 任务数量可能很大，只渲染当前页，避免一次性创建成千上万个 DOM/组件导致卡死 -->
        <div v-if="filteredTasks.length > pageSize" class="task-pagination">
            <n-pagination
                v-model:page="page"
                :page-size="pageSize"
                :item-count="filteredTasks.length"
                :page-slot="5"
            />
        </div>

        <TaskBatchActions
            :selectedCount="selectedRowKeys.length"
            @clear="handleBatchClear"
        />
    </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { openFileLocation } from '../api/fileApi'
import {
    NPagination,
    NButton,
    NPopconfirm,
    NCheckbox,
    NSpace,
    useNotification,
} from 'naive-ui'
import { useTaskStore } from '../stores/taskStore'
import { useSettingsStore } from '../stores/settingsStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useNarrowLayout } from '../composables/useNarrowLayout'
import TaskTabs from '../components/task/TaskTabs.vue'
import TaskTable from '../components/task/TaskTable.vue'
import TaskBatchActions from '../components/task/TaskBatchActions.vue'
import type {
    TaskAction,
    TaskActionExtra,
} from '../components/task/TaskRowActions'

const taskStore = useTaskStore()
const settingsStore = useSettingsStore()
const { retryTask } = useDownloadActions()
const notification = useNotification()

const activeTab = ref('all')
const selectedRowKeys = ref<string[]>([])
const retryingAll = ref(false)
const clearing = ref(false)
// 两个清除操作各自的“同时删除文件”勾选项，默认不勾选，避免误删磁盘文件
const deleteFileForCompleted = ref(false)
const deleteFileForAll = ref(false)

// 分页：任务列表可能包含上千条记录（尤其是“全部/已完成”），
// 一次渲染全部任务会创建大量组件实例并频繁重渲染，是崩溃与卡顿的主因之一。
const page = ref(1)
const pageSize = ref(50)

const tabCounts = computed(() => {
    const counts = {
        total: 0,
        waiting: 0,
        downloading: 0,
        paused: 0,
        completed: 0,
        interrupted: 0,
        error: 0,
    }
    for (const task of taskStore.tasks) {
        counts.total++
        if (task.status === 'waiting') counts.waiting++
        else if (task.status === 'downloading') counts.downloading++
        else if (task.status === 'paused') counts.paused++
        else if (task.status === 'completed') counts.completed++
        else if (task.status === 'interrupted') counts.interrupted++
        else if (task.status === 'error') counts.error++
    }
    return counts
})

const filteredTasks = computed(() => {
    const tab = activeTab.value
    return taskStore.tasks.filter((task) => {
        if (tab === 'all') return true
        return task.status === tab
    })
})

const pagedTasks = computed(() => {
    const start = (page.value - 1) * pageSize.value
    return filteredTasks.value.slice(start, start + pageSize.value)
})

/** 进行中的任务（清空全部历史时会被取消） */
const ACTIVE_STATUSES: string[] = [
    'waiting',
    'downloading',
    'paused',
    'processing',
]
const activeTaskCount = computed(
    () =>
        taskStore.tasks.filter((t) => ACTIVE_STATUSES.includes(t.status))
            .length,
)

/** “全部 / 已完成”标签页提供批量清除入口 */
const inClearableTab = computed(
    () => activeTab.value === 'all' || activeTab.value === 'completed',
)
const canClearCompleted = computed(
    () => inClearableTab.value && tabCounts.value.completed > 0,
)
const canClearAll = computed(
    () => inClearableTab.value && tabCounts.value.total > 0,
)

const showToolbar = computed(
    () =>
        (activeTab.value === 'error' && tabCounts.value.error > 0) ||
        (activeTab.value === 'interrupted' &&
            tabCounts.value.interrupted > 0) ||
        canClearCompleted.value ||
        canClearAll.value,
)

/** 是否显示“全部重试/恢复全部中断任务” */
const showRetryAll = computed(
    () =>
        (activeTab.value === 'error' && tabCounts.value.error > 0) ||
        (activeTab.value === 'interrupted' && tabCounts.value.interrupted > 0),
)

/** 是否存在可用的清除动作 */
const hasClearActions = computed(
    () => canClearCompleted.value || canClearAll.value,
)

/** 窄屏使用渐进披露，宽屏一次性展示全部批量操作 */
const isNarrow = useNarrowLayout()
const clearActionsOpen = ref(false)
const clearActionsVisible = computed(
    () => !isNarrow.value || clearActionsOpen.value,
)

/** 展开箭头为纯装饰，展开状态同时由按钮文字表达 */
const CHEVRON_ICON =
    '<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="m7 10 5 5 5-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>'

// 切换标签页时回到第一页，并收起清除操作展开区
watch(activeTab, () => {
    page.value = 1
    clearActionsOpen.value = false
})

// 任务被删除或筛选结果变少时，纠正越界页码，避免停留在空白页
watch(
    () => filteredTasks.value.length,
    (len) => {
        const maxPage = Math.max(1, Math.ceil(len / pageSize.value))
        if (page.value > maxPage) page.value = maxPage
    },
)

async function handleAction(
    action: TaskAction,
    taskId: string,
    extra?: TaskActionExtra,
) {
    try {
        // 操作统一交给 Rust 命令；列表变化由 task-updated/task-removed 事件回填。
        switch (action) {
            case 'cancel':
                await taskStore.cancelTask(taskId, extra?.deleteFile === true)
                break
            case 'pause':
                await taskStore.pauseTask(taskId)
                break
            case 'resume':
                // 中断任务恢复会重新读取当前下载设置；先完成待写入的设置变更。
                if (
                    taskStore.tasks.find((task) => task.id === taskId)
                        ?.status === 'interrupted'
                ) {
                    await settingsStore.flushSettings()
                }
                await taskStore.resumeTask(taskId)
                break
            case 'retry':
                await retryTask(taskId)
                break
            case 'remove': {
                const result = await taskStore.removeTask(
                    taskId,
                    extra?.deleteFile === true,
                )
                if (result.failed) {
                    throw new Error(result.errors.join('；'))
                }
                break
            }
            case 'open-location': {
                const task = taskStore.tasks.find((t) => t.id === taskId)
                if (task?.filePath) {
                    try {
                        await openFileLocation(task.filePath)
                    } catch (e) {
                        console.error('打开文件位置失败:', e)
                    }
                }
                break
            }
        }
        // 命令成功后才清除选中状态；失败时保留以便用户重试。
        selectedRowKeys.value = selectedRowKeys.value.filter(
            (id) => id !== taskId,
        )
    } catch (e: any) {
        notification.error({
            title: '操作失败',
            description: e?.message || String(e),
            duration: 4000,
        })
    }
}

async function handleBatchClear(deleteFile: boolean) {
    const ids = selectedRowKeys.value.slice()
    if (ids.length === 0) {
        return
    }
    try {
        // 批量命令由后端逐个删除并返回准确计数，前端不直接修改持久化记录。
        const result = await taskStore.removeTasks(ids, deleteFile)
        if (result.failed > 0) {
            notification.warning({
                title: '部分任务未清除',
                description: result.errors.join('；').slice(0, 200),
                duration: 4000,
            })
        }
        // 在批量删除流程完成后再清空选中键，避免删除过程中选中状态提前丢失。
        selectedRowKeys.value = []
    } catch (e: any) {
        notification.error({
            title: '清除任务失败',
            description: e?.message || String(e),
            duration: 4000,
        })
    }
}

/** 仅处理当前标签的任务；中断任务由用户明确发起恢复。 */
async function handleRetryAll() {
    if (retryingAll.value) return
    const targetStatus =
        activeTab.value === 'interrupted' ? 'interrupted' : 'error'
    const ids = taskStore.tasks
        .filter((task) => task.status === targetStatus)
        .map((t) => t.id)
    if (ids.length === 0) return

    retryingAll.value = true
    try {
        // 批量重试读取同一份当前设置，先完成防抖写盘。
        await settingsStore.flushSettings()
        const { succeeded, failed } =
            targetStatus === 'interrupted'
                ? await taskStore.resumeTasks(ids)
                : await taskStore.retryTasks(ids)
        notification.success({
            title: targetStatus === 'interrupted' ? '批量恢复' : '批量重试',
            description: `已重新入队 ${succeeded} 个任务${failed > 0 ? `，${failed} 个未能入队，请查看任务状态` : ''}`,
            duration: 4000,
        })
    } catch (e: any) {
        console.error('批量重试失败:', e)
        notification.error({
            title: '批量重试失败',
            description: e?.message || String(e),
            duration: 4000,
        })
    } finally {
        retryingAll.value = false
    }
}

/** 清除所有“已下载（已完成）”的任务记录，可选同时删除磁盘文件 */
async function handleClearCompleted() {
    if (clearing.value) return
    const deleteFile = deleteFileForCompleted.value
    deleteFileForCompleted.value = false

    const ids = taskStore.tasks
        .filter((t) => t.status === 'completed')
        .map((t) => t.id)
    if (ids.length === 0) return

    selectedRowKeys.value = []
    clearing.value = true
    try {
        // 成功通知使用后端真实成功数
        const result = await taskStore.removeTasks(ids, deleteFile)
        const fileMessage = deleteFile ? '，并删除对应文件' : ''
        const failedMessage =
            result.failed > 0 ? `，${result.failed} 个失败` : ''
        notification.success({
            title: '已清除',
            description: `已清除 ${result.succeeded} 个已下载的任务记录${fileMessage}${failedMessage}`,
            duration: 4000,
        })
    } finally {
        clearing.value = false
    }
}

/** 清除所有历史任务（含等待/下载/暂停/处理中的任务，会被一并取消） */
async function handleClearAll() {
    if (clearing.value) return
    const deleteFile = deleteFileForAll.value
    deleteFileForAll.value = false

    const ids = taskStore.tasks.map((t) => t.id)
    if (ids.length === 0) return

    selectedRowKeys.value = []
    clearing.value = true
    try {
        // 成功通知使用后端真实成功数
        const result = await taskStore.removeTasks(ids, deleteFile)
        const fileMessage = deleteFile ? '，并删除对应文件' : ''
        const failedMessage =
            result.failed > 0 ? `，${result.failed} 个失败` : ''
        notification.success({
            title: '已清除',
            description: `已清除全部 ${result.succeeded} 个任务记录${fileMessage}${failedMessage}`,
            duration: 4000,
        })
    } finally {
        clearing.value = false
    }
}
</script>

<style scoped>
/* 页面按 4dp 网格纵向排列，容器的宽度由 .main-content 控制 */
.task-view {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
    min-height: 100%;
    min-width: 0;
}

.task-pagination {
    display: flex;
    justify-content: center;
    padding-top: var(--md-space-1);
    flex-shrink: 0;
    min-width: 0;
    overflow-x: auto;
}

/* 批量操作区：一块 surface container，主操作最醒目 */
.task-toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--md-space-2) var(--md-space-3);
    padding: var(--md-space-4);
    min-width: 0;
    background: var(--md-surface-container-low);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-lg);
    flex-shrink: 0;
}

.task-toolbar-primary {
    flex-shrink: 0;
}

.task-toolbar-hint {
    flex: 1 1 200px;
    margin: 0;
    min-width: 0;
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    overflow-wrap: anywhere;
}

.task-toolbar-clear {
    display: flex;
    flex-wrap: wrap;
    gap: var(--md-space-2);
    min-width: 0;
}

.task-toolbar-disclosure {
    flex-shrink: 0;
}

.task-toolbar-chevron {
    display: inline-flex;
    margin-left: var(--md-space-1);
    transition: transform 0.15s ease;
}

.task-toolbar-chevron.is-open {
    transform: rotate(180deg);
}

.task-toolbar-confirm {
    max-width: 320px;
}

.task-toolbar-warn {
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-warning);
}

/* 紧凑布局：主操作整行，清除动作整行堆叠，触控高度由全局 48px 规则保证 */
@media (max-width: 767px) {
    .task-toolbar {
        align-items: stretch;
    }

    .task-toolbar-primary,
    .task-toolbar-disclosure,
    .task-toolbar-clear {
        flex: 1 1 100%;
    }

    .task-toolbar-clear {
        flex-direction: column;
    }

    .task-toolbar-clear > :deep(.n-button),
    .task-toolbar-primary,
    .task-toolbar-disclosure {
        width: 100%;
    }

    .task-toolbar-hint {
        flex: 1 1 100%;
    }

    .task-toolbar-disclosure {
        justify-content: flex-start;
    }
}
</style>

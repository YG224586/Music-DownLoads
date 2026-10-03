import {
    computed,
    defineComponent,
    h,
    ref,
    type PropType,
    type VNode,
} from 'vue'
import { NButton, NCheckbox, NPopover, useDialog } from 'naive-ui'
import type { TaskRecord } from '../../types'

export type TaskAction =
    'cancel' | 'pause' | 'resume' | 'retry' | 'remove' | 'open-location'

export interface TaskActionExtra {
    deleteFile?: boolean
}

export type TaskActionEmitter = (
    action: TaskAction,
    taskId: string,
    extra?: TaskActionExtra,
) => void

export interface TaskActionContext {
    emit: TaskActionEmitter
    isAndroid?: boolean
}

/** ⋯ 图标：纯装饰，控件的可访问名由 aria-label 提供 */
const MORE_ICON =
    '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="12" cy="12" r="1.7" /><circle cx="5.5" cy="12" r="1.7" /><circle cx="18.5" cy="12" r="1.7" /></svg>'

const MENU_STYLE: Record<string, string> = {
    display: 'flex',
    flexDirection: 'column',
    minWidth: '180px',
    padding: 'var(--md-space-1)',
    borderRadius: 'var(--md-shape-xs)',
    backgroundColor: 'var(--md-surface-container)',
    boxShadow: 'var(--md-elevation-2)',
}

const MENU_ITEM_STYLE: Record<string, string> = {
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--md-space-3)',
    width: '100%',
    minHeight: 'var(--md-target-min)',
    padding: '0 var(--md-space-3)',
    border: 'none',
    borderRadius: 'var(--md-shape-sm)',
    background: 'transparent',
    color: 'var(--md-error)',
    font: 'inherit',
    fontSize: 'var(--md-label-large)',
    lineHeight: 'var(--md-label-large-line)',
    fontWeight: 'var(--md-weight-medium)',
    textAlign: 'left',
    cursor: 'pointer',
}

/**
 * 单行操作的「更多」菜单。
 *
 * 破坏性动作（取消任务 / 删除记录）不直接摆在行上：一方面避免误触，
 * 另一方面每行只保留一个 filled 主操作，视觉层级才清楚。
 * 确认环节与「同时删除文件」勾选项沿用原 NPopconfirm 的文案与默认值。
 */
const TaskRowMenu = defineComponent({
    name: 'TaskRowMenu',
    props: {
        task: {
            type: Object as PropType<TaskRecord>,
            required: true,
        },
        act: {
            type: Function as PropType<TaskActionEmitter>,
            required: true,
        },
    },
    setup(props) {
        const show = ref(false)
        const dialog = useDialog()
        const deleteFile = ref(false)

        // 已完成 / 失败 / 中断：危险动作是「删除记录」；其余状态是「取消任务」
        const removeFlow = computed(() => {
            const status = props.task.status
            return (
                status === 'completed' ||
                status === 'error' ||
                status === 'interrupted'
            )
        })

        const dangerLabel = computed(() =>
            removeFlow.value ? '删除任务' : '取消任务',
        )

        const confirmTitle = computed(() =>
            removeFlow.value ? '确定删除该任务记录吗？' : '确定取消该任务吗？',
        )

        const checkboxLabel = computed(() =>
            removeFlow.value && props.task.status === 'completed'
                ? '同时删除已下载的文件'
                : '同时删除未下载完成的文件',
        )

        const reset = () => {
            deleteFile.value = false
        }

        const handleConfirm = () => {
            const deleteFileValue = deleteFile.value
            show.value = false
            reset()
            props.act(removeFlow.value ? 'remove' : 'cancel', props.task.id, {
                deleteFile: deleteFileValue,
            })
        }

        const openConfirm = () => {
            deleteFile.value = false
            dialog.warning({
                title: confirmTitle.value,
                content: () =>
                    h(
                        NCheckbox,
                        {
                            checked: deleteFile.value,
                            'onUpdate:checked': (value: boolean) => {
                                deleteFile.value = value
                            },
                        },
                        { default: () => checkboxLabel.value },
                    ),
                positiveText: '确认',
                negativeText: '返回',
                showIcon: false,
                positiveButtonProps: { text: true, type: 'error' },
                negativeButtonProps: { text: true },
                style: { borderRadius: 'var(--md-shape-xl)' },
                onPositiveClick: () => handleConfirm(),
            })
        }

        return () =>
            h(
                NPopover,
                {
                    show: show.value,
                    'onUpdate:show': (value: boolean) => {
                        show.value = value
                        if (!value) reset()
                    },
                    trigger: 'click',
                    placement: 'bottom-end',
                    showArrow: false,
                    contentStyle: {
                        maxWidth: 'calc(100vw - 32px)',
                        padding: '0',
                        borderRadius: 'var(--md-shape-xs)',
                        backgroundColor: 'var(--md-surface-container)',
                        boxShadow: 'var(--md-elevation-2)',
                    },
                },
                {
                    trigger: () =>
                        h(
                            NButton,
                            {
                                quaternary: true,
                                circle: true,
                                size: 'small',
                                // 内联 min-* 覆盖宿主作用域的 min-width:72px（min-width 优先于 width），
                                // 保证圆形「更多」按钮始终是 48dp 触控目标而不是被拉成胶囊。
                                style: {
                                    width: 'var(--md-target-min)',
                                    height: 'var(--md-target-min)',
                                    minWidth: 'var(--md-target-min)',
                                    minHeight: 'var(--md-target-min)',
                                },
                                'aria-label': `更多操作：${props.task.songTitle || '未知歌曲'}`,
                                'aria-haspopup': 'menu',
                            },
                            {
                                icon: () =>
                                    h('span', {
                                        innerHTML: MORE_ICON,
                                        'aria-hidden': 'true',
                                    }),
                            },
                        ),
                    default: () =>
                        h('div', { style: MENU_STYLE, role: 'menu' }, [
                            h(
                                'button',
                                {
                                    type: 'button',
                                    class: 'task-menu-item is-danger',
                                    role: 'menuitem',
                                    style: MENU_ITEM_STYLE,
                                    onClick: () => {
                                        show.value = false
                                        openConfirm()
                                    },
                                },
                                dangerLabel.value,
                            ),
                        ]),
                },
            )
    },
})

function createActionButton(
    label: string,
    action: TaskAction,
    emit: TaskActionEmitter,
    taskId: string,
    tonal: boolean,
): VNode {
    return h(
        NButton,
        {
            size: 'small',
            type: 'primary',
            secondary: tonal,
            onClick: () => emit(action, taskId),
        },
        { default: () => label },
    )
}

/**
 * 返回一行的操作节点。
 *
 * 层级规则（MD3 Expressive，手机优先）：
 * - 每行最多一个 filled 主操作，即当前状态最需要的那个动作；
 * - 次要动作使用 tonal（secondary）样式；
 * - 破坏性动作（取消 / 删除）收进「更多」菜单，并保留原有的确认与
 *   「同时删除文件」勾选项；
 * - 不再使用 type="warning" 的实心按钮（原本每行会出现两块深色实心色块）。
 *
 * TaskAction 语义与文案保持不变，父组件的数据流无需调整。
 */
export function renderActions(
    task: TaskRecord,
    context: TaskActionContext,
): VNode[] {
    const { emit, isAndroid = false } = context
    const taskId = task.id
    const nodes: VNode[] = []

    switch (task.status) {
        case 'processing':
            // 处理中不给操作：避免中断元数据写入或歌词处理
            return nodes
        case 'downloading':
            nodes.push(createActionButton('暂停', 'pause', emit, taskId, false))
            break
        case 'paused':
            nodes.push(
                createActionButton('恢复', 'resume', emit, taskId, false),
            )
            break
        case 'error':
            nodes.push(createActionButton('重试', 'retry', emit, taskId, false))
            break
        case 'interrupted':
            nodes.push(
                createActionButton('恢复', 'resume', emit, taskId, false),
            )
            break
        case 'completed':
            // 桌面端才提供「打开文件位置」，Android 上没有可用的文件管理器入口
            if (!isAndroid) {
                nodes.push(
                    createActionButton(
                        '打开文件位置',
                        'open-location',
                        emit,
                        taskId,
                        true,
                    ),
                )
            }
            break
        default:
            // waiting：只剩「取消」这一个破坏性动作，直接进「更多」菜单
            break
    }

    nodes.push(h(TaskRowMenu, { task, act: emit }))
    return nodes
}

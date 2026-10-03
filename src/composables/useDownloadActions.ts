import { h, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useDialog, useNotification, NButton } from 'naive-ui'
import type { Quality, SongInfo, QualityItem, DuplicateAction } from '../types'
import { useSettingsStore } from '../stores/settingsStore'
import { useTaskStore } from '../stores/taskStore'
import QualitySelector from '../components/search/QualitySelector.vue'

/** 只处理用户交互：音质选择、重名确认及通知。品质降级、路径与重试由 Rust 决定。 */
export function useDownloadActions() {
    const dialog = useDialog()
    const router = useRouter()
    const notification = useNotification()
    const settingsStore = useSettingsStore()
    const taskStore = useTaskStore()

    /** 只收集用户选择的品质标签，不在页面上做降级或文件名推断。 */
    function askQuality(qualities: QualityItem[]): Promise<string> {
        return new Promise((resolve, reject) => {
            const compRef = ref<InstanceType<typeof QualitySelector>>()
            const d = dialog.create({
                title: '选择下载音质',
                // M3：对话框不画前置图标（Naive 默认 showIcon 会画实心圆 info 图标）。
                showIcon: false,
                content: () => h(QualitySelector, { qualities, ref: compRef }),
                positiveText: '确定',
                negativeText: '取消',
                // M3：对话框动作是文字按钮；Naive 默认「确定」是实心主色、取消是 ghost 描边药丸。
                positiveButtonProps: { text: true },
                negativeButtonProps: { text: true },
                onPositiveClick: () => {
                    const val = compRef.value?.selected
                    if (val) {
                        resolve(val)
                    } else {
                        reject(new Error('未选择品质'))
                    }
                    d.destroy()
                },
                onNegativeClick: () => {
                    reject(new Error('用户取消'))
                    d.destroy()
                },
                onClose: () => reject(new Error('用户取消')),
            })
        })
    }

    /** Rust 返回重名确认请求后，再向用户询问覆盖、保留两份或取消。 */
    function askDuplicateAction(songTitle: string): Promise<DuplicateAction> {
        return new Promise((resolve) => {
            const d = dialog.create({
                title: '文件已存在',
                showIcon: false,
                content: `歌曲“${songTitle}”在下载目录中已存在同名文件，请选择处理方式：`,
                // M3：对话框动作是文字按钮（text），不用实心/描边按钮。
                action: () => [
                    h(
                        NButton,
                        {
                            size: 'small',
                            text: true,
                            onClick: () => {
                                resolve('overwrite')
                                d.destroy()
                            },
                        },
                        { default: () => '覆盖' },
                    ),
                    h(
                        NButton,
                        {
                            size: 'small',
                            text: true,
                            type: 'primary',
                            onClick: () => {
                                resolve('rename')
                                d.destroy()
                            },
                        },
                        { default: () => '保留两份' },
                    ),
                    h(
                        NButton,
                        {
                            size: 'small',
                            text: true,
                            type: 'error',
                            onClick: () => {
                                resolve('cancel')
                                d.destroy()
                            },
                        },
                        { default: () => '取消' },
                    ),
                ],
                onClose: () => resolve('cancel'),
            })
        })
    }

    async function submitSong(
        song: SongInfo,
        quality: string,
    ): Promise<'created' | 'error' | 'cancelled'> {
        let result = await taskStore.createTask(song, quality)
        if (result.outcome === 'needs_confirmation') {
            // Rust 发现实际路径冲突后才弹窗；选择结果交回同一命令重新校验。
            const choice = await askDuplicateAction(result.song_title)
            result = await taskStore.createTask(song, quality, choice)
        }
        if (result.outcome === 'cancelled') {
            return 'cancelled'
        }
        if (result.outcome === 'needs_confirmation') {
            throw new Error('文件状态已变化，请重新添加任务')
        }
        if (result.task.status === 'error') {
            return 'error'
        }
        return 'created'
    }

    async function downloadSingle(
        song: SongInfo,
        forceQuality?: Quality,
    ): Promise<void> {
        try {
            let quality = forceQuality ?? settingsStore.settings.defaultQuality
            if (quality === 'ask') {
                try {
                    // 用户选择「每次询问」时弹出音质选择器，档位取自该歌曲实际可下载的列表。
                    quality = await askQuality(song.qualities)
                } catch {
                    return
                }
            }
            // 设置有防抖写盘；先刷入存储，保证 Rust 读取的是用户当前选择。
            await settingsStore.flushSettings()
            const outcome = await submitSong(song, quality)
            if (outcome === 'error') {
                notification.warning({
                    title: '下载提示',
                    description: `歌曲“${song.title}”所选音质不可用，已标记为错误`,
                })
            }
            if (outcome !== 'cancelled' && settingsStore.settings.jumpToTask) {
                router.push('/task')
            }
        } catch (error: any) {
            console.error('下载失败:', error)
            notification.error({
                title: '下载失败',
                description: error?.message || String(error),
            })
        }
    }

    async function batchDownload(songs: SongInfo[]): Promise<void> {
        try {
            let quality = settingsStore.settings.defaultQuality
            if (quality === 'ask') {
                // 批量操作只询问一次用户偏好的品质；每首歌是否可用由 Rust 逐一判断。
                const union = new Map<string, QualityItem>()
                for (const song of songs) {
                    // 合并所有歌曲的可选档位去重，作为批量下载的统一候选列表。
                    for (const item of song.qualities) {
                        union.set(item.quality, item)
                    }
                }
                if (union.size === 0) {
                    quality = ''
                } else {
                    try {
                        quality = await askQuality([...union.values()])
                    } catch {
                        return
                    }
                }
            }
            await settingsStore.flushSettings()
            let errors = 0
            for (const song of songs) {
                const outcome = await submitSong(song, quality)
                if (outcome === 'error') {
                    errors++
                }
            }
            if (errors > 0) {
                notification.warning({
                    title: '批量下载',
                    description: `${errors} 首歌曲无可用音质，已标记为错误`,
                })
            }
            if (settingsStore.settings.jumpToTask) {
                router.push('/task')
            }
        } catch (error: any) {
            console.error('批量下载失败:', error)
            notification.error({
                title: '批量下载失败',
                description: error?.message || String(error),
            })
        }
    }

    async function retryTask(taskId: string): Promise<void> {
        try {
            await settingsStore.flushSettings()
            // 错误重试由 Rust 决定计数、续传和降级；中断任务使用独立的 resume 命令。
            if (!(await taskStore.retryTask(taskId))) {
                notification.warning({
                    title: '重试失败',
                    description: '任务未重新入队，请查看任务状态',
                })
            }
        } catch (error: any) {
            notification.error({
                title: '重试失败',
                description: error?.message || String(error),
            })
        }
    }

    return { downloadSingle, batchDownload, retryTask }
}

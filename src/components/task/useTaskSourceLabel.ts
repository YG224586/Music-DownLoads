import { onMounted } from 'vue'
import { useScriptSourceStore } from '../../stores/scriptSourceStore'

/**
 * 音源已被删除（任务里的 script:<id> 找不到对应音源）时的兜底文案。
 * platformLabel 对未知 id 会原样返回 "script:12"，内部标识绝不能展示给用户。
 */
const DELETED_SOURCE_LABEL = '已删除的音源'

// 音源列表通常只在设置页加载过；任务页首屏补一次，否则已安装的音源会被误判成已删除。
let loadedOnce = false

/**
 * 任务域共用的音源来源标签：platform → 可读名称。
 * 放在 components/task 下（本域写入范围内），移动列表与桌面表格共用同一套兜底逻辑。
 */
export function useTaskSourceLabel() {
    const store = useScriptSourceStore()

    onMounted(() => {
        if (loadedOnce || store.sources.length > 0 || store.loading) return
        loadedOnce = true
        void store.load()
    })

    function sourceLabel(platform: string | undefined | null): string {
        if (!platform) return ''
        const label = store.platformLabel(platform)
        return label.startsWith('script:') ? DELETED_SOURCE_LABEL : label
    }

    return { sourceLabel }
}

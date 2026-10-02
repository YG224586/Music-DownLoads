import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { PLATFORMS } from '../config/platforms'
import type { QualityItem, SourceConfigState } from '../types'
import * as sourceConfigApi from '../api/sourceConfigApi'

/**
 * 音源配置（QingMusic music.json 导入）。
 * 状态为 null 时表示未加载或加载失败，前端回退到全部内置平台，
 * 保证搜索在无配置时依然可用。
 */
export const useSourceConfigStore = defineStore('sourceConfig', () => {
    const state = ref<SourceConfigState | null>(null)
    const loaded = ref(false)
    const importing = ref(false)
    const importError = ref<string | null>(null)

    async function load(): Promise<void> {
        try {
            state.value = await sourceConfigApi.loadSourceConfig()
        } catch {
            state.value = null
        } finally {
            loaded.value = true
        }
    }

    async function importUrl(url: string): Promise<void> {
        importing.value = true
        importError.value = null
        try {
            state.value = await sourceConfigApi.importSourceConfig(url)
        } catch (error) {
            importError.value =
                error instanceof Error ? error.message : String(error)
            throw error
        } finally {
            importing.value = false
        }
    }

    async function toggle(id: string, enabled: boolean): Promise<void> {
        state.value = await sourceConfigApi.setSourceEnabled(id, enabled)
    }

    /** 当前可用的平台 key 列表（按 PLATFORMS 顺序）。全部停用时回退默认，避免搜索不可用。 */
    const enabledPlatforms = computed<string[]>(() => {
        const keys = (state.value?.sources ?? [])
            .filter((s) => s.enabled && s.platform)
            .map((s) => s.platform as string)
        if (keys.length === 0) {
            return PLATFORMS.map((p) => p.key)
        }
        return PLATFORMS.filter((p) => keys.includes(p.key)).map((p) => p.key)
    })

    /** 某平台允许的音质列表；null 表示不限制。 */
    function allowedQualities(platform: string): string[] | null {
        const source = (state.value?.sources ?? []).find(
            (s) => s.platform === platform || s.id === platform,
        )
        if (!source || source.levels.length === 0) {
            return null
        }
        return source.levels
    }

    /** 过滤音质列表；过滤后为空则回退原始列表，避免选择器被清空。 */
    function filterQualities(
        platform: string,
        qualities: QualityItem[],
    ): QualityItem[] {
        const allowed = allowedQualities(platform)
        if (!allowed) {
            return qualities
        }
        const filtered = qualities.filter((q) => allowed.includes(q.quality))
        return filtered.length > 0 ? filtered : qualities
    }

    return {
        state,
        loaded,
        importing,
        importError,
        load,
        importUrl,
        toggle,
        enabledPlatforms,
        allowedQualities,
        filterQualities,
    }
})

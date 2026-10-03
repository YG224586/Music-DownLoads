import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import * as scriptSourceApi from '../api/scriptSourceApi'
import { PLATFORMS } from '../config/platforms'
import type { ScriptSourceItem, SearchResponse } from '../types'

/** 平台 id → 完整名称：内置音源来自 config/platforms.ts，未知形态原样返回。 */
const BUILTIN_LABELS: Record<string, string> = Object.fromEntries(
    PLATFORMS.map((item) => [item.key, item.name]),
)

function messageOf(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
}

export const useScriptSourceStore = defineStore('scriptSource', () => {
    const sources = ref<ScriptSourceItem[]>([])
    const loading = ref(false)
    const error = ref('')

    /** 已启用的自定义音源，供搜索页拼装音源列表。 */
    const enabledSources = computed(() =>
        sources.value.filter((item) => item.enabled),
    )

    function applyState(state: { sources: ScriptSourceItem[] }) {
        sources.value = state.sources
    }

    /** 把音源平台 id 映射为展示名；"script:12" → 该音源名，未知形态原样返回。 */
    function platformLabel(platform: string): string {
        const builtin = BUILTIN_LABELS[platform]
        if (builtin) return builtin
        const matched = /^script:(\d+)$/.exec(platform)
        if (!matched) return platform
        const id = Number(matched[1])
        return sources.value.find((item) => item.id === id)?.name ?? platform
    }

    /** 反向映射：音源 id → 平台 id（"script:<id>"）。 */
    function platformOf(id: number): string {
        return `script:${id}`
    }

    function findById(id: number): ScriptSourceItem | undefined {
        return sources.value.find((item) => item.id === id)
    }

    async function load(): Promise<void> {
        loading.value = true
        try {
            applyState(await scriptSourceApi.loadScriptSources())
            error.value = ''
        } catch (loadError) {
            error.value = messageOf(loadError)
            console.error('加载自定义音源失败:', loadError)
        } finally {
            loading.value = false
        }
    }

    /** 安装脚本；成功返回新装上的条目，失败抛出（由调用方提示）。 */
    async function install(
        script: string,
        name?: string,
    ): Promise<ScriptSourceItem | null> {
        const known = new Set(sources.value.map((item) => item.id))
        try {
            applyState(await scriptSourceApi.installScriptSource(script, name))
            error.value = ''
        } catch (installError) {
            error.value = messageOf(installError)
            throw installError
        }
        return sources.value.find((item) => !known.has(item.id)) ?? null
    }

    async function setEnabled(id: number, enabled: boolean): Promise<void> {
        try {
            applyState(
                await scriptSourceApi.setScriptSourceEnabled(id, enabled),
            )
            error.value = ''
        } catch (toggleError) {
            error.value = messageOf(toggleError)
            throw toggleError
        }
    }

    async function remove(id: number): Promise<void> {
        try {
            applyState(await scriptSourceApi.removeScriptSource(id))
            error.value = ''
        } catch (removeError) {
            error.value = messageOf(removeError)
            throw removeError
        }
    }

    /** 试运行脚本（不落盘），失败时把错误留在返回值里而不是抛异常。 */
    async function test(
        script: string,
    ): Promise<scriptSourceApi.ScriptSourceTestResult> {
        return scriptSourceApi.testScriptSource(script)
    }

    async function search(
        id: number,
        keyword: string,
        page = 1,
        limit = 20,
    ): Promise<SearchResponse> {
        return scriptSourceApi.searchScriptSource(id, keyword, page, limit)
    }

    return {
        sources,
        loading,
        error,
        enabledSources,
        platformLabel,
        platformOf,
        findById,
        load,
        install,
        setEnabled,
        remove,
        test,
        search,
    }
})

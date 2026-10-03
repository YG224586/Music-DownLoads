import { invoke, isTauri } from '@tauri-apps/api/core'
import type {
    ScriptSourceItem,
    ScriptSourceState,
    SearchResponse,
} from '../types'
import { webRequest } from './webClient'

/**
 * 自定义音源（脚本音源）接口层。
 * 冻结契约见 _dev/script-spec/API.md §7：Tauri 命令 snake_case，Web 调试走 mock 的同名 REST。
 * 字段名（snake_case ↔ camelCase）的归一化只在本文件做，上层一律拿到 camelCase。
 */

/** test_script_source 的返回结构，见 API.md §7。 */
export interface ScriptSourceTestResult {
    ok: boolean
    name: string | null
    qualities: string[]
    sample: unknown
    error: string | null
}

interface RawScriptSourceItem {
    id?: number | string
    name?: string
    description?: string | null
    qualities?: string[]
    enabled?: boolean
    installedAt?: string
    installed_at?: string
    scriptLength?: number
    script_length?: number
}

interface RawScriptSourceState {
    sources?: RawScriptSourceItem[] | null
}

function normalizeItem(raw: RawScriptSourceItem): ScriptSourceItem {
    return {
        id: Number(raw.id ?? 0),
        name: String(raw.name ?? ''),
        description: raw.description ?? null,
        qualities: Array.isArray(raw.qualities)
            ? raw.qualities.map(String)
            : [],
        enabled: Boolean(raw.enabled),
        installedAt: String(raw.installedAt ?? raw.installed_at ?? ''),
        scriptLength: Number(raw.scriptLength ?? raw.script_length ?? 0),
    }
}

function normalizeState(value: unknown): ScriptSourceState {
    const raw = (value ?? {}) as RawScriptSourceState
    const list = Array.isArray(raw.sources) ? raw.sources : []
    return { sources: list.map(normalizeItem) }
}

function normalizeTestResult(value: unknown): ScriptSourceTestResult {
    const raw = (value ?? {}) as {
        ok?: boolean
        name?: string | null
        qualities?: string[]
        sample?: unknown
        error?: string | null
    }
    return {
        ok: Boolean(raw.ok),
        name: raw.name ?? null,
        qualities: Array.isArray(raw.qualities)
            ? raw.qualities.map(String)
            : [],
        sample: raw.sample ?? null,
        error: raw.error ?? null,
    }
}

/** 列出已安装的自定义音源。 */
export async function loadScriptSources(): Promise<ScriptSourceState> {
    if (!isTauri()) {
        return normalizeState(await webRequest<unknown>('/api/script-sources'))
    }
    return normalizeState(await invoke<unknown>('list_script_sources'))
}

/** 安装一段脚本，name 省略时由脚本里的 source.name 决定。 */
export async function installScriptSource(
    script: string,
    name?: string,
    url?: string,
): Promise<ScriptSourceState> {
    if (!isTauri()) {
        const body = { name: name ?? null, script, url: url ?? null }
        return normalizeState(
            await webRequest<unknown>('/api/script-sources/install', {
                method: 'POST',
                body: JSON.stringify(body),
            }),
        )
    }
    return normalizeState(
        await invoke<unknown>('install_script_source', {
            name: name ?? null,
            script,
            url: url ?? null,
        }),
    )
}

/** 启用或停用某个已安装音源。 */
export async function setScriptSourceEnabled(
    id: number,
    enabled: boolean,
): Promise<ScriptSourceState> {
    if (!isTauri()) {
        return normalizeState(
            await webRequest<unknown>('/api/script-sources/toggle', {
                method: 'POST',
                body: JSON.stringify({ id, enabled }),
            }),
        )
    }
    return normalizeState(
        await invoke<unknown>('set_script_source_enabled', { id, enabled }),
    )
}

/** 卸载某个已安装音源。 */
export async function removeScriptSource(
    id: number,
): Promise<ScriptSourceState> {
    if (!isTauri()) {
        return normalizeState(
            await webRequest<unknown>('/api/script-sources/remove', {
                method: 'POST',
                body: JSON.stringify({ id }),
            }),
        )
    }
    return normalizeState(await invoke<unknown>('remove_script_source', { id }))
}

/** 试运行脚本，不写入存储。 */
export async function testScriptSource(
    script: string,
): Promise<ScriptSourceTestResult> {
    if (!isTauri()) {
        return normalizeTestResult(
            await webRequest<unknown>('/api/script-sources/test', {
                method: 'POST',
                body: JSON.stringify({ script }),
            }),
        )
    }
    return normalizeTestResult(
        await invoke<unknown>('test_script_source', { script }),
    )
}

/** 用某个自定义音源搜索歌曲（供搜索页使用）。 */
export async function searchScriptSource(
    id: number,
    keyword: string,
    page: number,
    limit: number,
): Promise<SearchResponse> {
    if (!isTauri()) {
        return webRequest<SearchResponse>('/api/script-sources/search', {
            method: 'POST',
            body: JSON.stringify({ id, keyword, page, limit }),
        })
    }
    return invoke<SearchResponse>('search_script_source', {
        id,
        keyword,
        page,
        limit,
    })
}

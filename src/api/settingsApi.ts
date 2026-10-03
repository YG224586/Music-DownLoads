import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@tauri-apps/api/core'
import { normalizePlatformCookies } from '../types'
import type { PlatformCookies, Settings } from '../types'
import { WebRequestError, webRequest } from './webClient'

/** Rust 返回带修订号的可见设置；凭据和内部存储字段不会进入页面。 */
export interface SettingsSnapshot {
    revision: number
    settings: Partial<Settings>
}

export class SettingsConflictError extends Error {
    readonly fields: string[]
    readonly snapshot: SettingsSnapshot

    constructor(fields: string[], snapshot: SettingsSnapshot) {
        super('设置已被其他页面更新')
        this.fields = fields
        this.snapshot = snapshot
    }
}

export async function loadSettings(): Promise<SettingsSnapshot> {
    if (!isTauri()) {
        return webRequest<SettingsSnapshot>('/api/settings')
    }
    return invoke<SettingsSnapshot>('get_settings_snapshot')
}

/**
 * 字段级期望原值：`null` 表示读取时该字段在服务端并不存在。
 *
 * 逐字段允许 `null`，所以既能传 `Partial<Settings>`，也能传 `{ [字段]: null }`。
 */
export type SettingsExpected = {
    [K in keyof Settings]?: Settings[K] | null
}

/** 提交字段及读取时的原值。相同字段发生并发修改时，保留快照交给页面决策。 */
export async function patchSettings(
    changes: Partial<Settings>,
    expected: SettingsExpected,
): Promise<SettingsSnapshot> {
    const patch = { changes, expected }
    try {
        if (!isTauri()) {
            return await webRequest<SettingsSnapshot>('/api/settings', {
                method: 'PATCH',
                body: JSON.stringify(patch),
            })
        }
        return await invoke<SettingsSnapshot>('patch_settings', { patch })
    } catch (error) {
        // Tauri command 的错误是 JSON 字符串，HTTP 的 409 则保留了解析后的响应体。
        const response =
            error instanceof WebRequestError
                ? error.body
                : parseTauriError(error)
        if (isConflictResponse(response)) {
            throw new SettingsConflictError(response.fields, response.snapshot)
        }
        throw error
    }
}

function parseTauriError(error: unknown): unknown {
    try {
        return JSON.parse(String(error)) as unknown
    } catch {
        return null
    }
}

function isConflictResponse(value: unknown): value is {
    fields: string[]
    snapshot: SettingsSnapshot
} {
    if (typeof value !== 'object' || value === null) return false
    const candidate = value as { fields?: unknown; snapshot?: unknown }
    return (
        Array.isArray(candidate.fields) &&
        typeof candidate.snapshot === 'object'
    )
}

export function getDefaultDownloadDir(): Promise<string> {
    if (!isTauri()) {
        return webRequest('/api/settings/default-download-dir')
    }
    return invoke<string>('get_default_download_dir')
}

/**
 * 读取四个音源的账号 Cookie（QQ 音乐 / 酷狗 / 网易云 / 咪咕）。
 *
 * 为什么不走 patch_settings：核心白名单从 v1.0.8 起已接受 platformCookies，但字段级 patch
 * 是整体替换、不做空串归一化，Tauri 端继续用专用命令 get_platform_cookies（网页版没有该
 * 命令，退回设置快照里的同名字段读回原值）。
 *
 * 返回固定四键的映射；空串 = 未配置（该音源按匿名能力降级，不换源）。
 */
export async function getPlatformCookies(): Promise<PlatformCookies> {
    if (!isTauri()) {
        const snapshot = await loadSettings()
        return normalizePlatformCookies(snapshot.settings.platformCookies)
    }
    return normalizePlatformCookies(
        await invoke<unknown>('get_platform_cookies'),
    )
}

/**
 * 整体替换四个音源的账号 Cookie，返回后端归一化后的值。
 *
 * 后端会把空串 / 纯空白按「未配置」清除并 trim，且不做 revision 冲突检测，
 * 所以调用方必须用返回值刷新界面，而不是假定写入的就是提交的内容。
 */
export async function setPlatformCookies(
    cookies: PlatformCookies,
): Promise<PlatformCookies> {
    if (!isTauri()) {
        // 网页版没有专用命令：按字段级写入提交，原值取自同一快照以保证 expected 正确。
        const before = await loadSettings()
        const snapshot = await patchSettings(
            { platformCookies: cookies },
            { platformCookies: before.settings.platformCookies ?? null },
        )
        return normalizePlatformCookies(snapshot.settings.platformCookies)
    }
    return normalizePlatformCookies(
        await invoke<unknown>('set_platform_cookies', { cookies }),
    )
}

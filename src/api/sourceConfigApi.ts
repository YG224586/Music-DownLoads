import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '@tauri-apps/api/core'
import type { SourceConfigState } from '../types'
import { webRequest } from './webClient'

/** 读取当前音源配置（未导入时返回默认双平台状态）。 */
export async function loadSourceConfig(): Promise<SourceConfigState> {
    if (!isTauri()) {
        return webRequest<SourceConfigState>('/api/source-config')
    }
    return invoke<SourceConfigState>('get_source_config')
}

/** 从 URL 下载 QingMusic 风格 music.json 并导入。 */
export async function importSourceConfig(
    url: string,
): Promise<SourceConfigState> {
    if (!isTauri()) {
        return webRequest<SourceConfigState>('/api/source-config/import', {
            method: 'POST',
            body: JSON.stringify({ url }),
        })
    }
    return invoke<SourceConfigState>('import_source_config', { url })
}

/** 启用/停用某个音源。 */
export async function setSourceEnabled(
    id: string,
    enabled: boolean,
): Promise<SourceConfigState> {
    if (!isTauri()) {
        return webRequest<SourceConfigState>('/api/source-config/toggle', {
            method: 'POST',
            body: JSON.stringify({ id, enabled }),
        })
    }
    return invoke<SourceConfigState>('set_source_enabled', { id, enabled })
}

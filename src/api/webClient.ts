import { reactive } from 'vue'

/** 访问凭据仅保存在当前浏览器会话；关闭浏览器后重新登录。 */
const CREDENTIAL_KEY = 'hotdownloader-web-credential'
const LEGACY_TOKEN_KEY = 'hotdownloader-web-token'
type AuthMode = 'none' | 'token' | 'password'

export const webSession = reactive({
    authorized: false,
    checking: false,
    mode: null as AuthMode | null,
    error: '',
})

let authorization = ''

export function webHeaders(): HeadersInit {
    return authorization ? { Authorization: authorization } : {}
}

/** 保留 HTTP 状态和响应体，设置页据此区分字段冲突与普通请求错误。 */
export class WebRequestError extends Error {
    readonly status: number
    readonly body: unknown

    constructor(message: string, status: number, body: unknown) {
        super(message)
        this.status = status
        this.body = body
    }
}

export async function webRequest<T>(
    path: string,
    options: RequestInit = {},
): Promise<T> {
    const headers = new Headers(options.headers)
    if (authorization) {
        headers.set('Authorization', authorization)
    }
    if (options.body !== undefined) {
        headers.set('Content-Type', 'application/json')
    }

    const response = await fetch(path, {
        ...options,
        headers,
        cache: 'no-store',
    })
    if (response.status === 401) {
        webSession.authorized = false
        throw new WebRequestError('访问凭据无效或已失效', 401, null)
    }
    if (!response.ok) {
        const body = (await response.json().catch(() => null)) as {
            error?: string
        } | null
        throw new WebRequestError(
            body?.error ?? `请求失败：HTTP ${response.status}`,
            response.status,
            body,
        )
    }
    return response.json() as Promise<T>
}

function basicAuthorization(username: string, password: string): string {
    const bytes = new TextEncoder().encode(`${username}:${password}`)
    let binary = ''
    for (const byte of bytes) binary += String.fromCharCode(byte)
    return `Basic ${btoa(binary)}`
}

async function checkAccess(candidate: string): Promise<boolean> {
    webSession.checking = true
    webSession.error = ''
    authorization = candidate
    try {
        await webRequest<unknown[]>('/api/tasks')
        webSession.authorized = true
        if (candidate) sessionStorage.setItem(CREDENTIAL_KEY, candidate)
        else sessionStorage.removeItem(CREDENTIAL_KEY)
        sessionStorage.removeItem(LEGACY_TOKEN_KEY)
        return true
    } catch (error) {
        authorization = ''
        webSession.authorized = false
        webSession.error =
            error instanceof Error ? error.message : String(error)
        sessionStorage.removeItem(CREDENTIAL_KEY)
        sessionStorage.removeItem(LEGACY_TOKEN_KEY)
        return false
    } finally {
        webSession.checking = false
    }
}

export async function authorizeWeb(token?: string): Promise<boolean> {
    if (webSession.mode === null) {
        webSession.checking = true
        try {
            const response = await fetch('/api/auth/mode', {
                cache: 'no-store',
            })
            if (!response.ok)
                throw new Error(`读取认证配置失败：HTTP ${response.status}`)
            const result = (await response.json()) as { mode: AuthMode }
            if (!['none', 'token', 'password'].includes(result.mode))
                throw new Error('未知认证模式')
            webSession.mode = result.mode
        } catch (error) {
            webSession.error =
                error instanceof Error ? error.message : String(error)
            webSession.checking = false
            return false
        }
    }

    const saved = sessionStorage.getItem(CREDENTIAL_KEY) ?? ''
    const legacyToken = sessionStorage.getItem(LEGACY_TOKEN_KEY) ?? ''
    const candidate =
        webSession.mode === 'none'
            ? ''
            : webSession.mode === 'token'
              ? `Bearer ${token?.trim() ?? (saved.startsWith('Bearer ') ? saved.slice(7) : legacyToken)}`
              : saved.startsWith('Basic ')
                ? saved
                : ''
    return checkAccess(candidate)
}

export async function authorizeWebWithPassword(
    username: string,
    password: string,
): Promise<boolean> {
    return checkAccess(basicAuthorization(username, password))
}

<template>
    <!--
        为什么需要账号：直链由**各音源自己的服务端**按账号权益签发，匿名请求拿不到
        （酷狗回「The Resource Needs to be Paid」、网易云 302 到 /404、咪咕回「暂不提供试听地址」、
        QQ 音乐匿名取链被平台关闭，一律返回 result=104003）。缺凭据时按各平台匿名能力降级，
        绝不换到其它音源。
    -->
    <SettingRow label="平台账号 Cookie" stacked>
        <template #description>
            <span class="account-help-line"
                >直链由各音源自己的服务端按账号权益签发，匿名请求拿不到。
                填入某个平台的 Cookie
                后，下载该平台曲目时就走该平台自己的账号链路。</span
            >
            <span class="account-help-line"
                >QQ 音乐的匿名通道已被平台关闭（匿名取链一律返回
                result=104003），不填 Cookie 时 QQ 曲目会直接报「需要 QQ
                音乐账号」；其余留空的平台按匿名能力降级，只拿该平台匿名能给的档位，拿不到时提示该平台自己的失败原因；
                <strong>任何情况下都不会改用其它音源的接口</strong>。</span
            >
            <span class="account-help-line"
                >Cookie
                只保存在本机设置里，不写入任务记录、不进入日志；保存成功后对之后发起的下载立即生效。</span
            >
        </template>
    </SettingRow>

    <SettingRow
        v-if="loadError"
        destructive
        label="读取账号 Cookie 失败"
        :description="loadError"
    />

    <SettingRow
        v-for="platform in PLATFORMS"
        :key="platform.key"
        :label="platform.label"
        stacked
    >
        <template #description>
            <span class="account-help-line">{{ platform.description }}</span>
            <span class="account-help-line">{{ platform.howToGet }}</span>
        </template>
        <template #default="{ labelId }">
            <div class="cookie-field">
                <n-input
                    class="cookie-input"
                    type="textarea"
                    :rows="2"
                    :autosize="{ minRows: 2, maxRows: 6 }"
                    :value="drafts[platform.key]"
                    :placeholder="platform.placeholder"
                    :input-props="{ 'aria-labelledby': labelId }"
                    @update:value="(value) => handleInput(platform.key, value)"
                    @blur="() => void save(platform.key)"
                />
                <div class="cookie-actions">
                    <span class="cookie-status">{{
                        statusText(platform.key)
                    }}</span>
                    <span
                        class="cookie-feedback"
                        :class="{ 'is-error': hasError(platform.key) }"
                        >{{ feedbackText(platform.key) }}</span
                    >
                    <n-button
                        size="small"
                        secondary
                        :loading="savingKey === platform.key"
                        :disabled="!isDirty(platform.key)"
                        @click="() => void save(platform.key)"
                        >保存</n-button
                    >
                    <n-button
                        size="small"
                        quaternary
                        :disabled="
                            !isConfigured(platform.key) ||
                            savingKey === platform.key
                        "
                        @click="() => void clearCookie(platform.key)"
                        >清除</n-button
                    >
                </div>
            </div>
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { NButton, NInput } from 'naive-ui'
import * as settingsApi from '../../api/settingsApi'
import { EMPTY_PLATFORM_COOKIES, PLATFORM_COOKIE_KEYS } from '../../types'
import type { PlatformCookies, PlatformCookieKey } from '../../types'
import SettingRow from './SettingRow.vue'

/**
 * 各音源的账号 Cookie 设置（QQ 音乐 / 酷狗 / 网易云 / 咪咕）。
 *
 * 为什么需要账号：直链由各音源自己的服务端按账号权益签发，匿名请求拿不到；QQ 音乐更彻底，
 * 平台已经关闭匿名取链（一律 result=104003）。缺凭据时按各平台自己的匿名能力降级，
 * **不做跨音源替换**——本组件只负责把用户自己的账号凭据写到本机设置，取链与降级逻辑在下载核心。
 *
 * 读写走专用命令（见 api/settingsApi.ts 的 get/setPlatformCookies）：核心设置白名单虽然从
 * v1.0.8 起接受 platformCookies 字段，但字段级 patch 只做整体替换、不带 trim 归一化，
 * 所以界面仍用专用命令，并以命令返回值为准。保存没有 revision 冲突检测。
 *
 * 安全约定：Cookie 不打印到控制台、不拼进任何提示文本；保存失败的提示只传达失败原因。
 */
interface PlatformDefinition {
    key: PlatformCookieKey
    label: string
    description: string
    howToGet: string
    placeholder: string
}

/** 顺序即界面顺序；键名与 Rust 侧 PlatformCookies 一致，不能改。 */
const PLATFORMS: readonly PlatformDefinition[] = [
    {
        key: 'qq',
        label: 'QQ 音乐账号 Cookie',
        description:
            '填写后：用 QQ 音乐自己的账号权益取链，可下载 QQ 音乐的付费 / VIP 曲目，能取到哪些档位取决于该账号权益。留空：匿名通道已被平台关闭（取链一律返回 result=104003），QQ 曲目会直接报「需要 QQ 音乐账号」，不会改用其它音源。',
        howToGet:
            '获取方式：在电脑浏览器登录 y.qq.com（QQ 音乐网页版）后按 F12 → Application → Cookies，复制 uin 与 qqmusic_key（旧版叫 qm_keyst）两个键值，拼成整行粘贴。',
        placeholder: 'uin=o0123456789; qqmusic_key=…',
    },
    {
        key: 'kugou',
        label: '酷狗音乐账号 Cookie',
        description:
            '填写后：用酷狗自己的账号权益取链，可下载酷狗付费 / VIP 曲目。留空：按酷狗匿名能力降级，不会改用其它音源。',
        howToGet:
            '获取方式：在浏览器登录酷狗后复制整条 Cookie，至少包含 token、userid、dfid、mid 四个字段。',
        placeholder: 'token=…; userid=…; dfid=…; mid=…',
    },
    {
        key: 'netease',
        label: '网易云音乐账号 Cookie',
        description:
            '填写后：用网易云自己的账号权益取链，可下载 VIP 曲目，并解开匿名下的无损降级限制。留空：按网易云匿名能力降级，不会改用其它音源。',
        howToGet:
            '获取方式：在浏览器登录 music.163.com 后复制整条 Cookie，至少包含 MUSIC_U。',
        placeholder: 'MUSIC_U=…',
    },
    {
        key: 'migu',
        label: '咪咕音乐账号 Cookie',
        description:
            '填写后：用咪咕自己的账号权益取链，可下载咪咕付费 / VIP 独占曲目。留空：按咪咕匿名能力降级，不会改用其它音源。',
        howToGet:
            '获取方式：在浏览器登录咪咕音乐后复制整条 Cookie，至少包含 token、userId。',
        placeholder: 'token=…; userId=…',
    },
]

/** 输入框内容；用户正在编辑的值，保存成功后才与 persisted 对齐。 */
const drafts = reactive<PlatformCookies>({ ...EMPTY_PLATFORM_COOKIES })
/** 后端确认过的值（已 trim，空串 = 未配置）；状态标识与「是否有改动」都以它为准。 */
const persisted = ref<PlatformCookies>({ ...EMPTY_PLATFORM_COOKIES })
const loading = ref(true)
const loadError = ref('')
const savingKey = ref<PlatformCookieKey | null>(null)
const errors = ref<Partial<Record<PlatformCookieKey, string>>>({})
const savedKey = ref<PlatformCookieKey | null>(null)

/** 「已保存」是中性、非打断式反馈：短暂显示后自动消失。 */
const FEEDBACK_MS = 2500
let feedbackTimer: ReturnType<typeof setTimeout> | null = null

/**
 * 错误文本兜底：后端理论上不回显 Cookie，但一旦错误信息里夹带了凭据，
 * 就在显示前替换掉，避免整条账号凭据出现在设置页上。
 */
function sanitizeError(message: string): string {
    let text = message
    for (const key of PLATFORM_COOKIE_KEYS) {
        for (const candidate of [drafts[key], persisted.value[key]]) {
            const secret = candidate.trim()
            if (secret.length >= 8 && text.includes(secret)) {
                text = text.split(secret).join('（已隐藏）')
            }
        }
    }
    return text
}

function errorText(error: unknown): string {
    return sanitizeError(error instanceof Error ? error.message : String(error))
}

function isConfigured(key: PlatformCookieKey): boolean {
    return persisted.value[key].length > 0
}

function isDirty(key: PlatformCookieKey): boolean {
    return drafts[key].trim() !== persisted.value[key]
}

function hasError(key: PlatformCookieKey): boolean {
    return Boolean(errors.value[key])
}

function statusText(key: PlatformCookieKey): string {
    if (loading.value) return '正在读取…'
    return isConfigured(key) ? '已配置' : '未配置（匿名）'
}

function feedbackText(key: PlatformCookieKey): string {
    const error = errors.value[key]
    if (error) return `保存失败：${error}`
    return savedKey.value === key ? '已保存' : ''
}

function scheduleFeedbackClear() {
    if (feedbackTimer) clearTimeout(feedbackTimer)
    feedbackTimer = setTimeout(() => {
        feedbackTimer = null
        savedKey.value = null
    }, FEEDBACK_MS)
}

function handleInput(key: PlatformCookieKey, value: string) {
    drafts[key] = value
    // 用户继续编辑时，上一次的保存/失败提示已经不对应当前内容，立即撤下。
    if (errors.value[key]) delete errors.value[key]
    if (savedKey.value === key) savedKey.value = null
}

async function loadCookies() {
    loading.value = true
    loadError.value = ''
    try {
        const current = await settingsApi.getPlatformCookies()
        persisted.value = current
        for (const key of PLATFORM_COOKIE_KEYS) drafts[key] = current[key]
    } catch (error) {
        loadError.value = errorText(error)
    } finally {
        loading.value = false
    }
}

/**
 * 保存单个平台的 Cookie（失焦或点「保存」触发）。
 *
 * 只提交有改动的平台：其余键沿用 persisted，避免把别的输入框里还没保存的内容一起写进去。
 */
async function save(key: PlatformCookieKey) {
    if (loading.value || savingKey.value === key) return
    const value = drafts[key].trim()
    if (value === persisted.value[key]) {
        // 只有首尾空白差异：把输入框收敛到已保存的值，不产生无意义的写入。
        drafts[key] = value
        return
    }
    savingKey.value = key
    if (errors.value[key]) delete errors.value[key]
    savedKey.value = null
    try {
        // 后端会 trim，并把空串 / 纯空白按「未配置」清除，因此必须用返回值刷新界面。
        const confirmed = await settingsApi.setPlatformCookies({
            ...persisted.value,
            [key]: value,
        })
        persisted.value = confirmed
        drafts[key] = confirmed[key]
        savedKey.value = key
        scheduleFeedbackClear()
    } catch (error) {
        errors.value[key] = errorText(error)
    } finally {
        savingKey.value = null
    }
}

/** 清除 = 置空保存（后端按未配置处理），随后按该平台匿名能力降级。 */
async function clearCookie(key: PlatformCookieKey) {
    drafts[key] = ''
    await save(key)
}

onMounted(() => {
    void loadCookies()
})

onBeforeUnmount(() => {
    if (feedbackTimer) clearTimeout(feedbackTimer)
})
</script>

<style scoped>
.account-help-line {
    display: block;
    overflow-wrap: anywhere;
}

.account-help-line + .account-help-line {
    margin-top: var(--md-space-1);
}

.cookie-field {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-2);
    width: 100%;
    min-width: 0;
}

.cookie-input {
    width: 100%;
}

/* Cookie 是长串 key=value，等宽字体与自动换行才能看清、也不产生横向滚动 */
.cookie-input :deep(textarea) {
    font-family: var(--md-font-plain);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    overflow-wrap: anywhere;
}

.cookie-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--md-space-2) var(--md-space-3);
}

.cookie-status {
    color: var(--md-on-surface-variant);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
}

.cookie-feedback {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--md-on-surface-variant);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    overflow-wrap: anywhere;
}

.cookie-feedback.is-error {
    color: var(--md-error);
}
</style>

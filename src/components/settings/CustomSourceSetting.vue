<template>
    <!-- 已安装的自定义音源：一行一个，开关立即生效 -->
    <SettingRow
        v-for="item in store.sources"
        :key="item.id"
        :label="item.name"
        :description="describe(item)"
    >
        <template #default="{ labelId }">
            <div class="source-actions">
                <n-button
                    quaternary
                    circle
                    :aria-label="`删除音源 ${item.name}`"
                    :loading="removingId === item.id"
                    @click="confirmRemove(item)"
                >
                    <svg
                        class="source-icon"
                        viewBox="0 0 24 24"
                        aria-hidden="true"
                    >
                        <path
                            d="M7 21q-.825 0-1.412-.587Q5 19.825 5 19V6H4V4h5V3h6v1h5v2h-1v13q0 .825-.587 1.413Q17.825 21 17 21ZM17 6H7v13h10ZM9 17h2V8H9Zm4 0h2V8h-2Z"
                        />
                    </svg>
                </n-button>
                <n-switch
                    :value="item.enabled"
                    :aria-label="`启用音源 ${item.name}`"
                    :aria-labelledby="labelId"
                    :loading="togglingId === item.id"
                    @update:value="(value: boolean) => toggle(item, value)"
                />
            </div>
        </template>
    </SettingRow>

    <!-- 空态：说明这是高级功能，不配置也能用内置音源 -->
    <SettingRow
        v-if="!store.sources.length && !store.loading && !store.error"
        label="还没有自定义音源"
        description="高级功能：粘贴一段 JS 脚本即可新增音源；不填也能正常使用内置音源。"
    />

    <SettingRow
        v-if="store.loading"
        label="正在加载自定义音源…"
        description="读取本机已保存的音源脚本。"
    />

    <SettingRow
        v-if="store.error"
        label="读取自定义音源失败"
        :description="store.error"
    />

    <!-- 新增音源：只保留「粘贴脚本」一条入口 -->
    <SettingRow
        stacked
        label="新增音源"
        description="把一段 JS 脚本粘贴到下面。点击「安装」会先由下载核心校验脚本，校验不通过不会保存。"
    >
        <template #default>
            <div class="source-form">
                <n-input
                    v-model:value="name"
                    placeholder="音源名称（可留空，使用脚本内声明的名称）"
                    :input-props="{ 'aria-label': '音源名称' }"
                />
                <n-input
                    v-model:value="script"
                    type="textarea"
                    :rows="8"
                    placeholder="在此粘贴音源脚本…"
                    :input-props="{ 'aria-label': '音源脚本' }"
                />
                <div class="source-form-actions">
                    <n-button
                        secondary
                        :disabled="!script.trim()"
                        :loading="testing"
                        @click="runTest"
                    >
                        测试
                    </n-button>
                    <n-button
                        type="primary"
                        :disabled="!script.trim()"
                        :loading="installing"
                        @click="install"
                    >
                        安装
                    </n-button>
                </div>
                <n-alert
                    v-if="testResult"
                    class="source-test-result"
                    :type="testResult.ok ? 'success' : 'error'"
                    :show-icon="true"
                >
                    {{ testMessage }}
                </n-alert>
            </div>
        </template>
    </SettingRow>

    <!-- 脚本格式说明：默认折叠，避免长文占据首屏 -->
    <li class="setting-item is-stacked">
        <button
            type="button"
            class="help-toggle"
            :aria-expanded="helpOpen"
            @click="helpOpen = !helpOpen"
        >
            <span>{{
                helpOpen ? '收起脚本格式说明' : '查看脚本格式说明'
            }}</span>
            <svg class="help-chevron" viewBox="0 0 24 24" aria-hidden="true">
                <path
                    d="M9.29 6.71a1 1 0 0 0 0 1.41L13.17 12l-3.88 3.88a1 1 0 1 0 1.42 1.41l4.58-4.58a1 1 0 0 0 0-1.42l-4.58-4.58a1 1 0 0 0-1.42 0z"
                />
            </svg>
        </button>
        <div v-if="helpOpen" class="help-body">
            <p class="help-text">
                脚本是一段普通 JS，需要声明一个 <code>source</code> 对象：
                名称、可选支持音质、<code>search</code> 搜索方法和
                <code>getUrl</code> 取直链方法。脚本在本机沙箱内运行，用
                <code>http.get/post</code> 发请求，用 <code>log</code> 打日志。
            </p>
            <pre class="help-sample">{{ SCRIPT_SAMPLE }}</pre>
        </div>
    </li>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import {
    NAlert,
    NButton,
    NInput,
    NSwitch,
    useDialog,
    useNotification,
} from 'naive-ui'
import type { ScriptSourceItem } from '../../types'
import type { ScriptSourceTestResult } from '../../api/scriptSourceApi'
import { useScriptSourceStore } from '../../stores/scriptSourceStore'
import SettingRow from './SettingRow.vue'

const store = useScriptSourceStore()
const dialog = useDialog()
const notification = useNotification()

const name = ref('')
const script = ref('')
const testing = ref(false)
const installing = ref(false)
const testResult = ref<ScriptSourceTestResult | null>(null)
const helpOpen = ref(false)
const removingId = ref<number | null>(null)
const togglingId = ref<number | null>(null)

const SCRIPT_SAMPLE = `var source = {
    name: '示例音源',
    qualities: ['128kmp3'],
    search: function (keyword, page, limit) {
        var res = http.get('https://example.com/api/search?q=' + encodeURIComponent(keyword))
        var data = JSON.parse(res.body)
        return {
            songs: data.list.map(function (item) {
                return {
                    id: item.id,
                    title: item.name,
                    artist: item.singer,
                    album: item.album,
                    duration: item.duration,
                    cover: item.cover,
                    qualities: ['128kmp3'],
                }
            }),
            has_more: data.list.length >= limit,
        }
    },
    getUrl: function (song, quality) {
        var res = http.get('https://example.com/api/url?id=' + song.id + '&quality=' + quality)
        return JSON.parse(res.body).url
    },
}`

const testMessage = computed(() => {
    const result = testResult.value
    if (!result) {
        return ''
    }
    if (result.ok) {
        const qualities = result.qualities.length
            ? result.qualities.join(' / ')
            : '未声明音质'
        return `脚本可用：${result.name ?? '未命名音源'}，支持音质 ${qualities}`
    }
    return `脚本不可用：${result.error ?? '未知错误'}`
})

function describe(item: ScriptSourceItem): string {
    const qualities = item.qualities.length
        ? item.qualities.join(' / ')
        : '未声明音质'
    return `支持音质：${qualities} · 脚本 ${item.scriptLength} 字节`
}

async function runTest() {
    if (!script.value.trim() || testing.value) {
        return
    }
    testing.value = true
    try {
        testResult.value = await store.test(script.value)
    } finally {
        testing.value = false
    }
}

async function install() {
    if (!script.value.trim() || installing.value) {
        return
    }
    installing.value = true
    const trimmedName = name.value.trim()
    try {
        const installed = await store.install(
            script.value,
            trimmedName || undefined,
        )
        notification.success({
            content: `已安装音源「${installed?.name ?? trimmedName ?? '未命名音源'}」`,
            duration: 3000,
        })
        script.value = ''
        name.value = ''
        testResult.value = null
    } catch (error) {
        notification.error({
            content: `安装失败：${messageOf(error)}`,
            duration: 5000,
        })
    } finally {
        installing.value = false
    }
}

function toggle(item: ScriptSourceItem, value: boolean) {
    if (togglingId.value === item.id) {
        return
    }
    togglingId.value = item.id
    void store
        .setEnabled(item.id, value)
        .catch((error: unknown) => {
            notification.error({
                content: `开关切换失败：${messageOf(error)}`,
                duration: 5000,
            })
        })
        .finally(() => {
            togglingId.value = null
        })
}

function confirmRemove(item: ScriptSourceItem) {
    dialog.error({
        title: `删除音源「${item.name}」`,
        content: '删除后本机保存的脚本会被移除，且无法恢复。',
        positiveText: '删除',
        negativeText: '取消',
        positiveButtonProps: { type: 'error' },
        onPositiveClick: () => {
            removingId.value = item.id
            void store
                .remove(item.id)
                .catch((error: unknown) => {
                    notification.error({
                        content: `删除失败：${messageOf(error)}`,
                        duration: 5000,
                    })
                })
                .finally(() => {
                    removingId.value = null
                })
        },
    })
}

function messageOf(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
}
</script>

<style scoped>
.source-actions,
.source-form-actions {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
}

.source-form {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
    width: 100%;
    min-width: 0;
}

.source-form-actions {
    justify-content: flex-end;
}

.source-test-result {
    border-radius: var(--md-shape-sm);
}

.source-icon {
    width: 22px;
    height: 22px;
    fill: currentColor;
}

.help-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-3);
    box-sizing: border-box;
    width: 100%;
    min-height: var(--md-target-min);
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--md-primary);
    font: inherit;
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    text-align: left;
    cursor: pointer;
}

.help-chevron {
    width: 20px;
    height: 20px;
    flex: 0 0 20px;
    fill: currentColor;
    transition: transform var(--md-duration-short) var(--md-easing-standard);
}

.help-toggle[aria-expanded='true'] .help-chevron {
    transform: rotate(90deg);
}

.help-body {
    width: 100%;
    min-width: 0;
}

.help-text {
    margin: 0 0 var(--md-space-3);
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    overflow-wrap: anywhere;
}

.help-text code {
    padding: 0 4px;
    border-radius: var(--md-shape-xs);
    background: var(--md-surface-container-high);
    font-family: var(--md-font-plain);
    font-size: var(--md-label-medium);
}

.help-sample {
    box-sizing: border-box;
    margin: 0;
    padding: var(--md-space-3);
    border-radius: var(--md-shape-sm);
    background: var(--md-surface-container-high);
    color: var(--md-on-surface-variant);
    font-family: var(--md-font-plain);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    white-space: pre-wrap;
    word-break: break-word;
}
</style>

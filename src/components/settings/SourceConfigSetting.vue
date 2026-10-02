<template>
    <SettingRow label="音源配置" stacked>
        <template #description>
            导入 QingMusic 风格
            music.json，控制启用的音源与可下载音质；不导入时默认启用全部内置音源。
        </template>
        <template #default="{ labelId }">
            <div class="source-config">
                <div class="import-row">
                    <n-input
                        v-model:value="urlInput"
                        class="url-input"
                        :aria-labelledby="labelId"
                        placeholder="music.json 地址"
                        :disabled="store.importing"
                        clearable
                    />
                    <n-button
                        type="primary"
                        :loading="store.importing"
                        :disabled="store.importing || !urlInput.trim()"
                        @click="onImport"
                    >
                        导入
                    </n-button>
                </div>
                <p v-if="store.importError" class="import-error" role="alert">
                    {{ store.importError }}
                </p>
                <ul v-if="sources.length > 0" class="source-list">
                    <li
                        v-for="source in sources"
                        :key="source.id"
                        class="source-item"
                    >
                        <div class="source-text">
                            <span class="source-name">{{ source.name }}</span>
                            <n-tag
                                size="small"
                                :bordered="false"
                                :type="source.platform ? 'success' : 'default'"
                            >
                                {{ platformLabel(source) }}
                            </n-tag>
                        </div>
                        <n-switch
                            :value="source.enabled"
                            :disabled="!source.platform"
                            :aria-label="`${source.name}开关`"
                            @update:value="
                                (val: boolean) => onToggle(source.id, val)
                            "
                        />
                    </li>
                </ul>
            </div>
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { NButton, NInput, NSwitch, NTag } from 'naive-ui'
import { useSourceConfigStore } from '../../stores/sourceConfigStore'
import type { SourceMapping } from '../../types'
import SettingRow from './SettingRow.vue'

const DEFAULT_URL = 'https://13413.kstore.vip/QingMusic/music.json'

const store = useSourceConfigStore()
const urlInput = ref(DEFAULT_URL)
const sources = ref<SourceMapping[]>([])

const PLATFORM_NAMES: Record<string, string> = {
    qqmusic: 'QQ音乐',
    kuwo: '酷我音乐',
}

function platformLabel(source: SourceMapping): string {
    if (!source.platform) {
        return '暂不支持'
    }
    return PLATFORM_NAMES[source.platform] ?? source.platform
}

function refreshSources(): void {
    sources.value = store.state?.sources ?? []
}

async function onImport(): Promise<void> {
    try {
        await store.importUrl(urlInput.value.trim())
        refreshSources()
    } catch {
        /* 错误已写入 store.importError 展示 */
    }
}

async function onToggle(id: string, enabled: boolean): Promise<void> {
    try {
        await store.toggle(id, enabled)
    } catch (error) {
        console.error('切换音源失败', error)
    }
    refreshSources()
}

onMounted(() => {
    if (store.state) {
        urlInput.value = store.state.url ?? DEFAULT_URL
    }
    refreshSources()
    if (!store.loaded) {
        void store.load().then(refreshSources)
    }
})
</script>

<style scoped>
.source-config {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
    width: 100%;
}

.import-row {
    display: flex;
    flex: 1 1 auto;
    gap: var(--md-space-2);
    align-items: center;
    width: 100%;
}

.url-input {
    flex: 1 1 auto;
    min-width: 0;
}

.import-error {
    margin: 0;
    color: var(--md-error);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    overflow-wrap: anywhere;
}

.source-list {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
}

.source-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-3);
    min-height: 48px;
    padding: var(--md-space-1) 0;
}

.source-text {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    gap: var(--md-space-2);
    min-width: 0;
}

.source-name {
    color: var(--md-on-surface);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    overflow-wrap: anywhere;
}
</style>

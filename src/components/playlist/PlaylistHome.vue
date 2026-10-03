<template>
    <div class="playlist-home">
        <section class="playlist-section">
            <h2 class="section-title">导入歌单</h2>
            <SearchBar
                v-model:keyword="importInput"
                v-model:platform="currentPlatform"
                :platform-options="importPlatforms"
                placeholder="请输入歌单链接或 ID"
                button-text="导入歌单"
                @search="handleImport"
            />
        </section>

        <section class="playlist-section">
            <h2 class="section-title">我的 QQ 音乐歌单</h2>
            <div v-if="myLoading" class="state-wrapper">
                <n-spin size="medium" />
            </div>
            <template v-else-if="myError">
                <n-alert type="error" title="获取我的歌单失败">{{
                    myError
                }}</n-alert>
                <n-button
                    class="retry-button"
                    secondary
                    @click="refreshMyPlaylists"
                >
                    重试
                </n-button>
            </template>
            <div v-else-if="!loggedIn" class="signin-block">
                <!--
                    登录入口已随扫码登录一起移除（见 task-34）：跳设置页只会白跑一趟，
                    因此这里只说明现状与可行范围，不再提供任何「前往登录」动作。
                -->
                <p class="signin-text">
                    本版本已移除应用内登录，未登录时仅支持导入公开歌单
                </p>
            </div>
            <PlaylistSearchResult
                v-else
                :playlists="myPlaylists"
                :has-more="false"
                :loading-more="false"
                empty-description="暂无自己创建的歌单"
                @click-playlist="(item) => emit('open-my', item)"
            />
        </section>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { NAlert, NButton, NSpin } from 'naive-ui'
import SearchBar from '../search/SearchBar.vue'
import PlaylistSearchResult from '../search/PlaylistSearchResult.vue'
import * as musicApi from '../../api/musicApi'
import { DEFAULT_PLATFORM, PLATFORMS } from '../../config/platforms'
import type { PlaylistSearchItem } from '../../types'

const emit = defineEmits<{
    (e: 'import', platform: string, input: string): void
    (e: 'open-my', item: PlaylistSearchItem): void
}>()

const route = useRoute()

// 歌单导入只有 QQ 音乐与酷我音乐实现了后端解析，其余平台在能力表里为 false；
// 直接把它们从选择器里去掉，避免用户选完才收到「暂不支持」的报错。
const importPlatforms = computed(() =>
    PLATFORMS.filter((item) => item.capabilities.playlist),
)

const currentPlatform = ref(DEFAULT_PLATFORM)
const importInput = ref('')
const loggedIn = ref(false)
const myPlaylists = ref<PlaylistSearchItem[]>([])
const myLoading = ref(false)
const myError = ref('')

// 请求序号：只接受最后一次请求的结果，避免快速切换时旧响应覆盖新状态。
let requestId = 0

function handleImport() {
    const input = importInput.value.trim()
    if (!input) return
    emit('import', currentPlatform.value, input)
}

async function refreshMyPlaylists() {
    // 登录可能在设置页发生变化；每次进入入口页都重新读取当前账号。
    const currentRequest = ++requestId
    myLoading.value = true
    myError.value = ''
    try {
        const status = await musicApi.getLoginStatus('qqmusic')
        if (currentRequest !== requestId) return
        loggedIn.value = status.logged_in
        if (!status.logged_in) {
            myPlaylists.value = []
            return
        }
        const result = await musicApi.fetchCreatedPlaylists()
        if (currentRequest !== requestId) return
        myPlaylists.value = result.playlists
    } catch (error) {
        if (currentRequest !== requestId) return
        myError.value = error instanceof Error ? error.message : String(error)
    } finally {
        if (currentRequest === requestId) myLoading.value = false
    }
}

onMounted(() => {
    void refreshMyPlaylists()
})

// 页面被 keep-alive 缓存；从设置页返回时重新读取可能变化的 QQ 登录态。
watch(
    () => route.path,
    (path, previousPath) => {
        if (path === '/playlist' && previousPath !== '/playlist')
            void refreshMyPlaylists()
    },
)

onUnmounted(() => {
    requestId++
})
</script>

<style scoped>
/* 入口页：分区直接排在页面表面上，纵向按 4dp 节奏排布。 */
.playlist-home {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-6);
    min-width: 0;
}

.playlist-section {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
    min-width: 0;
}

.section-title {
    margin: 0;
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    overflow-wrap: anywhere;
}

/* 未登录提示：紧凑的一块 tonal 表面，不留大片空白。 */
.signin-block {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--md-space-3);
    padding: var(--md-space-5) var(--md-space-4);
    background-color: var(--md-surface-container-low);
    border-radius: var(--md-shape-md);
}

.signin-text {
    margin: 0;
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
}

.state-wrapper {
    display: flex;
    justify-content: center;
    padding: var(--md-space-8) 0;
}

.retry-button {
    align-self: flex-start;
}

.playlist-home :deep(.search-bar) {
    margin-bottom: 0;
}

.playlist-home :deep(.n-alert) {
    border-radius: var(--md-shape-md);
}
</style>

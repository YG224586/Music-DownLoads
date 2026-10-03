<template>
    <div class="search-view">
        <!-- 平台绑定 + 搜索类型切换 -->
        <div class="search-header">
            <SearchBar
                :keyword="keyword"
                @update:keyword="onKeywordInput"
                v-model:platform="currentPlatform"
                :platform-options="platformOptions"
                :placeholder="searchPlaceholder"
                button-text="搜索"
                @search="handleSearch"
            />

            <!-- 搜索类型：互斥单选，用 M3 segmented button（选中段带勾选标记） -->
            <div class="type-switch">
                <n-radio-group
                    :value="searchType"
                    class="type-group"
                    aria-label="搜索类型"
                    @update:value="(v) => switchSearchType(v as SearchType)"
                >
                    <n-radio-button
                        v-for="option in SEARCH_TYPES"
                        :key="option.value"
                        :value="option.value"
                        :disabled="isSearchTypeDisabled(option.value)"
                        class="type-segment"
                    >
                        <span class="segment-content">
                            <svg
                                class="segment-check"
                                :class="{
                                    'is-visible': searchType === option.value,
                                }"
                                viewBox="0 0 24 24"
                                width="18"
                                height="18"
                                aria-hidden="true"
                            >
                                <path
                                    d="m5 13 4 4 10-10"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                />
                            </svg>
                            <span class="segment-label">{{
                                option.label
                            }}</span>
                        </span>
                    </n-radio-button>
                </n-radio-group>
            </div>
        </div>

        <!-- 空闲：历史与热搜 -->
        <template v-if="pageMode === 'idle'">
            <SearchHistory
                :history="historyStore.history"
                @select="onHistorySelect"
                @remove="onHistoryRemove"
                @clear="historyStore.clearHistory"
            />
            <HotKeywords
                :keywords="hotKeywords"
                :loading="hotLoading"
                @select="onHotClick"
            />
            <!-- 空白引导：吸收空闲状态的剩余高度，避免页面下半部分大片留白 -->
            <div class="idle-guide">
                <svg
                    class="idle-guide-icon"
                    viewBox="0 0 24 24"
                    width="48"
                    height="48"
                    aria-hidden="true"
                >
                    <circle
                        cx="11"
                        cy="11"
                        r="7"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                    />
                    <path
                        d="m16.2 16.2 4.3 4.3"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                    />
                </svg>
                <p class="idle-guide-text">输入关键词开始搜索</p>
            </div>
        </template>

        <!-- 输入中：搜索建议 -->
        <template v-else-if="pageMode === 'suggestions'">
            <SearchSuggestions
                :data="suggestions"
                @select="onSuggestionSelect"
            />
        </template>

        <!-- 已搜索：搜索结果 -->
        <template v-else-if="pageMode === 'results'">
            <!-- 加载中 -->
            <div v-if="searchLoading" class="loading-wrapper">
                <n-spin size="medium" />
                <span class="visually-hidden" role="status">正在搜索…</span>
            </div>

            <!-- 歌曲搜索结果列表 -->
            <SearchResultList
                v-if="searchType === 'song' && songHasSearched && !songLoading"
                :songs="songSearchResults"
                v-model:selectedIds="songSelectedIds"
                :has-more="songHasMore"
                :loading-more="songLoadingMore"
                :error="songError"
                @download="onSingleDownload"
                @retry="handleSearch"
                @load-more="loadMoreSongs"
                @click-artist="openRelatedArtist"
                @click-album="openSongAlbum"
            />

            <!-- 歌手搜索结果列表 -->
            <template v-else-if="searchType === 'artist'">
                <n-alert
                    v-if="artistError"
                    type="error"
                    title="歌手搜索失败"
                    class="search-error"
                >
                    {{ artistError }}
                    <n-button
                        @click="
                            artistSearchResults.length
                                ? loadMoreArtists()
                                : handleSearch()
                        "
                        >重试</n-button
                    >
                </n-alert>
                <ArtistSearchResult
                    v-if="
                        artistHasSearched &&
                        !artistLoading &&
                        (!artistError || artistSearchResults.length)
                    "
                    :artists="artistSearchResults"
                    :has-more="artistHasMore"
                    :loading-more="artistLoadingMore"
                    @click-artist="
                        (artist) => openArtist(currentPlatform, artist)
                    "
                    @load-more="loadMoreArtists"
                />
            </template>

            <!-- 专辑搜索结果列表 -->
            <template v-else-if="searchType === 'album'">
                <n-alert
                    v-if="albumError"
                    type="error"
                    title="专辑搜索失败"
                    class="search-error album-error"
                >
                    {{ albumError }}
                    <n-button
                        @click="
                            albumSearchResults.length
                                ? loadMoreAlbums()
                                : handleSearch()
                        "
                        >重试</n-button
                    >
                </n-alert>
                <AlbumSearchResult
                    v-if="
                        albumHasSearched &&
                        !albumLoading &&
                        (!albumError || albumSearchResults.length)
                    "
                    :albums="albumSearchResults"
                    :platform="currentPlatform"
                    :has-more="albumHasMore"
                    :loading-more="albumLoadingMore"
                    @click-album="(album) => openAlbum(currentPlatform, album)"
                    @load-more="loadMoreAlbums"
                    @click-artist="openRelatedArtist"
                />
            </template>

            <!-- 歌单搜索结果列表 -->
            <template v-else-if="searchType === 'playlist'">
                <n-alert
                    v-if="playlistError"
                    type="error"
                    title="歌单搜索失败"
                    class="search-error"
                >
                    {{ playlistError }}
                    <n-button
                        @click="
                            playlistSearchResults.length
                                ? loadMorePlaylists()
                                : handleSearch()
                        "
                        >重试</n-button
                    >
                </n-alert>
                <PlaylistSearchResult
                    v-if="
                        playlistHasSearched &&
                        !playlistLoading &&
                        (!playlistError || playlistSearchResults.length)
                    "
                    :playlists="playlistSearchResults"
                    :has-more="playlistHasMore"
                    :loading-more="playlistLoadingMore"
                    @click-playlist="goToPlaylist"
                    @load-more="loadMorePlaylists"
                />
            </template>

            <!-- 批量下载栏（仅在歌曲搜索模式且有选中时显示） -->
            <BatchDownloadBar
                v-if="searchType === 'song' && songSelectedIds.length > 0"
                :selectedCount="songSelectedIds.length"
                @batch-download="onBatchDownload"
            />
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import { NSpin, NButton, NAlert, NRadioGroup, NRadioButton } from 'naive-ui'
import { useRoute, useRouter } from 'vue-router'
import SearchBar from '../components/search/SearchBar.vue'
import SearchHistory from '../components/search/SearchHistory.vue'
import HotKeywords from '../components/search/HotKeywords.vue'
import SearchSuggestions from '../components/search/SearchSuggestions.vue'
import SearchResultList from '../components/search/SearchResultList.vue'
import ArtistSearchResult from '../components/search/ArtistSearchResult.vue'
import { useArtistSearch } from '../composables/useArtistSearch'
import AlbumSearchResult from '../components/search/AlbumSearchResult.vue'
import { useAlbumSearch } from '../composables/useAlbumSearch'
import PlaylistSearchResult from '../components/search/PlaylistSearchResult.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import { useHistoryStore } from '../stores/historyStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useSongSearch } from '../composables/useSongSearch'
import { usePlaylistSearch } from '../composables/usePlaylistSearch'
import * as musicApi from '../api/musicApi'
import type {
    QualityItem,
    SearchSuggestionData,
    PlaylistSearchItem,
    SongInfo,
} from '../types'
import { PLATFORMS, DEFAULT_PLATFORM } from '../config/platforms'
import type { PlatformOption } from '../config/platforms'
import { useMusicNavigation } from '../composables/useMusicNavigation'
import { useScriptSourceStore } from '../stores/scriptSourceStore'

const router = useRouter()
const route = useRoute()
const { openArtist, openAlbum, openRelatedArtist, openSongAlbum } =
    useMusicNavigation()
const keyword = ref('')
const currentPlatform = ref(DEFAULT_PLATFORM)

const scriptSourceStore = useScriptSourceStore()

// 内置音源固定（QQ音乐 / 酷我音乐），自定义脚本音源在设置页安装后追加到选择器
const platformOptions = computed<PlatformOption[]>(() => [
    ...PLATFORMS,
    ...scriptSourceStore.enabledSources.map((item) => ({
        key: scriptSourceStore.platformOf(item.id),
        label: item.name,
    })),
])
watch(
    platformOptions,
    (options) => {
        if (
            options.length > 0 &&
            !options.some((p) => p.key === currentPlatform.value)
        ) {
            currentPlatform.value = options[0].key
        }
    },
    { immediate: true },
)

// 自定义脚本音源：标识形如 "script:<id>"（契约 §6）
const scriptSourceId = computed<number | null>(() => {
    const matched = /^script:(\d+)$/.exec(currentPlatform.value)
    return matched ? Number(matched[1]) : null
})
const isScriptPlatform = computed(() => scriptSourceId.value !== null)
// 展示用音源名，避免把 "script:<id>" 机器标识暴露给用户
const currentSourceLabel = computed(
    () =>
        scriptSourceStore.platformLabel(currentPlatform.value) || '自定义音源',
)
// 脚本音源只提供歌曲搜索（契约 §3/§7），其余搜索类型不可用
function isSearchTypeDisabled(type: SearchType) {
    return isScriptPlatform.value && type !== 'song'
}

// 搜索类型
type SearchType = 'song' | 'artist' | 'album' | 'playlist'
const searchType = ref<SearchType>('song')

// 分段按钮的取值与文案（顺序即界面顺序）
const SEARCH_TYPES: { value: SearchType; label: string }[] = [
    { value: 'song', label: '歌曲' },
    { value: 'artist', label: '歌手' },
    { value: 'album', label: '专辑' },
    { value: 'playlist', label: '歌单' },
]
const pageMode = ref<'idle' | 'suggestions' | 'results'>('idle')

// 使用歌曲搜索 composable，解构出状态和方法
const {
    searchResults: songSearchResults,
    selectedIds: songSelectedIds,
    loading: songLoading,
    hasSearched: songHasSearched,
    hasMore: songHasMore,
    loadingMore: songLoadingMore,
    error: songError,
    searchSongs: searchSongFunc,
    loadMoreSongs: loadMoreSongsFunc,
    reset: resetSongSearch,
} = useSongSearch()

// 使用歌单搜索 composable，解构出状态和方法
const {
    playlists: playlistSearchResults,
    loading: playlistLoading,
    hasSearched: playlistHasSearched,
    hasMore: playlistHasMore,
    loadingMore: playlistLoadingMore,
    error: playlistError,
    searchPlaylists: searchPlaylistFunc,
    loadMorePlaylists: loadMorePlaylistFunc,
    reset: resetPlaylistSearch,
} = usePlaylistSearch()

const {
    albums: albumSearchResults,
    loading: albumLoading,
    loadingMore: albumLoadingMore,
    hasSearched: albumHasSearched,
    hasMore: albumHasMore,
    error: albumError,
    search: searchAlbumFunc,
    reset: resetAlbumSearch,
    loadMore: loadMoreAlbums,
} = useAlbumSearch()
const {
    artists: artistSearchResults,
    loading: artistLoading,
    loadingMore: artistLoadingMore,
    hasSearched: artistHasSearched,
    hasMore: artistHasMore,
    error: artistError,
    search: searchArtistFunc,
    reset: resetArtistSearch,
    loadMore: loadMoreArtists,
} = useArtistSearch()
const searchPlaceholder = computed(
    () =>
        ({
            song: '搜索歌曲、歌手、专辑',
            artist: '输入关键词搜索歌手',
            album: '输入关键词搜索专辑',
            playlist: '输入关键词搜索歌单',
        })[searchType.value],
)
const searchLoading = computed(
    () =>
        ({
            song: songLoading.value,
            artist: artistLoading.value,
            album: albumLoading.value,
            playlist: playlistLoading.value,
        })[searchType.value],
)

// 历史与热搜
const historyStore = useHistoryStore()
const hotKeywords = ref<string[]>([])
const hotLoading = ref(false)

// 下载操作
const { downloadSingle, batchDownload } = useDownloadActions()

// 搜索建议相关
const suggestions = ref<SearchSuggestionData>({
    song: [],
    singer: [],
    album: [],
    mv: [],
})

let abortController: AbortController | null = null
let debounceTimer: ReturnType<typeof setTimeout> | null = null

function cancelSuggestions() {
    if (debounceTimer) {
        clearTimeout(debounceTimer)
        debounceTimer = null
    }
    if (abortController) {
        // IPC 无法取消传输；标记失效，阻止旧响应写入页面。
        abortController.abort()
        abortController = null
    }
}

// 仅用户编辑输入框时进入建议页。
function onKeywordInput(newVal: string) {
    if (newVal === keyword.value) return
    keyword.value = newVal
    // 脚本音源没有搜索建议接口（契约 §3），输入时保留当前结果，不切换到建议页。
    if (isScriptPlatform.value) return
    cancelSuggestions()
    resetSearches()
    const term = newVal.trim()
    pageMode.value = term ? 'suggestions' : 'idle'
    suggestions.value = {
        song: [],
        singer: [],
        album: [],
        mv: [],
    }
    if (!term) return

    const platform = currentPlatform.value
    debounceTimer = setTimeout(async () => {
        debounceTimer = null
        const controller = new AbortController()
        abortController = controller
        try {
            const res = await musicApi.fetchSuggestions(platform, term)
            if (!controller.signal.aborted) {
                suggestions.value = res
            }
        } catch {
            // 输入时已清空建议，请求失败时保持为空。
        } finally {
            if (abortController === controller) {
                abortController = null
            }
        }
    }, 300)
}

// 点击建议项
function onSuggestionSelect(word: string, type: keyof SearchSuggestionData) {
    searchType.value =
        type === 'album' ? 'album' : type === 'singer' ? 'artist' : 'song'
    keyword.value = word
    handleSearch()
}

onBeforeUnmount(cancelSuggestions)

function resetSearches() {
    resetSongSearch()
    resetPlaylistSearch()
    resetAlbumSearch()
    resetArtistSearch()
}

// 获取热搜
async function fetchHotKeywords() {
    // 脚本音源没有热搜接口，避免用 "script:<id>" 请求内置接口。
    if (isScriptPlatform.value) {
        hotKeywords.value = []
        return
    }
    hotLoading.value = true
    try {
        hotKeywords.value = await musicApi.getHotKeywords(currentPlatform.value)
    } catch {
        hotKeywords.value = []
    } finally {
        hotLoading.value = false
    }
}

onMounted(() => {
    fetchHotKeywords()
    void scriptSourceStore.load()
})

// 平台切换
watch(
    currentPlatform,
    () => {
        cancelSuggestions()
        // 脚本音源只支持歌曲搜索，切换过去时回到歌曲分段。
        if (isScriptPlatform.value) searchType.value = 'song'
        fetchHotKeywords()
        suggestions.value = {
            song: [],
            singer: [],
            album: [],
            mv: [],
        }
        resetSearches()
        if (pageMode.value === 'results') void handleSearch()
    },
    { flush: 'sync' },
)

// 切换搜索类型
function switchSearchType(type: SearchType) {
    if (type === searchType.value) return
    cancelSuggestions()
    resetSearches()
    searchType.value = type
    if (pageMode.value === 'results') void handleSearch()
}

// 热搜点击
function onHotClick(word: string) {
    keyword.value = word
    handleSearch()
}

// 历史点击
function onHistorySelect(term: string) {
    keyword.value = term
    handleSearch()
}

function onHistoryRemove(term: string) {
    historyStore.removeHistoryItem(term)
}

// 自定义脚本音源搜索（契约 §3：返回歌曲对象数组）
const SCRIPT_PAGE_SIZE = 20
let scriptPage = 1

// 脚本返回的歌曲对象，字段可能缺失，统一做兜底
interface RawScriptSong {
    id?: string | number
    mid?: string | number
    title?: string
    artist?: string
    album?: string
    cover?: string
    coverUrl?: string
    qualities?: unknown
}

// 品质可能是字符串数组（契约 §3），也可能是 { quality, filename, size } 对象数组
// （内置接口的形状）。对象形状里带着体积信息，保留下来才能在选择器里显示大小。
function scriptQualityItems(value: unknown): QualityItem[] {
    if (!Array.isArray(value)) return []
    const items: QualityItem[] = []
    for (const entry of value) {
        if (typeof entry === 'string') {
            if (entry.length > 0) {
                items.push({ quality: entry, filename: '', size: 0 })
            }
            continue
        }
        if (!entry || typeof entry !== 'object') continue
        const raw = entry as {
            quality?: unknown
            filename?: unknown
            size?: unknown
        }
        if (typeof raw.quality !== 'string' || raw.quality.length === 0)
            continue
        items.push({
            quality: raw.quality,
            filename: typeof raw.filename === 'string' ? raw.filename : '',
            size:
                typeof raw.size === 'number' && Number.isFinite(raw.size)
                    ? raw.size
                    : 0,
        })
    }
    return items
}

// 把脚本返回的歌曲映射为统一的 SongInfo
function toScriptSongInfo(
    raw: unknown,
    platform: string,
    index: number,
    fallbackQualities: string[],
): SongInfo {
    const item = (raw ?? {}) as RawScriptSong
    const rawId = item.id ?? item.mid ?? index + 1
    const numericId = Number(rawId)
    const ownQualities = scriptQualityItems(item.qualities)
    // 歌曲自身没声明品质时，退回音源声明的品质列表（契约 §3）
    const qualities: QualityItem[] =
        ownQualities.length > 0
            ? ownQualities
            : fallbackQualities.map((quality) => ({
                  quality,
                  filename: '',
                  size: 0,
              }))
    return {
        platform,
        id: Number.isFinite(numericId) ? numericId : index + 1,
        mid: String(rawId),
        title: String(item.title ?? ''),
        artist: String(item.artist ?? ''),
        album: String(item.album ?? ''),
        coverUrl: item.cover ?? item.coverUrl ?? '',
        mediaMid: '',
        qualities,
    }
}

// 脚本报错文案：中文可读，且不把 "script:<id>" 机器标识暴露给用户
function scriptErrorText(error: unknown): string {
    const label = currentSourceLabel.value
    const detail = (
        error instanceof Error ? error.message : String(error ?? '')
    )
        .replace(/script:\d+/g, label)
        .trim()
    return detail ? `${label}：${detail}` : `${label}暂时不可用，请稍后重试`
}

// 脚本音源搜索：复用歌曲结果状态与列表组件，分页行为与内置音源一致
async function runScriptSearch(term: string) {
    const sourceId = scriptSourceId.value
    if (sourceId === null) return
    const platform = currentPlatform.value
    const declared = scriptSourceStore.findById(sourceId)?.qualities ?? []

    scriptPage = 1
    songSearchResults.value = []
    songSelectedIds.value = []
    songError.value = null
    songHasMore.value = false
    songHasSearched.value = true
    songLoading.value = true
    try {
        const response = await scriptSourceStore.search(
            sourceId,
            term,
            1,
            SCRIPT_PAGE_SIZE,
        )
        songSearchResults.value = response.songs.map((raw, index) =>
            toScriptSongInfo(raw, platform, index, declared),
        )
        songHasMore.value = Boolean(response.has_more)
    } catch (error) {
        songError.value = scriptErrorText(error)
    } finally {
        songLoading.value = false
    }
}

async function loadMoreScriptSongs() {
    const sourceId = scriptSourceId.value
    if (
        sourceId === null ||
        songLoading.value ||
        songLoadingMore.value ||
        !songHasMore.value
    ) {
        return
    }
    const platform = currentPlatform.value
    const declared = scriptSourceStore.findById(sourceId)?.qualities ?? []
    const nextPage = scriptPage + 1
    songLoadingMore.value = true
    try {
        const response = await scriptSourceStore.search(
            sourceId,
            keyword.value.trim(),
            nextPage,
            SCRIPT_PAGE_SIZE,
        )
        const known = new Set(songSearchResults.value.map((song) => song.mid))
        const appended = response.songs
            .map((raw, index) =>
                toScriptSongInfo(raw, platform, index, declared),
            )
            .filter((song) => !known.has(song.mid))
        songSearchResults.value = [...songSearchResults.value, ...appended]
        songHasMore.value = Boolean(response.has_more)
        scriptPage = nextPage
    } catch (error) {
        songError.value = scriptErrorText(error)
    } finally {
        songLoadingMore.value = false
    }
}

// 统一搜索入口
async function handleSearch() {
    cancelSuggestions()
    const term = keyword.value.trim()
    if (!term) {
        pageMode.value = 'idle'
        resetSearches()
        return
    }

    pageMode.value = 'results'
    suggestions.value = { song: [], singer: [], album: [], mv: [] }
    historyStore.addHistory(term)
    if (isScriptPlatform.value) {
        await runScriptSearch(term)
    } else if (searchType.value === 'song') {
        await searchSongFunc(currentPlatform.value, term)
    } else if (searchType.value === 'artist') {
        await searchArtistFunc(currentPlatform.value, term)
    } else if (searchType.value === 'album') {
        await searchAlbumFunc(currentPlatform.value, term)
    } else if (searchType.value === 'playlist') {
        await searchPlaylistFunc(currentPlatform.value, term)
    }
}

// 加载更多歌曲
function loadMoreSongs() {
    if (isScriptPlatform.value) {
        void loadMoreScriptSongs()
        return
    }
    loadMoreSongsFunc(currentPlatform.value, keyword.value)
}

// 加载更多歌单
function loadMorePlaylists() {
    loadMorePlaylistFunc(currentPlatform.value, keyword.value)
}

// 记录搜索页的完整地址，歌单详情返回时才能恢复当前搜索结果。
function goToPlaylist(pl: PlaylistSearchItem) {
    router.push({
        path: '/playlist',
        query: {
            platform: currentPlatform.value,
            id: pl.id,
        },
        state: { musicReturnTo: route.fullPath },
    })
}

// 单曲下载
function onSingleDownload(song: SongInfo) {
    downloadSingle(song)
}

// 批量下载
function onBatchDownload() {
    const songs = songSearchResults.value.filter((s) =>
        songSelectedIds.value.includes(s.mid),
    )
    if (songs.length > 0) {
        batchDownload(songs)
    }
}
</script>

<style scoped>
.search-view {
    display: flex;
    flex-direction: column;
    min-width: 0;
    /* 防止底部导航遮挡 */
    min-height: 100%;
    padding-bottom: 0;
}

.search-header {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
    margin-bottom: var(--md-space-4);
    min-width: 0;
}

/* 搜索栏和类型切换的间距由 search-header 统一控制 */
.search-header :deep(.search-bar) {
    margin-bottom: 0;
}

/* M3 segmented button：整组等宽、外轮廓胶囊、内部分隔用描边。 */
.type-switch {
    min-width: 0;
}

.type-switch :deep(.n-radio-group) {
    display: flex;
    width: 100%;
    min-width: 0;
}

.type-switch :deep(.n-radio-button) {
    flex: 1 1 0;
    min-width: 0;
    min-height: var(--md-target-min);
    padding: 0 var(--md-space-2);
}

.type-switch :deep(.n-radio-button__label) {
    display: flex;
    justify-content: center;
    min-width: 0;
}

/*
 * 选中态用填充表达（M3 segmented button），不叠加下划线：
 * Naive 默认把选中段的下边框涂成主色，这里改回中性描边，
 * 只保留 secondary-container 实底填充 + on-secondary-container 文字。
 * Naive 的 --n-* 变量是内联样式，类选择器覆盖不动，必须写真实 CSS 属性。
 */
.type-switch :deep(.n-radio-button--checked) {
    background-color: var(--md-secondary-container);
    border-color: var(--md-outline-variant);
    box-shadow: none;
}

.type-switch :deep(.n-radio-button--checked .n-radio__label) {
    color: var(--md-on-secondary-container);
}

.type-switch :deep(.n-radio-button--checked .n-radio-button__state-border) {
    box-shadow: none;
}

.segment-content {
    display: inline-flex;
    align-items: center;
    gap: var(--md-space-1);
    min-width: 0;
}

/* 勾选标记只在选中段显示；未选中保留占位，避免文字左右跳动。 */
.segment-check {
    flex-shrink: 0;
    visibility: hidden;
    opacity: 0;
}

.segment-check.is-visible {
    visibility: visible;
    opacity: 1;
}

.segment-label {
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    font-weight: var(--md-weight-medium);
    white-space: nowrap;
}

.loading-wrapper {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--md-space-3);
    padding: var(--md-space-10) 0;
}

/* 空闲引导：撑满剩余高度并居中，消除下半屏留白。 */
.idle-guide {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--md-space-3);
    min-height: var(--md-space-10);
    padding: var(--md-space-6) var(--md-space-4) var(--md-space-10);
    text-align: center;
}

.idle-guide-icon {
    width: var(--md-space-10);
    height: var(--md-space-10);
    color: var(--md-outline);
}

.idle-guide-text {
    margin: 0;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
}

/*
 * 矮屏（如 320×568）：热搜已经占满可视高度，没有可吸收的剩余空间，
 * 而 .search-view 的 min-height: 100% 会把被底部导航遮住的那段也算进来，
 * 若继续 flex-grow，引导语就会被导航条压住。这里改为只占内容高度。
 */
@media (max-height: 640px) {
    .idle-guide {
        flex: 0 0 auto;
        gap: var(--md-space-2);
        padding: var(--md-space-2) var(--md-space-4) 0;
    }

    .idle-guide-icon {
        display: none;
    }
}

.search-error {
    margin-bottom: var(--md-space-4);
}

/* 屏幕阅读器专用文本：为加载状态提供可朗读名称。 */
.visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    border: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
}
</style>

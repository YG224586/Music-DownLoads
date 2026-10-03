<template>
    <div class="artist-view">
        <n-button quaternary class="back-btn" @click="goBack">
            <span class="back-content">
                <svg
                    class="back-icon"
                    viewBox="0 0 24 24"
                    width="20"
                    height="20"
                    aria-hidden="true"
                >
                    <path
                        d="M19 12H5m0 0 6-6m-6 6 6 6"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    />
                </svg>
                <span>{{ backLabel }}</span>
            </span>
        </n-button>
        <n-alert
            v-if="routeError"
            type="error"
            title="无法打开歌手"
            class="view-error"
            >{{ routeError }}</n-alert
        >
        <template v-else-if="artist">
            <div class="artist-header">
                <img
                    v-if="artist.coverUrl"
                    :src="artist.coverUrl"
                    class="artist-avatar"
                    alt="歌手头像"
                />
                <div v-else class="artist-avatar artist-avatar--empty" />
                <div class="artist-info">
                    <h2 class="artist-name">{{ artist.name || '歌手' }}</h2>
                    <p v-if="artist.alias || artist.region" class="artist-line">
                        {{
                            [artist.alias, artist.region]
                                .filter(Boolean)
                                .join(' · ')
                        }}
                    </p>
                    <p class="artist-line">
                        {{ songPage.total.value ?? artist.songCount }} 首歌曲 ·
                        {{ albumPage.total.value ?? artist.albumCount }} 张专辑
                    </p>
                </div>
            </div>
            <!-- 内容分区切换：互斥两段，用 segmented button（选中段带勾选标记） -->
            <div class="artist-tabs">
                <n-radio-group
                    :value="tab"
                    class="tabs-group"
                    aria-label="歌手内容分区"
                    @update:value="selectTab"
                >
                    <n-radio-button value="songs" class="tab-segment">
                        <span class="segment-content">
                            <svg
                                class="segment-check"
                                :class="{ 'is-visible': tab === 'songs' }"
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
                            <span class="segment-label">歌曲</span>
                        </span>
                    </n-radio-button>
                    <n-radio-button value="albums" class="tab-segment">
                        <span class="segment-content">
                            <svg
                                class="segment-check"
                                :class="{ 'is-visible': tab === 'albums' }"
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
                            <span class="segment-label">专辑</span>
                        </span>
                    </n-radio-button>
                </n-radio-group>
            </div>
            <template v-if="tab === 'songs'">
                <n-alert
                    v-if="songPage.error.value"
                    type="error"
                    title="获取歌手歌曲失败"
                    class="view-error"
                >
                    {{ songPage.error.value }}
                    <n-button @click="songPage.loadMore">重试</n-button>
                </n-alert>
                <div
                    v-if="songPage.loading.value && !songPage.loaded.value"
                    class="loading"
                >
                    <n-spin />
                    <span class="visually-hidden" role="status"
                        >正在加载歌手歌曲…</span
                    >
                </div>
                <SearchResultList
                    v-if="songPage.items.value.length"
                    :songs="songPage.items.value"
                    v-model:selectedIds="selectedIds"
                    @download="downloadSingle"
                    @click-artist="openRelatedArtist"
                    @click-album="openSongAlbum"
                />
                <n-empty
                    v-else-if="songPage.loaded.value && !songPage.error.value"
                    description="暂无可用歌曲"
                />
                <LoadMoreButton
                    v-if="songPage.hasMore.value"
                    :loading="songPage.loading.value"
                    :disabled="songPage.loading.value"
                    @click="songPage.loadMore"
                />
                <BatchDownloadBar
                    v-if="selectedIds.length"
                    :selected-count="selectedIds.length"
                    @batch-download="downloadSelected"
                />
            </template>
            <template v-else>
                <n-alert
                    v-if="albumPage.error.value"
                    type="error"
                    title="获取歌手专辑失败"
                    class="view-error"
                >
                    {{ albumPage.error.value }}
                    <n-button @click="albumPage.loadMore">重试</n-button>
                </n-alert>
                <div
                    v-if="albumPage.loading.value && !albumPage.loaded.value"
                    class="loading"
                >
                    <n-spin />
                    <span class="visually-hidden" role="status"
                        >正在加载歌手专辑…</span
                    >
                </div>
                <AlbumSearchResult
                    v-if="albumPage.loaded.value"
                    :albums="albumPage.items.value"
                    :platform="platform"
                    :has-more="false"
                    :loading-more="false"
                    @click-album="(album) => openAlbum(platform, album)"
                    @click-artist="openRelatedArtist"
                />
                <LoadMoreButton
                    v-if="albumPage.hasMore.value"
                    :loading="albumPage.loading.value"
                    :disabled="albumPage.loading.value"
                    @click="albumPage.loadMore"
                />
            </template>
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import {
    NAlert,
    NButton,
    NEmpty,
    NSpin,
    NRadioGroup,
    NRadioButton,
} from 'naive-ui'
import type { ArtistInfo, AlbumInfo, SongInfo } from '../types'
import { fetchArtistSongs, fetchArtistAlbums } from '../api/musicApi'
import { usePagedList } from '../composables/usePagedList'
import { useDownloadActions } from '../composables/useDownloadActions'
import SearchResultList from '../components/search/SearchResultList.vue'
import AlbumSearchResult from '../components/search/AlbumSearchResult.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import LoadMoreButton from '../components/search/LoadMoreButton.vue'
import { useMusicNavigation } from '../composables/useMusicNavigation'

const route = useRoute()
const { openAlbum, openRelatedArtist, openSongAlbum, goBack, backLabel } =
    useMusicNavigation()
// 每个缓存的详情实例使用创建时的查询参数。
const query = { ...route.query }
const platform = typeof query.platform === 'string' ? query.platform : ''
const artist = ref<ArtistInfo | null>(null)
const tab = ref<'songs' | 'albums'>('songs')
const selectedIds = ref<string[]>([])
const routeError = ref('')
const songPage = usePagedList<SongInfo>((song) => song.mid)
const albumPage = usePagedList<AlbumInfo>((album) => album.id)
const { downloadSingle, batchDownload } = useDownloadActions()
let albumsStarted = false

onMounted(() => {
    if (
        typeof query.id !== 'string' ||
        !query.id ||
        !['qqmusic', 'kuwo'].includes(platform)
    ) {
        routeError.value = '缺少有效的歌手信息，请返回搜索重新选择'
        return
    }
    const id = query.id
    const text = (key: string) =>
        typeof query[key] === 'string' ? (query[key] as string) : ''
    const count = (key: string) => Math.max(0, Number(text(key)) || 0)
    artist.value = {
        id,
        name: text('name'),
        coverUrl: text('cover'),
        alias: text('alias'),
        region: text('region'),
        songCount: count('songs'),
        albumCount: count('albums'),
    }
    void songPage.start(async (page) => {
        const result = await fetchArtistSongs(platform, id, page)
        return {
            items: result.songs,
            total: result.total,
            hasMore: result.has_more,
        }
    })
})

function showAlbums() {
    tab.value = 'albums'
    if (albumsStarted || !artist.value) return
    albumsStarted = true
    const id = artist.value.id
    void albumPage.start(async (page) => {
        const result = await fetchArtistAlbums(platform, id, page)
        return {
            items: result.albums,
            total: result.total,
            hasMore: result.has_more,
        }
    })
}

// 分段按钮只是切换视图；专辑分区首次进入时才发起请求。
function selectTab(value: string | number | boolean) {
    if (value === 'albums') {
        showAlbums()
    } else {
        tab.value = 'songs'
    }
}

function downloadSelected() {
    batchDownload(
        songPage.items.value.filter((song) =>
            selectedIds.value.includes(song.mid),
        ),
    )
}
</script>

<style scoped>
.artist-view {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-5);
    min-width: 0;
}

/* 返回是导航而非动作：用文字按钮，不做填充主操作。 */
.back-btn.n-button {
    align-self: flex-start;
    padding: 0 var(--md-space-2);
    --n-text-color: var(--md-primary);
    --n-text-color-hover: var(--md-primary);
    --n-text-color-pressed: var(--md-primary);
    --n-text-color-focus: var(--md-primary);
    --n-color-hover: var(--md-surface-container-high);
    --n-color-pressed: var(--md-surface-container-highest);
    --n-color-focus: var(--md-surface-container-high);
}

.back-content {
    display: inline-flex;
    align-items: center;
    gap: var(--md-space-1);
}

.back-icon {
    flex-shrink: 0;
}

.artist-header {
    display: flex;
    align-items: center;
    gap: var(--md-space-4);
    min-width: 0;
}

.artist-avatar {
    width: 96px;
    height: 96px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: var(--md-shape-full);
    background-color: var(--md-surface-container-high);
}

.artist-avatar--empty {
    border: 1px solid var(--md-outline-variant);
}

.artist-info {
    min-width: 0;
    overflow-wrap: anywhere;
}

.artist-name {
    margin: 0 0 var(--md-space-1);
    font-size: var(--md-headline-small);
    line-height: var(--md-headline-small-line);
    /* M3 Expressive：headline 系列统一走 emphasis 字重（700），与技术页标题一致。 */
    font-weight: var(--md-headline-weight);
    color: var(--md-on-surface);
}

.artist-line {
    margin: 0;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
}

.artist-line + .artist-line {
    margin-top: 2px;
}

/* 分段按钮：整组等宽，选中段用勾选标记 + 颜色双重表达。 */
.artist-tabs {
    min-width: 0;
}

.artist-tabs :deep(.n-radio-group) {
    display: flex;
    width: 100%;
    min-width: 0;
}

.artist-tabs :deep(.n-radio-button) {
    flex: 1 1 0;
    min-width: 0;
    min-height: var(--md-target-min);
    padding: 0 var(--md-space-2);
}

.artist-tabs :deep(.n-radio-button__label) {
    display: flex;
    justify-content: center;
    min-width: 0;
}

.segment-content {
    display: inline-flex;
    align-items: center;
    gap: var(--md-space-1);
    min-width: 0;
}

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

.loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--md-space-3);
    padding: var(--md-space-10) 0;
}

.view-error {
    margin-bottom: 0;
}

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

@media (max-width: 599px) {
    .artist-avatar {
        width: 80px;
        height: 80px;
    }
}
</style>

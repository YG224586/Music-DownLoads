<template>
    <div class="album-view">
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
        <div v-if="loading" class="loading" role="status">
            <n-spin size="medium" description="正在获取专辑全部歌曲…" />
        </div>
        <n-alert
            v-else-if="error"
            type="error"
            title="获取专辑失败"
            class="view-error"
        >
            {{ error }}
            <n-button @click="loadAlbum">重试</n-button>
        </n-alert>
        <template v-else-if="album">
            <div class="album-header">
                <img
                    v-if="album.coverUrl"
                    :src="album.coverUrl"
                    class="album-cover"
                    alt="专辑封面"
                />
                <div v-else class="album-cover album-cover--empty" />
                <div class="album-info">
                    <h2 class="album-name">{{ album.name || '专辑' }}</h2>
                    <p class="album-artist">
                        <ArtistNames
                            :platform="platform"
                            :artists="album.artists"
                            :fallback="album.artist"
                            @click-artist="openRelatedArtist"
                        />
                    </p>
                    <p class="album-meta">
                        {{ album.songCount }} 首<span v-if="album.publishDate">
                            · {{ album.publishDate }}</span
                        >
                    </p>
                </div>
            </div>
            <SearchResultList
                v-if="songs.length"
                :songs="songs"
                v-model:selectedIds="selectedIds"
                @download="downloadSingle"
                @click-artist="openRelatedArtist"
                @click-album="openSongAlbum"
            />
            <n-empty v-else description="该专辑暂无可用歌曲" />
            <BatchDownloadBar
                v-if="selectedIds.length"
                :selected-count="selectedIds.length"
                @batch-download="downloadSelected"
            />
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { NButton, NSpin, NAlert, NEmpty } from 'naive-ui'
import type { AlbumInfo, SongInfo } from '../types'
import { fetchAlbumSongs } from '../api/musicApi'
import { useDownloadActions } from '../composables/useDownloadActions'
import SearchResultList from '../components/search/SearchResultList.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import ArtistNames from '../components/search/ArtistNames.vue'
import { useMusicNavigation } from '../composables/useMusicNavigation'

const route = useRoute()
const { openRelatedArtist, openSongAlbum, goBack, backLabel } =
    useMusicNavigation()
const query = { ...route.query }
const platform = typeof query.platform === 'string' ? query.platform : ''
const album = ref<AlbumInfo | null>(null)
const songs = ref<SongInfo[]>([])
const selectedIds = ref<string[]>([])
const loading = ref(false)
const error = ref('')
const { downloadSingle, batchDownload } = useDownloadActions()
async function loadAlbum() {
    if (loading.value) return
    album.value = null
    songs.value = []
    selectedIds.value = []
    error.value = ''
    loading.value = true
    try {
        if (typeof query.platform !== 'string' || typeof query.id !== 'string')
            throw new Error('缺少专辑信息，请返回搜索重新选择')
        const result = await fetchAlbumSongs(query.platform, query.id)
        // 后端可能返回 200 但缺少 album 字段（畸形响应）；缺字段时回退到路由 query，
        // 与 musicApi.ts 的 songs 兜底配套，避免 TypeError 文本直接暴露给用户
        album.value = {
            ...result.album,
            name:
                result.album?.name ||
                (typeof query.name === 'string' ? query.name : ''),
            artist:
                result.album?.artist ||
                (typeof query.artist === 'string' ? query.artist : ''),
            publishDate:
                result.album?.publishDate ||
                (typeof query.date === 'string' ? query.date : ''),
        }
        songs.value = result.songs
    } catch (e) {
        error.value = String(e)
    } finally {
        loading.value = false
    }
}

onMounted(loadAlbum)

function downloadSelected() {
    batchDownload(
        songs.value.filter((song) => selectedIds.value.includes(song.mid)),
    )
}
</script>

<style scoped>
.album-view {
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

.loading {
    display: flex;
    justify-content: center;
    padding: var(--md-space-10) 0;
}

.album-header {
    display: flex;
    align-items: center;
    gap: var(--md-space-4);
    min-width: 0;
}

.album-cover {
    width: 112px;
    height: 112px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: var(--md-shape-md);
    background-color: var(--md-surface-container-high);
}

.album-cover--empty {
    border: 1px solid var(--md-outline-variant);
}

.album-info {
    min-width: 0;
    overflow-wrap: anywhere;
}

.album-name {
    margin: 0 0 var(--md-space-2);
    font-size: var(--md-headline-small);
    line-height: var(--md-headline-small-line);
    /* M3 Expressive：headline 系列统一走 emphasis 字重（700），与技术页标题一致。 */
    font-weight: var(--md-headline-weight);
    color: var(--md-on-surface);
}

.album-artist {
    margin: 0;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
}

/*
 * 专辑头部的歌手链接独占一行，不适用 WCAG 2.5.8 的「行内目标」例外，
 * 因此在本地作用域把命中区抬到 48dp；上下各 -12px 负外边距抵消增高，
 * 文本基线与行高保持不变（头部视觉高度不变）。
 * ArtistNames.vue 中供列表行内复用的 24px 语义不受影响。
 */
.album-artist :deep(.artist-link.n-button) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: var(--md-target-min);
    min-height: var(--md-target-min);
    margin-top: -12px;
    margin-bottom: -12px;
}

.album-meta {
    margin: 2px 0 0;
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
}

.view-error {
    margin-bottom: 0;
}

@media (max-width: 599px) {
    .album-cover {
        width: 88px;
        height: 88px;
    }
}
</style>

<template>
    <div class="playlist-detail">
        <!-- 返回是文字按钮 + 前置箭头，命中区由全局 48px 规则保证 -->
        <n-button class="back-button" quaternary @click="emit('back')">
            <span class="back-icon" aria-hidden="true" v-html="BACK_ICON" />{{
                backLabel
            }}
        </n-button>

        <div v-if="loading" class="loading-wrapper">
            <n-spin size="medium" />
        </div>
        <n-alert v-else-if="error" type="error" title="加载歌单失败">
            {{ error }}
            <n-button v-if="retryable" @click="emit('retry')">重试</n-button>
        </n-alert>
        <template v-else-if="playlist">
            <div class="playlist-info">
                <img
                    v-if="playlist.coverUrl"
                    :src="playlist.coverUrl"
                    class="playlist-cover"
                    alt="歌单封面"
                />
                <div class="playlist-details">
                    <div class="playlist-name">{{ playlist.name }}</div>
                    <div v-if="playlist.creator" class="playlist-creator">
                        创建者：{{ playlist.creator }}
                    </div>
                    <div class="playlist-meta">
                        歌曲数：{{ playlist.songCount }} · 播放量：{{
                            formatPlayCount(playlist.playCount)
                        }}
                    </div>
                </div>
            </div>

            <template v-if="songs.length">
                <div class="list-header">
                    <n-checkbox
                        :checked="isAllSelected"
                        :indeterminate="isIndeterminate"
                        @update:checked="
                            (checked) => emit('toggle-all', checked)
                        "
                    >
                        全选
                    </n-checkbox>
                    <span class="count-text">
                        已选 {{ selectedIds.length }} / {{ songs.length }} 首
                    </span>
                </div>
                <div class="song-items">
                    <SongItem
                        v-for="song in songs"
                        :key="song.mid"
                        :song="song"
                        :selected="selectedIds.includes(song.mid)"
                        @toggle-select="
                            (selected) =>
                                emit('toggle-select', song.mid, selected)
                        "
                        @download="(item) => emit('download', item)"
                        @click-artist="
                            (platform, artist) =>
                                emit('click-artist', platform, artist)
                        "
                        @click-album="(item) => emit('click-album', item)"
                    />
                </div>
                <BatchDownloadBar
                    v-if="selectedIds.length > 0"
                    :selected-count="selectedIds.length"
                    @batch-download="emit('batch-download')"
                />
            </template>
            <n-empty v-else description="该歌单暂无可用歌曲" />
        </template>
    </div>
</template>

<script setup lang="ts">
import { NAlert, NButton, NCheckbox, NEmpty, NSpin } from 'naive-ui'
import SongItem from '../search/SongItem.vue'
import BatchDownloadBar from '../search/BatchDownloadBar.vue'
import type { ArtistReference, PlaylistInfo, SongInfo } from '../../types'
import { formatPlayCount } from '../../utils/format'

defineProps<{
    backLabel: string
    loading: boolean
    error: string
    retryable: boolean
    playlist: PlaylistInfo | null
    songs: SongInfo[]
    selectedIds: string[]
    isAllSelected: boolean
    isIndeterminate: boolean
}>()

const emit = defineEmits<{
    (e: 'back'): void
    (e: 'retry'): void
    (e: 'toggle-all', checked: boolean): void
    (e: 'toggle-select', songMid: string, selected: boolean): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
    (e: 'batch-download'): void
}>()

/** 返回箭头为纯装饰，按钮的可访问名来自 backLabel 文字 */
const BACK_ICON =
    '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M14.5 6 9 12l5.5 6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>'
</script>

<style scoped>
/* 详情页纵向排布：信息卡 → 选择栏 → 歌曲列表 → 批量下载条 */
.playlist-detail {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-4);
    min-width: 0;
}

.back-button {
    align-self: flex-start;
    /* 文字按钮左内边距去掉后与页面左边缘对齐，命中区仍为 48dp 高 */
    margin-left: calc(var(--md-space-3) * -1);
}

.back-icon {
    display: inline-flex;
    margin-right: var(--md-space-1);
}

.loading-wrapper {
    display: flex;
    justify-content: center;
    padding: var(--md-space-8) 0;
}

.playlist-detail :deep(.n-alert) {
    border-radius: var(--md-shape-md);
}

/* 歌单信息卡：封面 + 名称/创建者/统计 */
.playlist-info {
    display: flex;
    gap: var(--md-space-4);
    align-items: center;
    padding: var(--md-space-4);
    min-width: 0;
    background-color: var(--md-surface-container-low);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-lg);
}

.playlist-cover {
    width: 96px;
    height: 96px;
    border-radius: var(--md-shape-md);
    object-fit: cover;
    flex-shrink: 0;
    /* 封面加载失败时仍是可辨识的占位块，避免出现破图 */
    background-color: var(--md-surface-container-high);
    color: var(--md-on-surface-variant);
    font-size: var(--md-label-small);
    line-height: var(--md-label-small-line);
}

.playlist-details {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
}

.playlist-name {
    font-size: var(--md-title-large);
    line-height: var(--md-title-large-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
}

.playlist-creator {
    margin-top: var(--md-space-1);
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
}

.playlist-meta {
    margin-top: var(--md-space-1);
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
}

/* 全选与已选计数：勾选框自带 48dp 命中区 */
.list-header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--md-space-3);
    min-width: 0;
    padding: 0 var(--md-space-1);
}

.count-text {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    font-variant-numeric: tabular-nums;
}

.song-items {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-2);
    min-width: 0;
}

/* 手机端信息卡收紧：封面 80dp，名称降一级 */
@media (max-width: 767px) {
    .playlist-info {
        align-items: flex-start;
        gap: var(--md-space-3);
        padding: var(--md-space-3);
    }

    .playlist-cover {
        width: 80px;
        height: 80px;
    }

    .playlist-name {
        font-size: var(--md-title-medium);
        line-height: var(--md-title-medium-line);
    }
}
</style>

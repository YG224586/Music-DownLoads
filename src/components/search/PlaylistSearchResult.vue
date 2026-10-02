<template>
    <div class="playlist-search-result">
        <template v-if="playlists.length > 0">
            <div class="playlist-row-list">
                <div
                    v-for="pl in playlists"
                    :key="`${pl.id}:${pl.dirid ?? ''}`"
                    class="playlist-row"
                    role="button"
                    tabindex="0"
                    :aria-label="`打开歌单 ${pl.name}`"
                    @click="$emit('click-playlist', pl)"
                    @keydown.enter="$emit('click-playlist', pl)"
                    @keydown.space.prevent="$emit('click-playlist', pl)"
                >
                    <img
                        v-if="pl.coverUrl"
                        :src="pl.coverUrl"
                        class="playlist-cover"
                        alt="歌单封面"
                    />
                    <div v-else class="playlist-cover playlist-cover--empty" />
                    <div class="playlist-info">
                        <div class="playlist-name">{{ pl.name }}</div>
                        <div v-if="pl.creator" class="playlist-creator">
                            {{ pl.creator }}
                        </div>
                        <div class="playlist-meta">
                            {{ pl.songCount }} 首 ·
                            {{ formatPlayCount(pl.playCount) }}
                        </div>
                        <div v-if="pl.createdAt" class="playlist-date">
                            创建：{{ formatUnixTime(pl.createdAt) }}
                        </div>
                        <div v-if="pl.updatedAt" class="playlist-date">
                            更新：{{ formatUnixTime(pl.updatedAt) }}
                        </div>
                    </div>
                    <svg
                        class="row-arrow"
                        viewBox="0 0 24 24"
                        width="20"
                        height="20"
                        aria-hidden="true"
                    >
                        <path
                            d="m9 5 7 7-7 7"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        />
                    </svg>
                </div>
            </div>
            <LoadMoreButton
                v-if="hasMore"
                :loading="loadingMore"
                :disabled="loadingMore"
                @click="$emit('load-more')"
            />
        </template>
        <div v-else class="empty-result">
            <n-empty :description="emptyDescription ?? '未找到相关歌单'" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { NEmpty } from 'naive-ui'
import type { PlaylistSearchItem } from '../../types'
import LoadMoreButton from './LoadMoreButton.vue'
import { formatPlayCount, formatUnixTime } from '../../utils/format'

defineProps<{
    playlists: PlaylistSearchItem[]
    hasMore: boolean
    loadingMore: boolean
    emptyDescription?: string
}>()

defineEmits<{
    (e: 'click-playlist', playlist: PlaylistSearchItem): void
    (e: 'load-more'): void
}>()
</script>

<style scoped>
.playlist-row-list {
    display: flex;
    flex-direction: column;
}

/* 整行是一个导航目标：role=button + Enter/Space 键盘可达。 */
.playlist-row {
    display: flex;
    align-items: center;
    gap: var(--md-space-3);
    min-width: 0;
    min-height: var(--md-target-min);
    padding: var(--md-space-3) 0;
    border-top: 1px solid var(--md-outline-variant);
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.playlist-row:first-child {
    border-top: none;
}

.playlist-row:hover {
    background-color: var(--md-surface-container-high);
}

.playlist-row:focus-visible {
    outline: 3px solid var(--md-primary);
    outline-offset: -3px;
    border-radius: var(--md-shape-sm);
}

.playlist-cover {
    width: 56px;
    height: 56px;
    border-radius: var(--md-shape-md);
    object-fit: cover;
    flex-shrink: 0;
    background-color: var(--md-surface-container-high);
}

.playlist-cover--empty {
    border: 1px solid var(--md-outline-variant);
}

.playlist-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.playlist-name {
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.playlist-creator {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    overflow-wrap: anywhere;
}

.playlist-meta {
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    overflow-wrap: anywhere;
}

.playlist-date {
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    overflow-wrap: anywhere;
}

.row-arrow {
    flex-shrink: 0;
    color: var(--md-on-surface-variant);
}

.empty-result {
    display: flex;
    justify-content: center;
    padding: var(--md-space-10) var(--md-space-4);
}

@media (prefers-reduced-motion: reduce) {
    .playlist-row {
        transition: none;
    }
}
</style>

<template>
    <div class="playlist-search-result">
        <template v-if="playlists.length > 0">
            <div class="playlist-card-list">
                <div
                    v-for="pl in playlists"
                    :key="`${pl.id}:${pl.dirid ?? ''}`"
                    class="playlist-card"
                    role="button"
                    tabindex="0"
                    @click="$emit('click-playlist', pl)"
                    @keydown.enter="$emit('click-playlist', pl)"
                    @keydown.space.prevent="$emit('click-playlist', pl)"
                >
                    <img
                        v-if="pl.coverUrl"
                        :src="pl.coverUrl"
                        class="playlist-card-cover"
                        alt="歌单封面"
                    />
                    <div class="playlist-card-info">
                        <div class="playlist-card-name">{{ pl.name }}</div>
                        <div v-if="pl.creator" class="playlist-card-creator">
                            {{ pl.creator }}
                        </div>
                        <div class="playlist-card-meta">
                            {{ pl.songCount }} 首 ·
                            {{ formatPlayCount(pl.playCount) }}
                        </div>
                        <div v-if="pl.createdAt" class="playlist-card-date">
                            创建：{{ formatUnixTime(pl.createdAt) }}
                        </div>
                        <div v-if="pl.updatedAt" class="playlist-card-date">
                            更新：{{ formatUnixTime(pl.updatedAt) }}
                        </div>
                    </div>
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
.playlist-card-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.playlist-card {
    display: flex;
    gap: 12px;
    align-items: center;
    min-width: 0;
    padding: 12px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    cursor: pointer;
    transition: border-color 0.2s;
}

.playlist-card:hover {
    border-color: var(--color-text-secondary);
}

.playlist-card:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 2px;
}

.playlist-card-cover {
    width: 60px;
    height: 60px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
}

.playlist-card-info {
    flex: 1;
    min-width: 0;
}

.playlist-card-name {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.playlist-card-creator {
    overflow-wrap: anywhere;
    color: var(--color-text-secondary);
    font-size: 13px;
    margin-top: 2px;
}

.playlist-card-meta {
    overflow-wrap: anywhere;
    color: var(--color-text-secondary);
    font-size: 12px;
    margin-top: 2px;
}

.playlist-card-date {
    overflow-wrap: anywhere;
    color: var(--color-text-secondary);
    font-size: 12px;
    margin-top: 2px;
}

.empty-result {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}
</style>

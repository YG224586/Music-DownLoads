<template>
    <div class="artist-search-result">
        <template v-if="artists.length > 0">
            <div class="artist-row-list">
                <button
                    type="button"
                    v-for="pl in artists"
                    :key="pl.id"
                    class="artist-row"
                    @click="$emit('click-artist', pl)"
                >
                    <img
                        v-if="pl.coverUrl"
                        :src="pl.coverUrl"
                        class="artist-cover"
                        alt="歌手封面"
                    />
                    <div v-else class="artist-cover artist-cover--empty" />
                    <div class="artist-info">
                        <div class="artist-name">{{ pl.name }}</div>
                        <div class="artist-sub">
                            {{
                                [pl.alias, pl.region]
                                    .filter(Boolean)
                                    .join(' · ')
                            }}
                        </div>
                        <div class="artist-meta">
                            {{ pl.songCount }} 首歌曲 ·
                            {{ pl.albumCount }} 张专辑
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
                </button>
            </div>
            <LoadMoreButton
                v-if="hasMore"
                :loading="loadingMore"
                :disabled="loadingMore"
                @click="$emit('load-more')"
            />
        </template>
        <div v-else class="empty-result">
            <n-empty description="未找到相关歌手" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { NEmpty } from 'naive-ui'
import type { ArtistInfo } from '../../types'
import LoadMoreButton from './LoadMoreButton.vue'

defineProps<{
    artists: ArtistInfo[]
    hasMore: boolean
    loadingMore: boolean
}>()

defineEmits<{
    (e: 'click-artist', artist: ArtistInfo): void
    (e: 'load-more'): void
}>()
</script>

<style scoped>
/* 歌手结果是可点击的 list item：整行命中，不做行行套卡。 */
.artist-row-list {
    display: flex;
    flex-direction: column;
}

.artist-row {
    display: flex;
    align-items: center;
    gap: var(--md-space-3);
    width: 100%;
    min-width: 0;
    min-height: var(--md-target-min);
    padding: var(--md-space-3) 0;
    border: none;
    border-top: 1px solid var(--md-outline-variant);
    background: transparent;
    color: var(--md-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.artist-row:first-child {
    border-top: none;
}

.artist-row:hover {
    background-color: var(--md-surface-container-high);
}

.artist-cover {
    width: 56px;
    height: 56px;
    border-radius: var(--md-shape-full);
    object-fit: cover;
    flex-shrink: 0;
    background-color: var(--md-surface-container-high);
}

.artist-cover--empty {
    border: 1px solid var(--md-outline-variant);
}

.artist-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.artist-name {
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.artist-sub {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    overflow-wrap: anywhere;
}

.artist-meta {
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
    .artist-row {
        transition: none;
    }
}
</style>

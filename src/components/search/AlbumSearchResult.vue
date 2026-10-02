<template>
    <div class="album-search-result">
        <template v-if="albums.length > 0">
            <div class="album-row-list">
                <div
                    v-for="album in albums"
                    :key="album.id"
                    class="album-row"
                    @click="$emit('click-album', album)"
                >
                    <img
                        v-if="album.coverUrl"
                        :src="album.coverUrl"
                        class="album-cover"
                        alt="专辑封面"
                    />
                    <div v-else class="album-cover album-cover--empty" />
                    <div class="album-info">
                        <div class="album-name">
                            <n-button
                                text
                                size="small"
                                class="album-name-link inline-link"
                                :aria-label="`打开专辑 ${album.name}`"
                                @click.stop="$emit('click-album', album)"
                                >{{ album.name }}</n-button
                            >
                        </div>
                        <div class="album-creator">
                            <ArtistNames
                                :platform="platform"
                                :artists="album.artists"
                                :fallback="album.artist"
                                @click-artist="
                                    (platform, artist) =>
                                        $emit('click-artist', platform, artist)
                                "
                            />
                        </div>
                        <div class="album-meta">
                            {{ album.songCount }} 首<span
                                v-if="album.publishDate"
                            >
                                · {{ album.publishDate }}</span
                            >
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
            <n-empty description="未找到相关专辑" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { NButton, NEmpty } from 'naive-ui'
import type { AlbumInfo, ArtistReference } from '../../types'
import LoadMoreButton from './LoadMoreButton.vue'
import ArtistNames from './ArtistNames.vue'

defineProps<{
    platform: string
    albums: AlbumInfo[]
    hasMore: boolean
    loadingMore: boolean
}>()

defineEmits<{
    (e: 'click-album', album: AlbumInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'load-more'): void
}>()
</script>

<style scoped>
.album-row-list {
    display: flex;
    flex-direction: column;
}

/* 整行可点（指针），键盘入口是专辑名按钮与歌手链接，避免嵌套 role=button。 */
.album-row {
    display: flex;
    align-items: center;
    gap: var(--md-space-3);
    min-width: 0;
    padding: var(--md-space-3) 0;
    border-top: 1px solid var(--md-outline-variant);
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.album-row:first-child {
    border-top: none;
}

.album-row:hover {
    background-color: var(--md-surface-container-high);
}

.album-cover {
    width: 56px;
    height: 56px;
    border-radius: var(--md-shape-md);
    object-fit: cover;
    flex-shrink: 0;
    background-color: var(--md-surface-container-high);
}

.album-cover--empty {
    border: 1px solid var(--md-outline-variant);
}

.album-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.album-name {
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* 行内文字链接：WCAG 2.5.8 行内例外取 24px，不撑高行。 */
.album-name-link.n-button {
    height: auto;
    min-height: 24px;
    max-width: 100%;
    padding: 0 2px;
    margin: 0 -2px;
    font-size: inherit;
    line-height: inherit;
    font-weight: inherit;
    vertical-align: baseline;
    color: var(--md-primary);
    text-decoration: underline;
}

.album-name-link.n-button :deep(.n-button__content) {
    display: block;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.album-creator {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.album-meta {
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    color: var(--md-on-surface-variant);
    overflow-wrap: anywhere;
}

.empty-result {
    display: flex;
    justify-content: center;
    padding: var(--md-space-10) var(--md-space-4);
}

@media (prefers-reduced-motion: reduce) {
    .album-row {
        transition: none;
    }
}
</style>

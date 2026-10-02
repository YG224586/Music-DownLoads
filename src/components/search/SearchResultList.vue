<template>
    <div class="search-result-list">
        <template v-if="songs.length > 0">
            <div class="list-header">
                <n-checkbox
                    :checked="isAllSelected"
                    :indeterminate="isIndeterminate"
                    aria-label="全选搜索结果"
                    @update:checked="toggleAll"
                >
                    全选
                </n-checkbox>
                <span class="count-text" aria-live="polite"
                    >已选 {{ selectedIds.length }} / {{ songs.length }} 首</span
                >
            </div>

            <div class="song-items">
                <SongItem
                    v-for="song in songs"
                    :key="song.mid"
                    :song="song"
                    :selected="selectedIds.includes(song.mid)"
                    @toggle-select="(val) => toggleSelect(song.mid, val)"
                    @download="(song) => $emit('download', song)"
                    @click-artist="
                        (platform, artist) =>
                            $emit('click-artist', platform, artist)
                    "
                    @click-album="(song) => $emit('click-album', song)"
                />
            </div>

            <!-- 使用 LoadMoreButton 组件替代原有按钮 -->
            <LoadMoreButton
                v-if="hasMore"
                :loading="loadingMore"
                :disabled="loadingMore"
                @click="$emit('load-more')"
            />
        </template>

        <!-- 请求失败与「确实没有结果」是两种状态，必须分开呈现，否则用户只会看到空列表 -->
        <div v-else-if="error" class="error-result">
            <n-alert type="error" title="搜索失败" class="search-error">
                {{ error }}
            </n-alert>
            <div class="retry-wrapper">
                <n-button type="primary" @click="$emit('retry')">重试</n-button>
            </div>
        </div>

        <div v-else class="empty-result">
            <n-empty description="暂无搜索结果" />
            <div class="retry-wrapper">
                <n-button type="primary" @click="$emit('retry')">重试</n-button>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { NAlert, NCheckbox, NEmpty, NButton } from 'naive-ui'
import type { ArtistReference, SongInfo } from '../../types'
import SongItem from './SongItem.vue'
import LoadMoreButton from './LoadMoreButton.vue'

// hasMore/loadingMore 由父组件 SearchView 传入，控制分页加载按钮显示与加载状态
const props = withDefaults(
    defineProps<{
        songs: SongInfo[]
        selectedIds: string[]
        hasMore?: boolean
        loadingMore?: boolean
        /** 搜索失败时的错误信息；为空表示请求成功（结果可能确实是 0 条） */
        error?: string | null
    }>(),
    {
        hasMore: false,
        loadingMore: false,
        error: null,
    },
)

const emit = defineEmits<{
    (e: 'update:selectedIds', ids: string[]): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
    (e: 'retry'): void // 新增重试事件
    (e: 'load-more'): void // 新增分页加载更多事件
}>()

const isAllSelected = computed(
    () =>
        props.songs.length > 0 &&
        props.selectedIds.length === props.songs.length,
)

const isIndeterminate = computed(
    () =>
        props.selectedIds.length > 0 &&
        props.selectedIds.length < props.songs.length,
)

function toggleAll(checked: boolean) {
    if (checked) {
        emit(
            'update:selectedIds',
            props.songs.map((s) => s.mid),
        )
    } else {
        emit('update:selectedIds', [])
    }
}

function toggleSelect(songMid: string, selected: boolean) {
    let newIds: string[]
    if (selected) {
        newIds = [...props.selectedIds, songMid]
    } else {
        newIds = props.selectedIds.filter((id) => id !== songMid)
    }
    emit('update:selectedIds', newIds)
}
</script>

<style scoped>
/* 选择栏是列表的紧邻上文，不做成卡片；勾选状态由复选框形状承担。 */
.list-header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-2) var(--md-space-3);
    min-width: 0;
    padding-bottom: var(--md-space-1);
}

.count-text {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    min-width: 0;
}

.song-items {
    display: flex;
    flex-direction: column;
    margin-top: var(--md-space-1);
}

.empty-result {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--md-space-4);
    padding: var(--md-space-10) var(--md-space-4);
}

/* 失败态是「需要用户动作」的状态：错误说明 + 唯一的重试操作 */
.error-result {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-4);
    padding: var(--md-space-6) 0;
}

.error-result .search-error {
    border-radius: var(--md-shape-md);
}

.retry-wrapper {
    text-align: center;
}
</style>

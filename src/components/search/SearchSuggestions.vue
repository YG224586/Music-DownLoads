<template>
    <div class="search-suggestions" v-if="hasAny">
        <!-- 建议列表是即时导航入口：用 list item，不把每一项包成卡片 -->
        <section v-if="data.song.length > 0" class="suggest-group">
            <h2 class="group-title">单曲</h2>
            <ul class="suggest-list">
                <li
                    v-for="(item, index) in data.song"
                    :key="item.mid ?? item.id ?? `song-${index}`"
                >
                    <button
                        type="button"
                        class="suggest-item"
                        @click="handleSelect(item, 'song')"
                    >
                        <span class="item-name">{{ item.name }}</span>
                        <span v-if="item.singer" class="item-singer"
                            >- {{ item.singer }}</span
                        >
                        <svg
                            class="item-arrow"
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
                </li>
            </ul>
        </section>

        <section v-if="data.singer.length > 0" class="suggest-group">
            <h2 class="group-title">歌手</h2>
            <ul class="suggest-list">
                <li
                    v-for="(item, index) in data.singer"
                    :key="item.mid ?? item.id ?? `singer-${index}`"
                >
                    <button
                        type="button"
                        class="suggest-item"
                        @click="handleSelect(item, 'singer')"
                    >
                        <span class="item-name">{{ item.name }}</span>
                        <svg
                            class="item-arrow"
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
                </li>
            </ul>
        </section>

        <section v-if="data.album.length > 0" class="suggest-group">
            <h2 class="group-title">专辑</h2>
            <ul class="suggest-list">
                <li
                    v-for="(item, index) in data.album"
                    :key="item.mid ?? item.id ?? `album-${index}`"
                >
                    <button
                        type="button"
                        class="suggest-item"
                        @click="handleSelect(item, 'album')"
                    >
                        <span class="item-name">{{ item.name }}</span>
                        <span v-if="item.singer" class="item-singer"
                            >- {{ item.singer }}</span
                        >
                        <svg
                            class="item-arrow"
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
                </li>
            </ul>
        </section>

        <section v-if="data.mv.length > 0" class="suggest-group">
            <h2 class="group-title">MV</h2>
            <ul class="suggest-list">
                <li
                    v-for="(item, index) in data.mv"
                    :key="item.vid ?? item.mid ?? item.id ?? `mv-${index}`"
                >
                    <button
                        type="button"
                        class="suggest-item"
                        @click="handleSelect(item, 'mv')"
                    >
                        <span class="item-name">{{ item.name }}</span>
                        <span v-if="item.singer" class="item-singer"
                            >- {{ item.singer }}</span
                        >
                        <svg
                            class="item-arrow"
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
                </li>
            </ul>
        </section>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { SearchSuggestionData, SearchSuggestionItem } from '../../types'

const props = defineProps<{
    data: SearchSuggestionData
}>()

const emit = defineEmits<{
    (e: 'select', keyword: string, type: keyof SearchSuggestionData): void
}>()

function handleSelect(
    item: SearchSuggestionItem,
    type: keyof SearchSuggestionData,
) {
    // 只有 name 存在时才触发选择
    if (item.name) {
        emit('select', item.name, type)
    }
}

const hasAny = computed(
    () =>
        props.data.song.length > 0 ||
        props.data.singer.length > 0 ||
        props.data.album.length > 0 ||
        props.data.mv.length > 0,
)
</script>

<style scoped>
/* 建议面板：用表面容器与页面分组，不叠加阴影与边框。 */
.search-suggestions {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-4);
    margin-top: var(--md-space-4);
    padding: var(--md-space-2) 0;
    background-color: var(--md-surface-container-low);
    border-radius: var(--md-shape-lg);
    overflow: hidden;
}

.suggest-group {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-1);
    min-width: 0;
}

.group-title {
    margin: 0;
    padding: var(--md-space-1) var(--md-space-4) 0;
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface-variant);
}

.suggest-list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
}

.suggest-item {
    display: flex;
    align-items: baseline;
    gap: var(--md-space-1);
    width: 100%;
    min-height: var(--md-target-min);
    padding: var(--md-space-2) var(--md-space-4);
    border: none;
    background: transparent;
    color: var(--md-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.suggest-item:hover {
    background-color: var(--md-surface-container-high);
}

.item-name {
    min-width: 0;
    flex: 0 1 auto;
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    color: var(--md-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.item-singer {
    min-width: 0;
    flex: 0 1 auto;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* 箭头表示「点击后进入下一层」，属于装饰信息，不承担状态含义。 */
.item-arrow {
    flex-shrink: 0;
    margin-left: auto;
    align-self: center;
    color: var(--md-on-surface-variant);
}

@media (prefers-reduced-motion: reduce) {
    .suggest-item {
        transition: none;
    }
}
</style>

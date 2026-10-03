<template>
    <div class="song-item" :class="{ 'is-selected': selected }">
        <!-- 选中态不只靠颜色：复选框勾选形状 + 容器色调同时变化 -->
        <n-checkbox
            :checked="selected"
            :aria-label="`选择 ${song.title}`"
            @update:checked="$emit('toggleSelect', $event)"
        />
        <div class="cover-wrapper">
            <img
                v-if="coverUrl && !coverFailed"
                :src="coverUrl"
                class="cover"
                alt="封面"
                loading="lazy"
                @error="coverFailed = true"
            />
            <!-- 图片加载失败：回落到占位块，并给读屏器一个明确的失败说明 -->
            <div
                v-else-if="coverFailed"
                class="cover placeholder"
                role="img"
                aria-label="封面加载失败"
            />
            <div v-else-if="coverLoading" class="cover placeholder" />
            <div
                v-else
                class="cover placeholder default"
                role="img"
                aria-label="暂无封面"
            />
        </div>
        <div class="info">
            <div class="title">{{ song.title }}</div>
            <!--
                歌手与专辑合并进同一条 48dp 元信息行：两个链接各自 ≥48x48 命中矩形且互不重叠，
                同时不把列表行撑高（若各占一行 48dp，每行会多出约 44px）。
            -->
            <div class="subtitle">
                <ArtistNames
                    :platform="song.platform"
                    :artists="song.artists"
                    :fallback="song.artist"
                    @click-artist="
                        (platform, artist) =>
                            $emit('click-artist', platform, artist)
                    "
                />
                <template v-if="song.album">
                    <span class="meta-separator" aria-hidden="true">·</span>
                    <n-button
                        v-if="albumId"
                        text
                        size="small"
                        class="album-link"
                        :aria-label="`查看专辑 ${song.album}`"
                        @click.stop="$emit('click-album', song)"
                    >
                        {{ song.album }}
                    </n-button>
                    <span v-else class="album-plain">{{ song.album }}</span>
                </template>
            </div>
            <div class="quality-tags">
                <!-- 自定义脚本音源的结果标注来源，内置音源不显示，避免噪音 -->
                <span v-if="sourceLabel" class="source-tag">{{
                    sourceLabel
                }}</span>
                <n-tag
                    v-for="q in sortedQualities.slice(0, 4)"
                    :key="q.quality"
                    size="tiny"
                    :bordered="false"
                    type="info"
                    class="quality-tag"
                >
                    {{ q.quality }}
                </n-tag>
                <n-tag
                    v-if="sortedQualities.length > 4"
                    size="tiny"
                    :bordered="false"
                    type="info"
                    class="quality-tag"
                >
                    +{{ sortedQualities.length - 4 }}
                </n-tag>
            </div>
        </div>
        <n-button
            size="small"
            secondary
            class="download-btn"
            @click="$emit('download', song)"
        >
            下载
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { NCheckbox, NButton, NTag } from 'naive-ui'
import type { ArtistReference, SongInfo } from '../../types'
import { ALL_QUALITY_ORDER } from '../../types'
import { fetchCover } from '../../api/musicApi'
import ArtistNames from './ArtistNames.vue'
import { getMusicEntityId } from '../../utils/music'
import { useScriptSourceStore } from '../../stores/scriptSourceStore'

const props = defineProps<{
    song: SongInfo
    selected: boolean
}>()

defineEmits<{
    (e: 'toggleSelect', selected: boolean): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
}>()

const albumId = computed(() =>
    getMusicEntityId(
        props.song.platform,
        props.song.albumId,
        props.song.albumMid,
    ),
)

// 自定义脚本音源：结果条目上标注来源音源名；内置音源保持原样。
const scriptSourceStore = useScriptSourceStore()
const sourceLabel = computed(() => {
    const platform = props.song.platform
    if (!platform.startsWith('script:')) return ''
    // 音源被删除时 platformLabel 会原样返回 "script:<id>"，不能把它暴露给用户。
    const label = scriptSourceStore.platformLabel(platform)
    return label === platform ? '自定义音源' : label
})

// 按品质从高到低排序
const sortedQualities = computed(() => {
    return [...props.song.qualities].sort((a, b) => {
        const ia = ALL_QUALITY_ORDER.indexOf(a.quality)
        const ib = ALL_QUALITY_ORDER.indexOf(b.quality)
        // 未知品质放在末尾
        const idxA = ia === -1 ? -1 : ia
        const idxB = ib === -1 ? -1 : ib
        return idxB - idxA // 降序
    })
})

// 优先展示歌曲自带的封面，缺少地址时按需请求。
const coverUrl = ref<string>('')
const coverLoading = ref(false)
// 图片请求失败（404/解码失败）时切回占位块，避免浏览器默认的破图图标。
const coverFailed = ref(false)

async function loadCoverIfNeeded() {
    coverFailed.value = false
    // 已有 URL 直接使用
    if (props.song.coverUrl) {
        coverUrl.value = props.song.coverUrl
        return
    }
    // 酷我场景下按需加载
    if (!props.song.id) return
    coverLoading.value = true
    // 捕获本次请求对应的歌曲 id：await 期间列表项可能已被复用（song prop 改变）
    const requestedId = props.song.id
    try {
        const url = await fetchCover('kuwo', requestedId)
        if (props.song.id === requestedId) {
            coverUrl.value = url
        }
    } catch {
        // 加载失败保持占位
    } finally {
        coverLoading.value = false
    }
}

onMounted(() => {
    loadCoverIfNeeded()
})

// 切换 song prop 时（如列表项重用）重新加载
watch(
    () => props.song.id,
    () => {
        coverFailed.value = false
        coverUrl.value = props.song.coverUrl
        loadCoverIfNeeded()
    },
)
</script>

<style scoped>
/* M3 list item：分隔线用 border-top，列表第一项不画线，末项也不留尾线。 */
.song-item {
    display: flex;
    align-items: flex-start;
    gap: var(--md-space-3);
    min-width: 0;
    padding: var(--md-space-3) 0;
    border-top: 1px solid var(--md-outline-variant);
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.song-item:first-child {
    border-top: none;
}

.song-item.is-selected {
    background-color: var(--md-secondary-container);
}

.song-item :deep(.n-checkbox) {
    flex-shrink: 0;
    /* 全局规则已给 .n-checkbox min-height:48px，这里补齐宽度得到 48x48 命中区；
       左右负 margin 抵消多出来的 32px 占位，视觉位置与行内排版保持不变。
       右侧负 margin 会让 48dp 框的最后 4dp 与封面矩形重叠，封面在 DOM 中靠后、
       会盖住这部分导致命中区实测只有 44x48（独立验证 P3-2）；
       抬到封面之上即可恢复完整 48x48，且不改变任何布局尺寸。 */
    position: relative;
    z-index: 1;
    width: var(--md-target-min);
    justify-content: center;
    margin-left: calc(-1 * var(--md-space-4));
    margin-right: calc(-1 * var(--md-space-4));
}

.cover-wrapper {
    width: 56px;
    height: 56px;
    flex-shrink: 0;
}

.cover {
    width: 56px;
    height: 56px;
    border-radius: var(--md-shape-md);
    object-fit: cover;
    display: block;
}

.cover.placeholder {
    background-color: var(--md-surface-container-high);
}

.cover.placeholder.default {
    background-color: var(--md-surface-container);
}

.info {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.title {
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/*
 * 元信息行：歌手与专辑共用同一条 48dp 命中带。
 * 两个链接都撑到 48x48（靠 min-width/min-height，不用 padding，避免相邻可点区重叠），
 * 文本超出时由 .n-button__content 省略号截断，行高不随内容膨胀。
 */
.subtitle {
    display: flex;
    align-items: center;
    gap: var(--md-space-1);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
    min-width: 0;
    min-height: var(--md-target-min);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* 子组件 ArtistNames 的容器：允许整组收缩，超长歌手名走省略号。 */
.subtitle :deep(.artist-names) {
    display: flex;
    align-items: center;
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
}

.meta-separator {
    flex-shrink: 0;
    color: var(--md-on-surface-variant);
}

.song-item.is-selected .subtitle {
    color: var(--md-on-secondary-container);
}

/* 可点击的歌手/专辑名用下划线区分，不单靠颜色表达可交互。 */
.subtitle :deep(.artist-link.n-button),
.album-link.n-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 1 auto;
    width: auto;
    height: auto;
    min-width: var(--md-target-min);
    min-height: var(--md-target-min);
    max-width: 100%;
    padding: 0;
    margin: 0;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    font-weight: inherit;
    color: var(--md-primary);
    text-decoration: underline;
    overflow: hidden;
}

.subtitle :deep(.artist-link.n-button .n-button__content),
.album-link.n-button :deep(.n-button__content) {
    display: block;
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.album-plain {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--md-on-surface-variant);
}

.quality-tags {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: flex-start;
    column-gap: var(--md-space-1);
    row-gap: var(--md-space-1);
    margin-top: var(--md-space-1);
}

/* M3 只读元数据标签：高度保持 22px（不可点，无需 48dp 命中区），圆角用 chip 的 8dp */
.quality-tag.n-tag {
    height: 22px;
    padding: 0 var(--md-space-2);
    border-radius: var(--md-shape-chip);
    background-color: var(--md-surface-container-high);
    color: var(--md-on-surface-variant);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
}

/* 来源音源标签：与音质标签同高，用填充色区分「来自自定义音源」 */
.source-tag {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 var(--md-space-2);
    border-radius: var(--md-shape-chip);
    background-color: var(--md-secondary-container);
    color: var(--md-on-secondary-container);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    white-space: nowrap;
}

.quality-tag.n-tag :deep(.n-tag__content) {
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
}

.download-btn.n-button {
    flex-shrink: 0;
    align-self: center;
}

@media (prefers-reduced-motion: reduce) {
    .song-item {
        transition: none;
    }
}
</style>

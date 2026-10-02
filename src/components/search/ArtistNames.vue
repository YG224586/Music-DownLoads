<template>
    <span class="artist-names">
        <template v-if="artists?.length">
            <template v-for="(artist, index) in artists" :key="index">
                <span v-if="index" class="artist-separator">{{
                    settingsStore.settings.artistSeparator
                }}</span>
                <n-button
                    v-if="getMusicEntityId(platform, artist.id, artist.mid)"
                    text
                    size="small"
                    class="artist-link inline-link"
                    :aria-label="`查看歌手 ${artist.name}`"
                    @click.stop="$emit('click-artist', platform, artist)"
                >
                    {{ artist.name }}
                </n-button>
                <span v-else class="artist-plain">{{ artist.name }}</span>
            </template>
        </template>
        <template v-else>{{ fallback }}</template>
    </span>
</template>

<script setup lang="ts">
import type { ArtistReference } from '../../types'
import { NButton } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { getMusicEntityId } from '../../utils/music'

defineProps<{
    platform: string
    artists?: ArtistReference[]
    fallback: string
}>()

defineEmits<{
    (e: 'click-artist', platform: string, artist: ArtistReference): void
}>()

const settingsStore = useSettingsStore()
</script>

<style scoped>
.artist-names {
    min-width: 0;
}

/*
 * 行内文字链接：嵌在正文行里，按 WCAG 2.5.8 的「行内目标」例外取 24px 命中高度，
 * 保证不撑高列表行；独立控件（按钮、芯片、复选项）仍为 48dp。
 */
.artist-link.n-button {
    height: auto;
    min-height: 24px;
    padding: 0 2px;
    margin: 0 -2px;
    font-size: inherit;
    line-height: inherit;
    font-weight: inherit;
    vertical-align: baseline;
    color: var(--md-primary);
    text-decoration: underline;
}

.artist-plain {
    color: inherit;
}

.artist-separator {
    color: inherit;
}
</style>

<template>
    <SettingRow label="示例">
        <template #description>
            <div class="preview-container">
                <div class="preview-line">
                    <span class="preview-label">歌手：</span>
                    <code class="preview-value">{{ exampleArtists }}</code>
                </div>
                <div class="preview-line">
                    <span class="preview-label">文件名：</span>
                    <code class="preview-value"
                        >{{ exampleFilename }}.flac</code
                    >
                </div>
            </div>
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useSettingsStore } from '../../stores/settingsStore'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()

// 用于预览的示例歌曲信息。artist 字段在示例中按用户配置的歌手分隔符拼接，
// 这样两个设置的修改可以即时在同一个示例里看到效果。
const exampleSong = {
    song: '千里之外',
    artistList: ['周杰伦', '费玉清'],
    album: '依然范特西',
    quality: 'flac',
}

// 过滤非法字符的函数（与后端 sanitize_name 一致）
const sanitize = (raw: string) => raw.replace(/[\\/:*?"<>|]/g, '_')

// 当前配置下的多歌手拼接结果（空值时回退为中文顿号）
const exampleArtists = computed(() => {
    const sep = settingsStore.settings.artistSeparator || '、'
    return exampleSong.artistList.join(sep)
})

// 根据当前模板与歌手拼接结果，生成示例文件名
const exampleFilename = computed(() => {
    const artist = exampleArtists.value
    const template =
        settingsStore.settings.namingTemplate || '{song} - {artist}'
    let name = template
        .replaceAll('{song}', exampleSong.song)
        .replaceAll('{artist}', artist)
        .replaceAll('{album}', exampleSong.album)
        .replaceAll('{quality}', exampleSong.quality)

    const sanitized = sanitize(name).trim()
    if (!sanitized) {
        const fallback = '{song} - {artist}'
            .replaceAll('{song}', exampleSong.song)
            .replaceAll('{artist}', artist)
            .replaceAll('{album}', exampleSong.album)
            .replaceAll('{quality}', exampleSong.quality)
        const fallbackSanitized = sanitize(fallback).trim()
        return fallbackSanitized || '未知歌曲'
    }
    return sanitized
})
</script>

<style scoped>
.preview-container {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-1);
    min-width: 0;
    width: 100%;
}

.preview-line {
    /* 长文件名整行换行而不是撑宽容器 */
    overflow-wrap: anywhere;
}

.preview-label {
    margin-right: var(--md-space-1);
}

.preview-value {
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--md-surface-container-high);
    color: var(--md-on-surface);
    font-family: var(--md-font-plain);
    font-size: var(--md-label-medium);
}
</style>

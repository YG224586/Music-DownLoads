<template>
    <SettingRow label="文件命名模板" stacked>
        <template #description>
            <span class="template-help-line"
                >可使用变量：<code>{song}</code>（歌名）、<code>{artist}</code>（歌手）、<code>{album}</code>（专辑）、<code>{quality}</code>（音质）</span
            >
            <span class="template-help-line"
                >若替换后结果为空或仅含非法字符，将自动使用默认模板“歌名 -
                歌手”</span
            >
        </template>
        <template #default="{ labelId }">
            <n-input
                class="template-input"
                :value="settingsStore.settings.namingTemplate"
                @update:value="
                    (val) => (settingsStore.settings.namingTemplate = val)
                "
                placeholder="{song} - {artist}"
                :input-props="{ 'aria-labelledby': labelId }"
            />
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { NInput } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()
</script>

<style scoped>
/* 输入框占满行宽：模板通常较长，窄视口下也不会把文字挤成一条 */
.template-input {
    width: 100%;
}

.template-help-line {
    display: block;
    overflow-wrap: anywhere;
}

.template-help-line + .template-help-line {
    margin-top: var(--md-space-1);
}

code {
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--md-surface-container-high);
    color: var(--md-on-surface);
    font-family: var(--md-font-plain);
    font-size: var(--md-label-medium);
}
</style>

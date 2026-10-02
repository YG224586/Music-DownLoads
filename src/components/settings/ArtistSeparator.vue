<template>
    <SettingRow label="歌手分隔符" stacked>
        <template #description>
            <span class="separator-help-line"
                >用于拼接多歌手场景下的分隔字符串，可填入
                <code>&amp;</code
                >、<code>/</code>、<code>，</code>等任意文本。</span
            >
            <span class="separator-help-line"
                >留空时将回退为默认的中文顿号：<code>、</code>。</span
            >
        </template>
        <template #default="{ labelId }">
            <n-input
                class="separator-input"
                :value="settingsStore.settings.artistSeparator"
                @update:value="
                    (val) => (settingsStore.settings.artistSeparator = val)
                "
                placeholder="、"
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
/* 分隔符通常只有一两个字符，给固定宽度即可，不必占满整行 */
.separator-input {
    width: 160px;
    max-width: 100%;
}

.separator-help-line {
    display: block;
    overflow-wrap: anywhere;
}

.separator-help-line + .separator-help-line {
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

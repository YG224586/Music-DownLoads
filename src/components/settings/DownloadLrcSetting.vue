<template>
    <SettingRow label="下载时同步保存 LRC 歌词文件">
        <template #default="{ labelId }">
            <n-switch
                aria-label="下载时同步保存 LRC 歌词文件"
                :aria-labelledby="labelId"
                :value="settingsStore.settings.downloadLrc"
                @update:value="
                    (val) => (settingsStore.settings.downloadLrc = val)
                "
            />
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { NSwitch } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()
</script>

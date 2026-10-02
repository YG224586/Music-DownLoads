<template>
    <SettingRow label="下载完成后写入歌曲标签">
        <template #default="{ labelId }">
            <n-switch
                aria-label="下载完成后写入歌曲标签"
                :aria-labelledby="labelId"
                :value="settingsStore.settings.writeMetadata"
                @update:value="
                    (val) => (settingsStore.settings.writeMetadata = val)
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

<template>
    <SettingRow label="添加任务后跳转至任务页">
        <template #default="{ labelId }">
            <n-switch
                aria-label="添加任务后跳转至任务页"
                :aria-labelledby="labelId"
                :value="settingsStore.settings.jumpToTask"
                @update:value="
                    (val) => (settingsStore.settings.jumpToTask = val)
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

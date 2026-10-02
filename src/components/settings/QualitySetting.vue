<template>
    <SettingRow label="默认音质">
        <template #default="{ labelId }">
            <n-select
                class="quality-select"
                :aria-labelledby="labelId"
                :value="settingsStore.settings.defaultQuality"
                @update:value="
                    (val: string) =>
                        (settingsStore.settings.defaultQuality = val)
                "
                :options="qualityOptions"
            />
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { NSelect } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { ALL_QUALITY_ORDER } from '../../types'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()

const qualityOptions = [
    { label: '每次询问', value: 'ask' },
    ...ALL_QUALITY_ORDER.map((q) => ({ label: q, value: q })),
]
</script>

<style scoped>
/* 固定宽度但允许在极窄视口收缩，避免出现横向滚动 */
.quality-select {
    width: 200px;
    max-width: 100%;
}
</style>

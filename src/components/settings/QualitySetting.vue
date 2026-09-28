<template>
    <n-form-item label="默认音质">
        <n-select
            :value="settingsStore.settings.defaultQuality"
            @update:value="
                (val: string) => (settingsStore.settings.defaultQuality = val)
            "
            :options="qualityOptions"
        />
    </n-form-item>
</template>

<script setup lang="ts">
import { NFormItem, NSelect } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { ALL_QUALITY_ORDER } from '../../types'

const settingsStore = useSettingsStore()

const qualityOptions = [
    { label: '每次询问', value: 'ask' },
    ...ALL_QUALITY_ORDER.map((q) => ({ label: q, value: q })),
]
</script>

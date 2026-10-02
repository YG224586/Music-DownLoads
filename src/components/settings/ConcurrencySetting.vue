<template>
    <SettingRow label="同时下载数">
        <template #default="{ labelId }">
            <!--
                不使用输入框内置的步进按钮：它们只有约 18dp 宽，触屏上难以命中。
                这里用两个独立按钮提供 ≥48dp 的加减目标，并各自带上可访问名。
            -->
            <div class="concurrency-control">
                <n-button
                    type="primary"
                    secondary
                    circle
                    class="step-button"
                    aria-label="减少同时下载数"
                    :disabled="current <= minConcurrent"
                    @click="setConcurrent(current - 1)"
                >
                    −
                </n-button>
                <n-input-number
                    :value="settingsStore.settings.maxConcurrent"
                    @update:value="setConcurrent"
                    :min="minConcurrent"
                    :max="maxConcurrent"
                    step="1"
                    :show-button="false"
                    class="concurrency-input"
                    :input-props="{
                        'aria-labelledby': labelId,
                        'aria-label': '同时下载数',
                    }"
                />
                <n-button
                    type="primary"
                    secondary
                    circle
                    class="step-button"
                    aria-label="增加同时下载数"
                    :disabled="current >= maxConcurrent"
                    @click="setConcurrent(current + 1)"
                >
                    +
                </n-button>
            </div>
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { NButton, NInputNumber } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import SettingRow from './SettingRow.vue'

const minConcurrent = 1
const maxConcurrent = 10

const settingsStore = useSettingsStore()

const current = computed(
    () => settingsStore.settings.maxConcurrent ?? minConcurrent,
)

/** 手输与加减按钮共用同一收敛逻辑，避免出现越界或 NaN。 */
function setConcurrent(value: number | null) {
    const numeric =
        typeof value === 'number' && Number.isFinite(value)
            ? Math.round(value)
            : minConcurrent
    settingsStore.settings.maxConcurrent = Math.min(
        maxConcurrent,
        Math.max(minConcurrent, numeric),
    )
}
</script>

<style scoped>
.concurrency-control {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--md-space-2);
    flex-wrap: wrap;
}

.step-button {
    min-width: var(--md-target-min);
    font-size: var(--md-title-medium);
}

.concurrency-input {
    width: 104px;
    max-width: 100%;
}

.concurrency-input :deep(.n-input__input-el) {
    text-align: center;
}

.concurrency-input :deep(.n-input__input) {
    display: flex;
    align-items: center;
}
</style>

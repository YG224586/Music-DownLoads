<template>
    <SettingRow label="重名文件处理策略" stacked>
        <template #default="{ labelId }">
            <n-radio-group
                class="strategy-group"
                role="radiogroup"
                :aria-labelledby="labelId"
                :value="settingsStore.settings.duplicateStrategy"
                @update:value="
                    (val) => (settingsStore.settings.duplicateStrategy = val)
                "
            >
                <n-radio-button value="ask">询问</n-radio-button>
                <n-radio-button value="overwrite">覆盖</n-radio-button>
                <n-radio-button value="rename">保留两份</n-radio-button>
                <n-radio-button value="cancel">取消</n-radio-button>
            </n-radio-group>
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { NRadioGroup, NRadioButton } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()
</script>

<style scoped>
/*
 * 四个互斥策略用 segmented button 表达（M3）。整体占满行宽、等分，
 * 基准宽度 72px 保证“保留两份”不被裁切；极窄视口下图元换行而不是溢出。
 */
.strategy-group {
    display: flex;
    flex-wrap: wrap;
    width: 100%;
    max-width: 420px;
}

.strategy-group :deep(.n-radio-button) {
    display: flex;
    flex: 1 1 72px;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: 0 var(--md-space-2);
}

.strategy-group :deep(.n-radio-button__label) {
    width: 100%;
    overflow-wrap: anywhere;
    text-align: center;
}
</style>

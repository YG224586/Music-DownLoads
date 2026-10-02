<template>
    <SettingRow
        label="清除搜索历史"
        description="将删除本机保存的全部搜索关键词，且无法撤销。"
        destructive
    >
        <template #default>
            <n-button type="error" secondary @click="confirmClearHistory"
                >清除</n-button
            >
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { NButton, useDialog } from 'naive-ui'
import { useHistoryStore } from '../../stores/historyStore'
import SettingRow from './SettingRow.vue'

const historyStore = useHistoryStore()
const dialog = useDialog()

/**
 * 破坏性动作必须二次确认：历史一旦清除无法恢复，
 * 直接执行会让误触（尤其手机端）造成不可逆的数据丢失。
 */
function confirmClearHistory() {
    dialog.error({
        title: '清除搜索历史',
        content: '将删除本机保存的全部搜索关键词，且无法撤销。',
        positiveText: '清除',
        negativeText: '取消',
        positiveButtonProps: { type: 'error' },
        onPositiveClick: () => {
            historyStore.clearHistory()
        },
    })
}
</script>

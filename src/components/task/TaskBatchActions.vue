<template>
    <div
        v-if="selectedCount > 0"
        class="batch-actions"
        role="region"
        aria-label="批量操作"
    >
        <span class="batch-actions-count" aria-live="polite">
            已选择 {{ selectedCount }} 个任务
        </span>
        <n-popconfirm
            :style="{ maxWidth: 'calc(100vw - 32px)' }"
            @positive-click="handleConfirm"
        >
            <template #trigger>
                <!-- 清除不可恢复：该区域唯一动作，用 filled error 明确破坏性；确认框保留文件删除勾选 -->
                <n-button type="error">清除所选</n-button>
            </template>
            <n-space vertical :size="8">
                <span>确认清除所选任务吗？</span>
                <n-checkbox v-model:checked="deleteFile">
                    同时删除已下载或未完成的文件
                </n-checkbox>
            </n-space>
        </n-popconfirm>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NButton, NPopconfirm, NCheckbox, NSpace } from 'naive-ui'

defineProps<{
    selectedCount: number
}>()

const emit = defineEmits<{
    (e: 'clear', deleteFile: boolean): void
}>()

const deleteFile = ref(false)

function handleConfirm() {
    emit('clear', deleteFile.value)
    // 重置复选框状态，防止下次打开弹窗时保留上次勾选
    deleteFile.value = false
}
</script>

<style scoped>
/* 选中态下的底部操作条：贴着滚动容器底部，铺满可用宽度（M3 bottom action bar） */
.batch-actions {
    position: sticky;
    bottom: 0;
    z-index: 2;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-2) var(--md-space-4);
    padding: var(--md-space-3) var(--page-padding);
    /* 抵消 .main-content 的内边距，让操作条延伸到内容区边缘，不再是一张浮在行上的圆角卡片 */
    margin: 0 calc(var(--page-padding) * -1) calc(var(--page-padding) * -1);
    min-width: 0;
    flex-shrink: 0;
    background: var(--md-surface-container-high);
    border-top: 1px solid var(--md-outline-variant);
    box-shadow: var(--md-elevation-2);
}

.batch-actions-count {
    flex: 1 1 auto;
    min-width: 0;
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
    overflow-wrap: anywhere;
}

.batch-actions :deep(.n-button) {
    flex-shrink: 0;
}

/* 极窄屏兜底：计数与按钮各占一行，按钮整行便于点击 */
@media (max-width: 359px) {
    .batch-actions-count {
        flex: 1 1 100%;
    }

    .batch-actions :deep(.n-button) {
        width: 100%;
    }
}
</style>

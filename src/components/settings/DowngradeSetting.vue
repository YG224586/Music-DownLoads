<template>
    <SettingRow label="自动降级">
        <template #default="{ labelId }">
            <n-switch
                aria-label="自动降级"
                :aria-labelledby="labelId"
                :value="settingsStore.settings.autoDowngrade"
                @update:value="
                    (val) => (settingsStore.settings.autoDowngrade = val)
                "
            />
        </template>
    </SettingRow>

    <SettingRow label="降级顺序" stacked>
        <template #description>
            <!-- 顺序文本允许换行：音质名称较长时截断会让用户看不到完整顺序 -->
            <div
                class="downgrade-summary"
                :title="downgradeOrderText"
                :aria-label="`当前降级顺序：${downgradeOrderText}`"
            >
                {{ downgradeOrderText }}
            </div>
            <p class="downgrade-help">
                <template v-if="settingsStore.settings.autoDowngrade">
                    目标音质不可用时，将从它的下一项开始，按从上到下的顺序依次尝试。
                </template>
                <template v-else>
                    当前顺序已保留，开启自动降级后可编辑并生效。
                </template>
            </p>
        </template>
        <template #default>
            <n-button
                type="primary"
                secondary
                :disabled="!settingsStore.settings.autoDowngrade"
                @click="openEditor"
            >
                自定义顺序
            </n-button>
        </template>
    </SettingRow>

    <!--
        排序过程只修改 draftOrder，点击“保存”后才一次性写回 Pinia。
        这样“取消”能完整撤销本次编辑，也不会让每次上移/下移都触发设置持久化。
    -->
    <n-modal
        v-model:show="showEditor"
        preset="card"
        title="自定义降级顺序"
        class="downgrade-modal"
        style="width: min(480px, calc(100vw - 32px))"
        :mask-closable="false"
    >
        <p id="downgrade-order-help" class="editor-help">
            排在目标音质之后的项目才会作为降级候选。目标音质本身可用时仍会直接下载。
        </p>

        <ol class="quality-order-list" aria-describedby="downgrade-order-help">
            <li
                v-for="(quality, index) in draftOrder"
                :key="quality"
                class="quality-order-item"
            >
                <span class="quality-index" aria-hidden="true">{{
                    index + 1
                }}</span>
                <span class="quality-name">{{ quality }}</span>
                <n-tag
                    v-if="quality === settingsStore.settings.defaultQuality"
                    size="small"
                    :bordered="false"
                >
                    默认
                </n-tag>
                <div class="move-actions">
                    <n-button
                        quaternary
                        :disabled="index === 0"
                        :aria-label="`上移 ${quality}`"
                        @click="moveQuality(index, -1)"
                    >
                        ↑
                    </n-button>
                    <n-button
                        quaternary
                        :disabled="index === draftOrder.length - 1"
                        :aria-label="`下移 ${quality}`"
                        @click="moveQuality(index, 1)"
                    >
                        ↓
                    </n-button>
                </div>
            </li>
        </ol>

        <!-- 视觉隐藏的实时区域会向读屏软件报告排序结果。 -->
        <span class="sr-only" aria-live="polite">{{ liveMessage }}</span>

        <template #footer>
            <div class="modal-actions">
                <n-button
                    text
                    :disabled="isDefaultOrder"
                    @click="resetDraftOrder"
                >
                    恢复默认顺序
                </n-button>
                <div class="modal-primary-actions">
                    <n-button @click="cancelEditor">取消</n-button>
                    <n-button
                        type="primary"
                        :disabled="!hasChanges"
                        @click="saveOrder"
                        >保存</n-button
                    >
                </div>
            </div>
        </template>
    </n-modal>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { NButton, NModal, NSwitch, NTag } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { QUALITY_DOWNGRADE_ORDER } from '../../types'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()

const showEditor = ref(false)
const draftOrder = ref<string[]>([])
const liveMessage = ref('')

const downgradeOrderText = computed(() =>
    settingsStore.settings.qualityDowngradeOrder.join(' → '),
)

const hasChanges = computed(
    () =>
        draftOrder.value.join('\u0000') !==
        settingsStore.settings.qualityDowngradeOrder.join('\u0000'),
)

const isDefaultOrder = computed(
    () =>
        draftOrder.value.join('\u0000') ===
        QUALITY_DOWNGRADE_ORDER.join('\u0000'),
)

/** 打开编辑器时克隆已保存顺序，保证关闭或取消不会污染持久设置。 */
function openEditor() {
    draftOrder.value = [...settingsStore.settings.qualityDowngradeOrder]
    liveMessage.value = ''
    showEditor.value = true
}

function cancelEditor() {
    showEditor.value = false
}

/**
 * 将指定音质移动一格。这里不直接修改设置：用户可以连续调整并预览，最后只在
 * 点击“保存”时提交一次。上/下移按钮同时兼容鼠标、键盘和触屏操作。
 */
function moveQuality(index: number, offset: -1 | 1) {
    const targetIndex = index + offset
    if (targetIndex < 0 || targetIndex >= draftOrder.value.length) return

    const next = [...draftOrder.value]
    const [quality] = next.splice(index, 1)
    next.splice(targetIndex, 0, quality)
    draftOrder.value = next
    liveMessage.value = `已将 ${quality} 移至第 ${targetIndex + 1} 位`
}

/** 恢复只作用于弹窗草稿；用户仍可通过“取消”保留之前保存的自定义顺序。 */
function resetDraftOrder() {
    draftOrder.value = [...QUALITY_DOWNGRADE_ORDER]
    liveMessage.value = '已恢复默认顺序，点击保存后生效'
}

function saveOrder() {
    // 使用新数组写回，避免后续编辑草稿时与 store 共用同一个数组引用。
    settingsStore.settings.qualityDowngradeOrder = [...draftOrder.value]
    showEditor.value = false
}
</script>

<style scoped>
/* 弹窗由 Modal 渲染到 body，使用专属类名定位；内容滚动，页脚保持可见 */
:global(.downgrade-modal) {
    max-height: calc(
        100vh - 32px - var(--safe-area-top) - var(--safe-area-bottom)
    );
    max-height: calc(
        100dvh - 32px - var(--safe-area-top) - var(--safe-area-bottom)
    );
    border-radius: var(--md-shape-xl);
}

:global(.downgrade-modal > .n-card-content) {
    min-height: 0;
    overflow-y: auto;
}

/* Naive 卡片内置的关闭按钮只有 18dp；放大到 48dp 命中区并保留圆形状态层 */
:global(.downgrade-modal .n-card-header__close) {
    box-sizing: border-box;
    width: var(--md-target-min);
    height: var(--md-target-min);
    border-radius: var(--md-shape-full);
    font-size: 22px;
}

.downgrade-summary {
    color: var(--md-on-surface);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    /* 完整展示顺序，不用省略号裁切 */
    overflow-wrap: anywhere;
}

.downgrade-help,
.editor-help {
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
}

.downgrade-help {
    margin: var(--md-space-1) 0 0;
}

.editor-help {
    margin: 0 0 var(--md-space-3);
}

.quality-order-list {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-2);
    max-height: 58vh;
    overflow-y: auto;
    padding: 0;
    margin: 0;
    list-style: none;
}

.quality-order-item {
    display: flex;
    min-height: 56px;
    align-items: center;
    gap: var(--md-space-3);
    padding: var(--md-space-1) var(--md-space-1) var(--md-space-1)
        var(--md-space-4);
    border: 1px solid var(--md-outline-variant);
    border-radius: var(--md-shape-md);
    background: var(--md-surface-container-lowest);
}

.quality-index {
    width: 22px;
    flex: 0 0 22px;
    color: var(--md-on-surface-variant);
    font-size: var(--md-label-medium);
    text-align: right;
}

.quality-name {
    min-width: 0;
    flex: 1;
    color: var(--md-on-surface);
    overflow-wrap: anywhere;
}

.move-actions,
.modal-primary-actions,
.modal-actions {
    display: flex;
    align-items: center;
}

.move-actions {
    gap: var(--md-space-1);
}

/* 「↑ / ↓」是纯文本按钮，不会被全局的纯图标按钮规则加宽，这里显式保证 48dp 命中区 */
.move-actions :deep(.n-button) {
    min-width: var(--md-target-min);
}

.modal-primary-actions {
    gap: var(--md-space-2);
}

.modal-actions {
    justify-content: space-between;
    gap: var(--md-space-4);
    flex-wrap: wrap;
}

.sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
}

@media (max-width: 767px) {
    :global(.downgrade-modal > .n-card-header),
    :global(.downgrade-modal > .n-card-content),
    :global(.downgrade-modal > .n-card__footer) {
        padding-left: var(--md-space-4);
        padding-right: var(--md-space-4);
    }

    .modal-actions {
        align-items: stretch;
        flex-direction: column;
    }

    .modal-primary-actions {
        justify-content: flex-end;
    }
}
</style>

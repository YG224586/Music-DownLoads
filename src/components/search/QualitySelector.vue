<template>
    <div class="quality-selector">
        <!-- 互斥单选：用 radio 列表，选中态由圆点填充形状表达，不只靠颜色。 -->
        <n-radio-group
            v-model:value="selected"
            class="quality-group"
            aria-label="选择下载音质"
        >
            <n-radio
                v-for="q in sortedQualities"
                :key="q.quality"
                :value="q.quality"
                class="quality-option"
            >
                <span class="quality-text">
                    <span class="quality-name">{{ q.quality }}</span>
                    <!-- 体积未知时（脚本音源通常不提供）不显示，避免出现「0.00 MB」 -->
                    <span v-if="q.size > 0" class="quality-size">
                        {{ formatSize(q.size) }}
                    </span>
                </span>
            </n-radio>
        </n-radio-group>
    </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { NRadio, NRadioGroup } from 'naive-ui'
import type { QualityItem } from '../../types'
import { ALL_QUALITY_ORDER } from '../../types'

const props = defineProps<{
    qualities: QualityItem[]
}>()

// 从高到低排序
const sortedQualities = computed(() =>
    [...props.qualities].sort(
        (a, b) =>
            ALL_QUALITY_ORDER.indexOf(b.quality) -
            ALL_QUALITY_ORDER.indexOf(a.quality),
    ),
)

// 默认选中最高可用品质（排序后的第一个）
const getDefaultQuality = (): string => {
    const sorted = [...props.qualities].sort(
        (a, b) =>
            ALL_QUALITY_ORDER.indexOf(b.quality) -
            ALL_QUALITY_ORDER.indexOf(a.quality),
    )
    return sorted[0]?.quality ?? ''
}

const selected = ref(getDefaultQuality())

function formatSize(bytes: number): string {
    return `${(bytes / 1048576).toFixed(2)} MB`
}

defineExpose({ selected })
</script>

<style scoped>
.quality-selector {
    max-height: min(50vh, 320px);
    overflow-y: auto;
    overscroll-behavior: contain;
}

.quality-group {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-1);
    min-width: 0;
}

/* 每个品质是一行 list item，命中区域不小于 48dp。 */
.quality-option.n-radio {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    width: 100%;
    min-height: var(--md-target-min);
    padding: var(--md-space-1) var(--md-space-3);
    border-radius: var(--md-shape-md);
    --n-radio-size: 20px;
    --n-label-padding: 0 0 0 var(--md-space-3);
    --n-color: transparent;
    --n-box-shadow: inset 0 0 0 2px var(--md-outline);
    --n-color-active: var(--md-primary);
    --n-box-shadow-active: inset 0 0 0 2px var(--md-primary);
    --n-dot-color-active: var(--md-on-primary);
    --n-text-color: var(--md-on-surface);
    --n-box-shadow-hover: inset 0 0 0 2px var(--md-on-surface);
    --n-box-shadow-focus: inset 0 0 0 2px var(--md-primary);
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.quality-option.n-radio:hover {
    background-color: var(--md-surface-container-high);
}

/* 选中：容器色调 + 圆点填充 + 字重加粗，三重非颜色独有线索。 */
.quality-option.n-radio--checked {
    background-color: var(--md-secondary-container);
}

.quality-option :deep(.n-radio__label) {
    flex: 1;
    min-width: 0;
}

.quality-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
}

.quality-option.n-radio--checked .quality-name {
    font-weight: var(--md-weight-bold);
    color: var(--md-on-secondary-container);
}

.quality-option.n-radio--checked .quality-size {
    color: var(--md-on-secondary-container);
}

.quality-name {
    overflow-wrap: anywhere;
    font-weight: var(--md-weight-medium);
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    color: var(--md-on-surface);
}

.quality-size {
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
    color: var(--md-on-surface-variant);
}

@media (prefers-reduced-motion: reduce) {
    .quality-option.n-radio {
        transition: none;
    }
}
</style>

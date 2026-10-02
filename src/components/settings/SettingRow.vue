<template>
    <li
        class="setting-item"
        :class="{ 'is-stacked': stacked, 'is-destructive': destructive }"
    >
        <div class="setting-text">
            <span :id="labelId" class="setting-label">
                <slot name="label">{{ label }}</slot>
            </span>
            <div
                v-if="description || $slots.description"
                class="setting-description"
            >
                <slot name="description">{{ description }}</slot>
            </div>
        </div>
        <div class="setting-control">
            <slot :label-id="labelId" />
        </div>
    </li>
</template>

<script setup lang="ts">
import { useId } from 'vue'

/**
 * 单个设置项行（M3 list item）：标签 + 可选说明 + 控件。
 *
 * 为什么不在每个设置组件里各写一份行结构：
 * 1. 设置页有 15 个以上设置项，行内边距、字号、间距必须完全一致，复制样式必然漂移；
 * 2. 行内控件的可访问名称需要一个稳定的 id，统一在这里生成才能保证每个控件
 *    都能通过 aria-labelledby 指到自己的标签文字（纯视觉相邻对读屏软件无效）。
 */
withDefaults(
    defineProps<{
        /** 设置项名称；需要富文本时改用 #label 插槽 */
        label?: string
        /** 补充说明，以 --md-body-small 呈现；需要富文本时改用 #description 插槽 */
        description?: string
        /** 控件换行到标签下方，供输入框、单选组等较宽控件使用 */
        stacked?: boolean
        /** 破坏性设置项：标签使用 error 角色，与常规设置形成视觉区分 */
        destructive?: boolean
    }>(),
    { label: '', description: '', stacked: false, destructive: false },
)

defineSlots<{
    default?: (props: { labelId: string }) => unknown
    label?: () => unknown
    description?: () => unknown
}>()

/** 供行内控件绑定 aria-labelledby 的标签 id（useId 在 Vue 3.5 可用） */
const labelId = `setting-label-${useId()}`
</script>

<style scoped>
.setting-item {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--md-space-4);
    min-height: 56px;
    padding: var(--md-space-3) var(--md-space-4);
    margin: 0;
    list-style: none;
}

/* 紧凑（手机）视口及宽控件场景：控件换行，避免挤压标签文字或产生横向溢出 */
.setting-item.is-stacked {
    flex-direction: column;
    align-items: stretch;
    gap: var(--md-space-3);
    padding: var(--md-space-4);
}

.setting-text {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--md-space-1);
    /* 允许文字换行而不是把行撑宽（M3：文字放大不裁切） */
    min-width: 0;
}

.setting-label {
    display: block;
    color: var(--md-on-surface);
    font-size: var(--md-body-large);
    font-weight: var(--md-weight-medium);
    line-height: var(--md-body-large-line);
    overflow-wrap: anywhere;
}

.setting-description {
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    overflow-wrap: anywhere;
}

/* 破坏性动作：颜色只是强化，真正表达“破坏性”的是确认对话框与动作文案本身 */
.setting-item.is-destructive .setting-label {
    color: var(--md-error);
}

.setting-control {
    display: flex;
    flex: 0 1 auto;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--md-space-2);
    min-width: 0;
}

.is-stacked .setting-control {
    justify-content: flex-start;
}
</style>

<template>
    <div class="search-bar">
        <!-- 平台选择：前置的次级操作。只有一个平台时整块隐藏 —— 不可用的选择器
             只会在输入区前堆一个假控件，藏起来视觉最干净。 -->
        <template v-if="showPlatformPicker">
            <n-dropdown
                :options="platformDropdownOptions"
                trigger="click"
                @select="handlePlatformSelect"
            >
                <button
                    type="button"
                    class="platform-btn"
                    :title="currentPlatformLabel"
                    :aria-label="currentPlatformLabel"
                >
                    <span class="platform-label">{{
                        currentPlatformLabel
                    }}</span>
                    <svg
                        class="platform-arrow"
                        viewBox="0 0 24 24"
                        width="18"
                        height="18"
                        aria-hidden="true"
                    >
                        <path
                            d="m7 10 5 5 5-5"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        />
                    </svg>
                </button>
            </n-dropdown>
            <!-- 1px 竖分隔符：把次级操作与输入区在视觉上切开 -->
            <span class="search-divider" aria-hidden="true"></span>
        </template>

        <!-- 输入区：无边框、无独立底色，与容器同形 -->
        <n-input
            v-model:value="keywordModel"
            :placeholder="placeholder"
            @keyup.enter="handleSearch"
            class="search-input"
        />

        <!-- 清空：有内容时才出现的次级操作，命中区 48dp -->
        <button
            v-if="keywordModel.length > 0"
            type="button"
            class="search-clear-btn"
            aria-label="清空搜索关键词"
            @click="handleClear"
        >
            <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
                <path
                    d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                    fill="currentColor"
                />
            </svg>
        </button>

        <!-- 搜索：区域内唯一的 filled 主操作，48x48、图标 24dp -->
        <n-button
            type="primary"
            @click="handleSearch"
            :disabled="!keywordModel.trim() || loading"
            :loading="loading"
            :aria-label="buttonText"
            class="search-btn"
        >
            <!-- 图标按钮：文字仅作可访问名，为输入框让出宽度 -->
            <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
                <circle
                    cx="11"
                    cy="11"
                    r="7"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                />
                <path
                    d="m16.2 16.2 4.3 4.3"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                />
            </svg>
        </n-button>
    </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { NInput, NButton, NDropdown } from 'naive-ui'
import type { PlatformOption } from '../../config/platforms'

const props = withDefaults(
    defineProps<{
        keyword: string
        placeholder?: string
        buttonText?: string
        loading?: boolean
        platform: string
        platformOptions: PlatformOption[]
    }>(),
    {
        placeholder: '搜索歌曲、歌手、专辑',
        buttonText: '搜索',
        loading: false,
    },
)

const emit = defineEmits<{
    (e: 'update:keyword', value: string): void
    (e: 'update:platform', value: string): void
    (e: 'search'): void
    (e: 'clear'): void
}>()

const keywordModel = ref(props.keyword)

// 只有一个平台时没有可选项，平台选择器整块隐藏（组件对外契约不变）。
const showPlatformPicker = computed(() => props.platformOptions.length > 1)

// 当前平台对应的显示 label
const currentPlatformLabel = computed(() => {
    const found = props.platformOptions.find((p) => p.key === props.platform)
    return found ? found.label : props.platform
})

// 下拉选项格式：Naive UI 需要 { label, key } 结构。
// 菜单项显示平台正式名（酷狗音乐、哔哩哔哩…），触发器仍用短 label 省窄屏宽度。
const platformDropdownOptions = computed(() => {
    return props.platformOptions.map((p) => ({
        label: p.name,
        key: p.key,
    }))
})

// 平台选择处理：触发 update:platform 事件，父组件更新 platform 值
function handlePlatformSelect(key: string) {
    emit('update:platform', key)
}

// 向上同步 keyword
watch(keywordModel, (val) => {
    emit('update:keyword', val)
})

// 向下同步 keyword：当父组件 keyword 变化时更新输入框
watch(
    () => props.keyword,
    (newVal) => {
        if (newVal !== keywordModel.value) {
            keywordModel.value = newVal
        }
    },
)

function handleSearch() {
    if (keywordModel.value.trim()) {
        emit('search')
    }
}

function handleClear() {
    // 清空输入并通知父组件清理页面状态（与 Naive 自带 clearable 的顺序一致）
    keywordModel.value = ''
    emit('clear')
}
</script>

<style scoped>
/* M3 search bar：单一容器、单一形状语言。
   56dp 高、全圆角、surface-container-high；默认态没有描边也没有外框阴影，
   内部控件一律退成「无边框 + 无独立底色」，只有聚焦时才出现 ring。 */
.search-bar {
    /* 容器内水平内边距：4(容器 padding) + 12 = 16dp，对齐 M3 的 16dp 起始留白 */
    --search-inline-pad: var(--md-space-3);

    display: flex;
    align-items: center;
    min-width: 0;
    min-height: var(--search-height);
    padding: var(--search-padding);
    border: none;
    border-radius: var(--search-radius);
    background-color: var(--search-bg);
    box-shadow: none;
    /* 透明 ring 常驻，聚焦时只换颜色：不占高度、不引起任何布局位移 */
    outline: 2px solid transparent;
    outline-offset: 1px;
    transition: outline-color var(--md-duration-short) var(--md-easing-standard);
    -webkit-tap-highlight-color: transparent;
}

.search-bar:focus-within {
    outline-color: var(--md-primary);
}

/* ---------- 次级操作（平台选择 / 清空）：透明底 + 状态层 ---------- */

.search-bar .platform-btn,
.search-bar .search-clear-btn {
    position: relative;
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: var(--md-target-min);
    padding: 0;
    border: none;
    border-radius: var(--md-shape-full);
    background-color: transparent;
    color: var(--md-on-surface-variant);
    font-family: var(--md-font-plain);
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
}

/* M3 状态层：用 currentColor + 不透明度表达 hover/pressed，状态不只靠颜色 */
.search-bar .platform-btn::before,
.search-bar .search-clear-btn::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--md-duration-short) var(--md-easing-standard);
}

@media (hover: hover) {
    .search-bar .platform-btn:hover::before,
    .search-bar .search-clear-btn:hover::before {
        opacity: var(--md-state-hover);
    }
}

.search-bar .platform-btn:active::before,
.search-bar .search-clear-btn:active::before {
    opacity: var(--md-state-pressed);
}

.search-bar .platform-btn:focus-visible,
.search-bar .search-clear-btn:focus-visible {
    outline: 2px solid var(--md-primary);
    outline-offset: 0;
}

.search-bar .platform-btn {
    flex: 0 1 auto;
    gap: var(--md-space-1);
    min-width: 0;
    max-width: 40%;
    padding: 0 var(--search-inline-pad);
    font-size: var(--md-body-medium);
    font-weight: var(--md-weight-medium);
    line-height: var(--md-body-medium-line);
}

.platform-label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.platform-arrow {
    flex-shrink: 0;
}

.search-divider {
    flex: 0 0 auto;
    width: 1px;
    height: var(--md-space-6);
    margin: 0 var(--md-space-2);
    background-color: var(--md-outline-variant);
}

.search-bar .search-clear-btn {
    width: var(--md-target-min);
}

/* ---------- 输入区：与容器同形，自身不画边框与底色 ---------- */

.search-bar .search-input {
    flex: 1 1 0%;
    min-width: 0;
    height: var(--md-target-min);
    padding: 0;
    border: none;
    border-radius: var(--md-shape-full);
    background-color: transparent;
    box-shadow: none;
    color: var(--md-on-surface);
    /* 注意：Naive 会把主题写成元素上的内联 --n-* 变量，内联变量压过类选择器，
       因此这里（以及下面 :deep 内）必须写真实属性，不能只覆盖 --n-* 变量。 */
}

.search-input :deep(.n-input-wrapper) {
    padding-left: var(--search-inline-pad);
    padding-right: var(--md-space-2);
}

.search-input :deep(.n-input__input-el) {
    font-size: var(--search-font-size);
    color: var(--md-on-surface);
    caret-color: var(--md-primary);
}

.search-input :deep(.n-input__placeholder) {
    font-size: var(--search-font-size);
    color: var(--md-on-surface-variant);
}

.search-input :deep(.n-input__border),
.search-input :deep(.n-input__state-border) {
    display: none;
}

.search-input :deep(.n-input__suffix) {
    align-items: center;
}

/* ---------- 主操作：唯一的 filled 圆形搜索按钮 ---------- */

.search-bar .search-btn {
    flex: 0 0 auto;
    width: var(--md-target-min);
    min-width: var(--md-target-min);
    height: var(--md-target-min);
    min-height: var(--md-target-min);
    padding: 0;
    border-radius: var(--md-shape-full);
}

.search-bar .search-btn :deep(.n-button__content) {
    display: flex;
    align-items: center;
    justify-content: center;
}
</style>

<template>
    <div class="search-bar">
        <!-- 平台选择：贴靠在文本框前缘的前置操作，不占据输入区宽度 -->
        <n-dropdown
            :options="platformDropdownOptions"
            trigger="click"
            @select="handlePlatformSelect"
        >
            <n-button class="platform-btn" :title="currentPlatformLabel">
                <span class="platform-label">{{ currentPlatformLabel }}</span>
                <svg
                    class="platform-arrow"
                    viewBox="0 0 24 24"
                    width="16"
                    height="16"
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
            </n-button>
        </n-dropdown>

        <n-input
            v-model:value="keywordModel"
            :placeholder="placeholder"
            clearable
            @keyup.enter="handleSearch"
            @clear="handleClear"
            class="search-input"
        />
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

// 当前平台对应的显示 label
const currentPlatformLabel = computed(() => {
    const found = props.platformOptions.find((p) => p.key === props.platform)
    return found ? found.label : props.platform
})

// 下拉选项格式：Naive UI 需要 { label, key } 结构
const platformDropdownOptions = computed(() => {
    return props.platformOptions.map((p) => ({
        label: p.label,
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
    // 点击清空按钮时，输入框已经变为空，同时通知父组件清理页面状态
    emit('clear')
}
</script>

<style scoped>
/* M3 search bar：56dp 高、全圆角、surface-container-high 容器，
   内部为「平台前置操作 + 无边框文本框 + 搜索动作」。 */
.search-bar {
    display: flex;
    align-items: center;
    gap: var(--md-space-1);
    min-width: 0;
    min-height: var(--search-height);
    padding: var(--md-space-1);
    /* 不画 1px 边框，让内部控件正好占满 48dp（56 - 4 - 4）；
       聚焦轮廓改用 inset 阴影，视觉等价且不占高度。 */
    border: none;
    border-radius: var(--search-radius);
    background-color: var(--search-bg);
    box-shadow: var(--search-shadow);
    transition: box-shadow var(--md-duration-short) var(--md-easing-standard);
}

.search-bar:focus-within {
    box-shadow:
        var(--search-shadow-focus),
        inset 0 0 0 1px var(--md-outline-variant);
}

/* 平台选择是次级动作：tonal 外形，与 filled 的搜索按钮区分主次。 */
.search-bar .platform-btn {
    flex-shrink: 0;
    max-width: 40vw;
    /* 命中区域 48dp：搜索栏内容盒正好 48dp（56 - 4 - 4）。 */
    min-height: var(--md-target-min);
    padding: 0 var(--md-space-3);
    border-radius: var(--md-shape-full);
    font-size: var(--md-label-large);
    --n-color: var(--md-secondary-container);
    --n-color-hover: var(--md-secondary-hover);
    --n-color-pressed: var(--md-secondary-pressed);
    --n-color-focus: var(--md-secondary-container);
    --n-text-color: var(--md-on-secondary-container);
    --n-text-color-hover: var(--md-on-secondary-container);
    --n-text-color-pressed: var(--md-on-secondary-container);
    --n-text-color-focus: var(--md-on-secondary-container);
    --n-border: none;
    --n-border-hover: none;
    --n-border-pressed: none;
    --n-border-focus: none;
    --n-border-radius: var(--md-shape-full);
}

.platform-label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.platform-arrow {
    flex-shrink: 0;
    margin-left: var(--md-space-1);
    color: currentColor;
}

/* 文本框退到容器里，自身不画边框与底色。 */
.search-bar .search-input {
    flex: 1;
    min-width: 0;
    --n-height: var(--md-target-min);
    --n-border: none;
    --n-border-hover: none;
    --n-border-focus: none;
    --n-border-disabled: none;
    --n-box-shadow-focus: none;
    --n-color: transparent;
    --n-color-focus: transparent;
    --n-color-disabled: transparent;
    --n-text-color: var(--md-on-surface);
    --n-text-color-focus: var(--md-on-surface);
    --n-placeholder-color: var(--md-on-surface-variant);
    --n-caret-color: var(--md-primary);
    --n-padding-left: var(--md-space-2);
    --n-padding-right: var(--md-space-2);
}

.search-input :deep(.n-input-wrapper) {
    padding-left: var(--n-padding-left);
    padding-right: var(--n-padding-right);
}

.search-input :deep(.n-input__input-el) {
    font-size: var(--search-font-size);
}

.search-input :deep(.n-input__border),
.search-input :deep(.n-input__state-border) {
    display: none;
}

.search-input :deep(.n-input__suffix) {
    align-items: center;
}

/* 搜索动作：区域内唯一的 filled 主操作，图标 24dp、命中区域 48dp。 */
.search-bar .search-btn {
    flex-shrink: 0;
    width: var(--md-target-min);
    min-height: var(--md-target-min);
    padding: 0;
    border-radius: var(--md-shape-full);
    --n-border-radius: var(--md-shape-full);
}

.search-bar .search-btn :deep(.n-button__content) {
    display: flex;
    align-items: center;
    justify-content: center;
}
</style>

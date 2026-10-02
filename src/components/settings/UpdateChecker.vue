<template>
    <!-- 检查更新入口 -->
    <div v-if="showEntry" class="check-update-entry">
        <n-button
            type="primary"
            secondary
            :loading="checkingUpdate"
            @click="handleCheckUpdate"
        >
            {{ updateButtonText }}
        </n-button>
    </div>

    <!-- 更新信息弹窗：最大宽度 600px，窄屏左右留白 16px -->
    <n-modal
        v-if="showModal"
        v-model:show="showUpdateModal"
        preset="card"
        class="update-modal"
        title="发现新版本"
        style="max-width: 600px; width: calc(100% - 32px)"
    >
        <div v-if="updateInfo" class="update-content">
            <p class="version-line">
                当前版本：{{ updateInfo.current_version }}
                <template v-if="isNewVersion">
                    ｜ 最新版本：{{ updateInfo.tag_name }}
                </template>
            </p>
            <p v-if="updateInfo.published_at" class="publish-date">
                发布时间：{{ updateInfo.published_at }}
            </p>
            <div class="update-body">
                <n-text class="body-label">更新内容：</n-text>
                <!-- 使用 v-html 渲染 Markdown 解析后的 HTML，提升可读性 -->
                <!-- 调用 renderMarkdown 函数生成安全 HTML；若无内容则显示默认文本 -->
                <div
                    class="body-text markdown-body"
                    v-html="
                        renderMarkdown(updateInfo.body) ||
                        '<p>（无更新说明）</p>'
                    "
                ></div>
            </div>
            <!-- 下载安装包直链区域（当存在匹配当前平台的 assets 时显示） -->
            <!-- 检查更新功能优化，只显示当前平台可用的安装包，避免用户下载错误文件 -->
            <div v-if="filteredAssets.length > 0" class="assets-section">
                <n-text class="body-label">下载安装包：</n-text>
                <div class="asset-list">
                    <!-- 使用 filteredAssets 计算属性，其根据 currentPlatform 过滤原始 assets -->
                    <a
                        v-for="asset in filteredAssets"
                        :key="asset.name"
                        class="asset-link"
                        :href="asset.browser_download_url"
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        {{ asset.name }}（{{ formatFileSize(asset.size) }}）
                    </a>
                </div>
            </div>
        </div>
        <template #footer>
            <div v-if="updateInfo" class="modal-actions">
                <n-button type="primary" @click="showUpdateModal = false"
                    >关闭</n-button
                >
                <n-button
                    v-if="updateInfo.html_url"
                    tag="a"
                    :href="updateInfo.html_url"
                    target="_blank"
                >
                    前往发布页
                </n-button>
            </div>
        </template>
    </n-modal>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUpdateChecker } from '../../composables/useUpdateChecker'

withDefaults(
    defineProps<{
        showEntry?: boolean
        showModal?: boolean
    }>(),
    {
        showEntry: true,
        showModal: false,
    },
)

const {
    checkingUpdate,
    updateInfo,
    showUpdateModal,
    filteredAssets,
    isNewVersion,
    renderMarkdown,
    formatFileSize,
    handleCheckUpdate,
} = useUpdateChecker()

const updateButtonText = computed(() => {
    if (checkingUpdate.value) return '检查更新'
    if (isNewVersion.value) return '打开更新窗口'
    return updateInfo.value ? '已是最新版本' : '检查更新'
})
</script>

<style scoped>
/* 检查更新入口样式：与关于入口类似，居中 */
.check-update-entry {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--md-space-3);
    flex-wrap: wrap;
    margin-top: var(--md-space-6);
}

/* 更新信息弹窗内部样式 */
/* Modal 渲染到 body，通过专属类名限制弹窗高度和内容滚动区域 */
:global(.update-modal) {
    max-height: calc(
        100vh - 32px - var(--safe-area-top) - var(--safe-area-bottom)
    );
    max-height: calc(
        100dvh - 32px - var(--safe-area-top) - var(--safe-area-bottom)
    );
    border-radius: var(--md-shape-xl);
}

:global(.update-modal > .n-card-content) {
    min-height: 0;
    overflow-y: auto;
}

/* Naive 卡片内置的关闭按钮只有 18dp；放大到 48dp 命中区并保留圆形状态层 */
:global(.update-modal .n-card-header__close) {
    box-sizing: border-box;
    width: var(--md-target-min);
    height: var(--md-target-min);
    border-radius: var(--md-shape-full);
    font-size: 22px;
}

.update-content {
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.6;
    /* 增加内容区上下空白，使弹窗不显得拥挤 */
    padding: var(--md-space-2) 0;
}

.version-line {
    color: var(--md-on-surface);
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    font-weight: var(--md-weight-medium);
    margin: 0 0 var(--md-space-2);
}

.publish-date {
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-small);
    line-height: var(--md-body-small-line);
    margin: 0 0 var(--md-space-4);
}

.update-body {
    margin-bottom: var(--md-space-5);
}

.body-label {
    color: var(--md-on-surface);
    font-weight: var(--md-weight-medium);
}

.body-text {
    margin-top: var(--md-space-1);
    /* 适配 Markdown 渲染后的 HTML 内容，取消 pre-wrap 改为正常换行 */
    color: var(--md-on-surface);
    max-height: 300px;
    overflow-y: auto;
    line-height: 1.6;
}

/* Markdown 内容的基础样式，保证标题、列表、代码块等可读 */
.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
    margin: var(--md-space-3) 0 var(--md-space-2);
    font-weight: 600;
}

.markdown-body :deep(p) {
    margin: var(--md-space-2) 0;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
    padding-left: var(--md-space-6);
    margin: var(--md-space-2) 0;
}

.markdown-body :deep(code) {
    background-color: var(--md-surface-container-high);
    padding: 2px 4px;
    border-radius: var(--md-shape-xs);
    font-size: 0.9em;
}

.markdown-body :deep(pre) {
    background-color: var(--md-surface-container-high);
    padding: var(--md-space-3);
    border-radius: var(--md-shape-sm);
    overflow-x: auto;
}

.markdown-body :deep(pre code) {
    background: none;
    padding: 0;
}

.markdown-body :deep(img) {
    max-width: 100%;
    height: auto;
}

.markdown-body :deep(table) {
    display: block;
    max-width: 100%;
    overflow-x: auto;
}

.markdown-body :deep(a) {
    color: var(--md-primary);
}

/* 资产列表样式 */
.assets-section {
    margin-bottom: var(--md-space-5);
}

.asset-list {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-1);
    margin-top: var(--md-space-2);
}

/* 安装包直链是触屏上的主要操作，保证 48dp 触控高度并允许长文件名换行 */
.asset-link {
    display: flex;
    align-items: center;
    min-height: var(--md-target-min);
    padding: 0 var(--md-space-2);
    border-radius: var(--md-shape-sm);
    overflow-wrap: anywhere;
    color: var(--md-primary);
    font-size: var(--md-body-medium);
    text-decoration: none;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.asset-link:hover,
.asset-link:focus-visible {
    background-color: var(--md-state-hover);
    text-decoration: underline;
}

.modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--md-space-3);
    flex-wrap: wrap;
}

@media (max-width: 767px) {
    :global(.update-modal > .n-card-header),
    :global(.update-modal > .n-card-content),
    :global(.update-modal > .n-card__footer) {
        padding-left: var(--md-space-4);
        padding-right: var(--md-space-4);
    }
}
</style>

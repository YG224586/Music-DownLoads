<template>
    <div class="about-view">
        <!-- 提供明确的返回导航入口 -->
        <n-button class="back-button" secondary @click="goBack">
            <template #icon>
                <svg
                    class="back-icon"
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                    focusable="false"
                >
                    <path
                        d="M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z"
                        fill="currentColor"
                    />
                </svg>
            </template>
            返回
        </n-button>

        <header class="about-header">
            <h1 class="app-title">HotDownloader</h1>
            <!-- 直接使用注入的变量，不再硬编码 -->
            <div class="app-version">版本 {{ version }}</div>
            <!-- 更新为与 README 一致的跨平台描述 -->
            <p class="app-description">
                基于共享 Rust 下载核心和 Vue 3 的音乐下载工具，支持 Tauri
                桌面端、Android 端与 Docker/Web
                部署，提供搜索、歌单导入、多任务下载、自动降级、音频解密等功能。
            </p>
        </header>

        <section class="about-section">
            <h2 class="section-title">开源链接</h2>
            <ul class="link-list">
                <li>
                    <!-- 独立成行的链接需要 ≥48dp 的命中区，方便触屏点击 -->
                    <a
                        class="link-row"
                        href="https://github.com/lerdb/HotDownloader"
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <span class="link-label">GitHub 仓库</span>
                        <svg
                            class="link-chevron"
                            viewBox="0 0 24 24"
                            aria-hidden="true"
                            focusable="false"
                        >
                            <path
                                d="M9.29 6.71a1 1 0 0 0 0 1.41L13.17 12l-3.88 3.88a1 1 0 1 0 1.42 1.41l4.58-4.58a1 1 0 0 0 0-1.42l-4.58-4.58a1 1 0 0 0-1.42 0z"
                                fill="currentColor"
                            />
                        </svg>
                    </a>
                </li>
            </ul>
        </section>

        <section class="about-section">
            <h2 class="section-title">开放源代码许可</h2>
            <p class="license-text">
                本项目基于
                <a
                    class="license-link"
                    href="https://www.apache.org/licenses/LICENSE-2.0"
                    target="_blank"
                    rel="noopener noreferrer"
                    >Apache License 2.0</a
                >
                开源。
            </p>
        </section>

        <section class="about-section">
            <h2 class="section-title">第三方组件</h2>

            <h3 class="sub-title">Rust ({{ rustComponents.length }})</h3>
            <ul class="component-list">
                <li v-for="item in rustComponents" :key="item.name">
                    <button
                        type="button"
                        class="component-item"
                        @click="openLicense(item)"
                    >
                        <span class="component-name">{{ item.name }}</span>
                        <span class="component-license">{{
                            item.license
                        }}</span>
                    </button>
                </li>
            </ul>

            <h3 class="sub-title">
                Frontend ({{ frontendComponents.length }})
            </h3>
            <ul class="component-list">
                <li v-for="item in frontendComponents" :key="item.name">
                    <button
                        type="button"
                        class="component-item"
                        @click="openLicense(item)"
                    >
                        <span class="component-name">{{ item.name }}</span>
                        <span class="component-license">{{
                            item.license
                        }}</span>
                    </button>
                </li>
            </ul>
        </section>

        <n-modal
            v-model:show="showModal"
            preset="card"
            class="license-modal"
            :title="modalTitle"
            style="width: min(720px, 92vw); max-height: 80vh"
            :bordered="false"
        >
            <n-scrollbar style="max-height: 60vh">
                <pre class="license-fulltext">{{ modalText }}</pre>
            </n-scrollbar>
        </n-modal>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NButton, NModal, NScrollbar } from 'naive-ui'
import { useRouter } from 'vue-router'
import {
    rustComponents,
    frontendComponents,
    licenseTexts,
    type ComponentInfo,
} from '../data/licenses'

const version = import.meta.env.VITE_APP_VERSION

const router = useRouter()

const showModal = ref(false)
const modalTitle = ref('')
const modalText = ref('')

// 处理返回按钮点击逻辑，确保用户能正确回到上一页
function goBack() {
    // 若存在历史记录则直接后退，否则回退到设置页
    if (window.history.length > 1) {
        router.back()
    } else {
        router.push('/settings')
    }
}

// 点击组件行时，弹出该组件涉及的许可证全文
function openLicense(item: ComponentInfo) {
    const ids = new Set<string>()
    const tokens = item.license
        .replace(/[()]/g, ' ')
        .split(/\s+/)
        .filter(Boolean)
    for (let i = 0; i < tokens.length; i++) {
        const t = tokens[i]
        if (t === 'OR' || t === 'AND') continue
        if (t === 'WITH') {
            i++
            continue
        }
        ids.add(t)
    }

    const parts: string[] = []
    for (const id of ids) {
        const found = licenseTexts.find((l) => l.id === id)
        if (found)
            parts.push(`── ${found.name} (${found.id}) ──\n\n${found.text}`)
    }

    modalTitle.value = `${item.name} @ ${item.version}`
    modalText.value =
        parts.length > 0 ? parts.join('\n\n') : '未找到许可证全文。'
    showModal.value = true
}
</script>

<style scoped>
.about-view {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-4);
    /* 长文限制可读宽度，避免桌面端一行过长 */
    max-width: 800px;
    min-width: 0;
    margin: 0 auto;
    padding: 0;
}

.back-button {
    align-self: flex-start;
}

.back-icon {
    width: 20px;
    height: 20px;
}

.about-header {
    padding: var(--md-space-5) var(--md-space-4);
    border-radius: var(--md-shape-lg);
    background-color: var(--md-surface-container-low);
    text-align: center;
}

.app-title {
    margin: 0 0 var(--md-space-1);
    color: var(--md-on-surface);
    font-size: var(--md-headline-small);
    line-height: var(--md-headline-small-line);
    font-weight: var(--md-weight-bold);
}

.app-version {
    margin-bottom: var(--md-space-4);
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
}

.app-description {
    margin: 0 auto;
    max-width: 56ch;
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-medium);
    line-height: 1.6;
}

.about-section {
    padding: var(--md-space-4) var(--md-space-4) var(--md-space-2);
    border-radius: var(--md-shape-lg);
    background-color: var(--md-surface-container-low);
}

.section-title {
    margin: 0 0 var(--md-space-3);
    color: var(--md-on-surface);
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
}

.sub-title {
    margin: var(--md-space-4) 0 var(--md-space-2);
    color: var(--md-on-surface);
    font-size: var(--md-title-small);
    line-height: var(--md-title-small-line);
    font-weight: var(--md-weight-medium);
}

.link-list,
.component-list {
    list-style: none;
    padding: 0;
    margin: 0;
}

.link-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-3);
    box-sizing: border-box;
    min-height: 56px;
    padding: 0 var(--md-space-2);
    border-radius: var(--md-shape-sm);
    color: var(--md-primary);
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    text-decoration: none;
    overflow-wrap: anywhere;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.link-row:hover,
.link-row:focus-visible {
    background-color: var(--md-state-hover);
    text-decoration: underline;
}

.link-chevron {
    width: 24px;
    height: 24px;
    flex: 0 0 24px;
    color: var(--md-on-surface-variant);
}

.license-text {
    margin: 0;
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-medium);
    line-height: 1.6;
}

/* 句子中的内联链接：用 inline-flex 撑出 48dp 命中区，同时保持与正文对齐 */
.license-link {
    display: inline-flex;
    align-items: center;
    min-height: var(--md-target-min);
    margin: 0 var(--md-space-1);
    padding: 0 var(--md-space-2);
    border-radius: var(--md-shape-sm);
    vertical-align: middle;
    color: var(--md-primary);
    overflow-wrap: anywhere;
}

.license-link:hover,
.license-link:focus-visible {
    background-color: var(--md-state-hover);
    text-decoration: underline;
}

/* 组件行是打开许可证全文的入口，做成整行按钮以保证键盘可达且 ≥48dp */
.component-item {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    box-sizing: border-box;
    width: 100%;
    min-height: 56px;
    padding: var(--md-space-2) var(--md-space-2);
    gap: var(--md-space-1) var(--md-space-4);
    flex-wrap: wrap;
    border: 0;
    border-bottom: 1px solid var(--md-outline-variant);
    border-radius: 0;
    background: transparent;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.component-list li:last-child .component-item {
    border-bottom: none;
}

.component-item:hover,
.component-item:focus-visible {
    background-color: var(--md-state-hover);
}

.component-name {
    overflow-wrap: anywhere;
    color: var(--md-on-surface);
    font-size: var(--md-body-medium);
    line-height: var(--md-body-medium-line);
}

.component-license {
    overflow-wrap: anywhere;
    color: var(--md-on-surface-variant);
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
}

.license-fulltext {
    margin: 0;
    color: var(--md-on-surface-variant);
    font-family: var(--md-font-plain);
    font-size: var(--md-label-medium);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
}

:global(.license-modal) {
    border-radius: var(--md-shape-xl);
}

/* Naive 卡片内置的关闭按钮只有 18dp；放大到 48dp 命中区并保留圆形状态层 */
:global(.license-modal .n-card-header__close) {
    box-sizing: border-box;
    width: var(--md-target-min);
    height: var(--md-target-min);
    border-radius: var(--md-shape-full);
    font-size: 22px;
}

@media (max-width: 599px) {
    .about-header,
    .about-section {
        padding: var(--md-space-4) var(--md-space-3);
    }

    .about-section {
        padding-bottom: var(--md-space-1);
    }
}
</style>

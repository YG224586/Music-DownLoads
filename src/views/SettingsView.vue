<template>
    <div class="settings-view">
        <n-alert
            v-if="settingsStore.saveError"
            type="error"
            class="settings-alert"
        >
            设置保存失败：{{ settingsStore.saveError }}
        </n-alert>
        <n-alert
            v-if="settingsStore.conflictFields.length"
            type="warning"
            class="settings-alert"
        >
            以下设置已在其他页面更新，请逐项选择要保留的值：
            <div
                v-for="field in settingsStore.conflictFields"
                :key="field"
                class="conflict-row"
            >
                <span class="conflict-label">{{ settingLabel(field) }}</span>
                <n-button @click="settingsStore.resolveConflict(field, false)"
                    >使用最新设置</n-button
                >
                <n-button
                    type="primary"
                    @click="settingsStore.resolveConflict(field, true)"
                    >保留本页修改</n-button
                >
            </div>
        </n-alert>

        <!-- 按主题分组：组标题 + 列表项 + 分隔线（移动端优先，避免每项套卡片） -->
        <section class="settings-group">
            <h2 class="group-title">账号设置</h2>
            <ul class="settings-list">
                <LoginSetting />
            </ul>
        </section>

        <section class="settings-group">
            <h2 class="group-title">基本设置</h2>
            <ul class="settings-list">
                <BuiltinSourceSetting />
                <QualitySetting />
                <DowngradeSetting />
                <ClearHistoryButton />
            </ul>
        </section>

        <section class="settings-group">
            <h2 class="group-title">自定义音源</h2>
            <ul class="settings-list">
                <CustomSourceSetting />
            </ul>
        </section>

        <section class="settings-group">
            <h2 class="group-title">下载设置</h2>
            <ul class="settings-list">
                <DirectorySetting />
                <NamingTemplate />
                <ArtistSeparator />
                <NamingPreview />
                <WriteMetadataSetting />
                <DownloadLrcSetting />
                <ConcurrencySetting />
                <JumpToTaskSetting />
                <DuplicateStrategySetting />
                <NotifySetting v-if="native" />
            </ul>
        </section>

        <!-- 检查更新组件 -->
        <UpdateChecker v-if="native" />

        <!-- 关于入口（始终位于页面底部） -->
        <section class="settings-group about-group">
            <ul class="settings-list">
                <li class="nav-item">
                    <button type="button" class="nav-row" @click="goAbout">
                        <span class="nav-label">关于 音乐下载</span>
                        <svg
                            class="nav-chevron"
                            viewBox="0 0 24 24"
                            aria-hidden="true"
                            focusable="false"
                        >
                            <path
                                d="M9.29 6.71a1 1 0 0 0 0 1.41L13.17 12l-3.88 3.88a1 1 0 1 0 1.42 1.41l4.58-4.58a1 1 0 0 0 0-1.42l-4.58-4.58a1 1 0 0 0-1.42 0z"
                                fill="currentColor"
                            />
                        </svg>
                    </button>
                </li>
            </ul>
        </section>
    </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { NAlert, NButton } from 'naive-ui'
import QualitySetting from '../components/settings/QualitySetting.vue'
import DowngradeSetting from '../components/settings/DowngradeSetting.vue'
import DirectorySetting from '../components/settings/DirectorySetting.vue'
import NamingTemplate from '../components/settings/NamingTemplate.vue'
import ArtistSeparator from '../components/settings/ArtistSeparator.vue'
import NamingPreview from '../components/settings/NamingPreview.vue'
import ConcurrencySetting from '../components/settings/ConcurrencySetting.vue'
import JumpToTaskSetting from '../components/settings/JumpToTaskSetting.vue'
import ClearHistoryButton from '../components/settings/ClearHistoryButton.vue'
import WriteMetadataSetting from '../components/settings/WriteMetadataSetting.vue'
import DownloadLrcSetting from '../components/settings/DownloadLrcSetting.vue'
import LoginSetting from '../components/settings/LoginSetting.vue'
import BuiltinSourceSetting from '../components/settings/BuiltinSourceSetting.vue'
import CustomSourceSetting from '../components/settings/CustomSourceSetting.vue'
import DuplicateStrategySetting from '../components/settings/DuplicateStrategySetting.vue'
import NotifySetting from '../components/settings/NotifySetting.vue'
import UpdateChecker from '../components/settings/UpdateChecker.vue'
import { isNativeRuntime } from '../api/runtimeApi'
import { useSettingsStore } from '../stores/settingsStore'
import type { Settings } from '../types'

const router = useRouter()

const native = isNativeRuntime()
const settingsStore = useSettingsStore()

const settingLabels: Partial<Record<keyof Settings, string>> = {
    defaultQuality: '默认音质',
    autoDowngrade: '自动降级',
    qualityDowngradeOrder: '音质降级顺序',
    downloadDir: '下载目录',
    namingTemplate: '文件命名规则',
    maxConcurrent: '并发下载数',
    jumpToTask: '添加后跳转任务',
    artistSeparator: '歌手连接符',
    safFolderUri: 'Android 文件夹',
    safFolderName: 'Android 文件夹名称',
    writeMetadata: '写入元数据',
    downloadLrc: '下载歌词',
    duplicateStrategy: '重复文件处理',
    notifyOnComplete: '完成通知',
}

function settingLabel(field: keyof Settings): string {
    return settingLabels[field] ?? field
}

function goAbout() {
    router.push('/settings/about')
}
</script>

<style scoped>
.settings-view {
    width: 100%;
    max-width: 800px;
    min-width: 0;
    /* 让设置页占满父容器高度，使用 flex 列布局 */
    display: flex;
    flex-direction: column;
    min-height: 100%;
}

.settings-alert {
    margin-bottom: var(--md-space-4);
}

.conflict-row {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
    flex-wrap: wrap;
    margin-top: var(--md-space-2);
}

.conflict-label {
    flex: 1 1 120px;
    min-width: 0;
    overflow-wrap: anywhere;
}

.settings-group {
    margin-bottom: var(--md-space-6);
    min-width: 0;
}

/* 分组标题：title-small + 重字重 + on-surface-variant（不用主色，避免与操作色抢视觉权重） */
.group-title {
    margin: 0 0 var(--md-space-2);
    padding: 0 var(--md-space-4);
    color: var(--md-on-surface-variant);
    font-size: var(--md-title-small);
    line-height: var(--md-title-small-line);
    font-weight: var(--md-weight-bold);
}

.settings-list {
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
}

/* 列表项之间用分隔线，而不是把每一项做成卡片 */
.settings-list :deep(.setting-item + .setting-item) {
    border-top: 1px solid var(--md-outline-variant);
}

/* 触控目标兜底：本选择器比全局 body .n-button 更具体，
   保证设置页里的按钮（含 size="small"）命中区不低于 48dp。 */
.settings-view :deep(.n-button) {
    min-height: var(--md-target-min);
}

/* 关于入口：整行可点击的导航项，始终位于页面底部 */
.about-group {
    margin-top: auto;
    margin-bottom: 0;
    padding-top: var(--md-space-6);
}

.nav-item {
    list-style: none;
}

.nav-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--md-space-3);
    box-sizing: border-box;
    width: 100%;
    min-height: 56px;
    padding: 0 var(--md-space-4);
    border: 0;
    border-radius: var(--md-shape-sm);
    background: transparent;
    color: var(--md-on-surface);
    font: inherit;
    font-size: var(--md-body-large);
    line-height: var(--md-body-large-line);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

.nav-row:hover {
    background-color: var(--md-state-hover);
}

.nav-row:active {
    background-color: var(--md-state-pressed);
}

.nav-label {
    min-width: 0;
    flex: 1 1 auto;
    overflow-wrap: anywhere;
}

.nav-chevron {
    width: 24px;
    height: 24px;
    flex: 0 0 24px;
    color: var(--md-on-surface-variant);
}

/* 窄屏收紧分组间距，保证首屏能看到更多设置项 */
@media (max-width: 599px) {
    .settings-group {
        margin-bottom: var(--md-space-5);
    }

    .group-title {
        padding: 0 var(--md-space-4);
    }
}
</style>

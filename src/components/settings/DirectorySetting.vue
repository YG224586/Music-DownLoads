<template>
    <SettingRow label="下载目录" :stacked="!isWeb">
        <template #description>
            <template v-if="isWeb">
                <span class="dir-line"
                    >服务器下载目录：{{
                        settingsStore.settings.downloadDir
                    }}</span
                >
                <span class="dir-line"
                    >通过 HOTDOWNLOADER_DOWNLOAD_DIR
                    设置容器内路径，并为该路径挂载下载卷。</span
                >
            </template>
            <template v-else-if="isAndroid">
                <span
                    v-if="settingsStore.settings.safFolderName"
                    class="dir-line"
                    >当前 SAF 文件夹：{{
                        settingsStore.settings.safFolderName
                    }}</span
                >
                <span v-else class="dir-line"
                    >默认下载目录：{{
                        settingsStore.settings.downloadDir
                    }}</span
                >
            </template>
        </template>
        <template #default="{ labelId }">
            <n-input-group v-if="!isWeb && !isAndroid" class="dir-input-group">
                <n-input
                    :value="settingsStore.settings.downloadDir"
                    readonly
                    placeholder="请选择下载目录"
                    :input-props="{ 'aria-labelledby': labelId }"
                />
                <n-button type="primary" @click="selectDirectory"
                    >选择</n-button
                >
            </n-input-group>
            <n-button
                v-else-if="isAndroid"
                type="primary"
                @click="selectSafFolder"
            >
                选择 SAF 文件夹
            </n-button>
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { NInput, NInputGroup, NButton } from 'naive-ui'
import { chooseDownloadDirectory, pickSafFolder } from '../../api/fileApi'
import { getRuntimePlatform } from '../../api/runtimeApi'
import { useSettingsStore } from '../../stores/settingsStore'
import SettingRow from './SettingRow.vue'

const settingsStore = useSettingsStore()

// 原生平台信息由 API 层提供，初始值为 false
const isAndroid = ref(false)
const isWeb = ref(false)

// Android 端初始化：异步获取平台信息，若为 Android 且未选择 SAF，则确保使用默认下载目录
onMounted(async () => {
    try {
        const currentPlatform = await getRuntimePlatform()
        isAndroid.value = currentPlatform === 'android'
        isWeb.value = currentPlatform === 'web'
    } catch (error) {
        console.warn('获取平台信息失败，默认按非 Android 处理', error)
        isAndroid.value = false
    }

    if (isAndroid.value && !settingsStore.settings.safFolderUri) {
        await settingsStore.getDefaultDownloadDir()
    }
})

// 目录选择
async function selectDirectory() {
    // 仅原生端调用
    try {
        const selected = await chooseDownloadDirectory()
        if (selected) {
            settingsStore.settings.downloadDir = selected
        }
    } catch (error) {
        console.error('选择目录失败:', error)
    }
}

async function selectSafFolder() {
    try {
        const json = await pickSafFolder()
        if (!json) {
            console.log('用户取消选择 SAF 文件夹')
            return
        }
        // 存储完整 JSON
        settingsStore.settings.safFolderUri = json

        // 解析 JSON 获取 URI 最后一段作为显示名称
        try {
            const parsed = JSON.parse(json)
            settingsStore.settings.safFolderName =
                parsed.uri.split('/').pop() || parsed.uri
        } catch {
            settingsStore.settings.safFolderName = 'SAF 文件夹'
        }
        settingsStore.settings.downloadDir = 'saf://'
    } catch (error) {
        console.error('选择 SAF 文件夹失败:', error)
    }
}
</script>

<style scoped>
/* 输入组在换行布局下占满整行宽度，窄视口下由输入框收缩，不产生横向溢出 */
.dir-input-group {
    width: 100%;
}

.dir-line {
    display: block;
    overflow-wrap: anywhere;
}

.dir-line + .dir-line {
    margin-top: var(--md-space-1);
}
</style>

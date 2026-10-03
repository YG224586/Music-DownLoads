<template>
    <SettingRow label="QQ 音乐账号" :description="description">
        <template #default>
            <n-button
                v-if="isLoggedIn"
                type="error"
                secondary
                :loading="loggingOut"
                @click="confirmLogout"
                >退出登录</n-button
            >
        </template>
    </SettingRow>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { NButton, useDialog } from 'naive-ui'
import * as musicApi from '../../api/musicApi'
import SettingRow from './SettingRow.vue'

/**
 * 账号设置（M3 list item）：只读展示登录状态 + 已登录时提供退出登录。
 *
 * 为什么不再有登录入口：用户要求移除登录入口（m09887），二维码与手动 uin/authst
 * 输入都已删除。但旧版本登录过的用户，本机仍保存着可用凭据（可继续下载与取歌单），
 * 所以必须保留只读状态与「退出登录」——否则凭据将永远无法在本机清除。
 * 退出登录在本版本不可逆（已无登录入口），因此走确认对话框。
 */
const PLATFORM = 'qqmusic'

const dialog = useDialog()

const isLoggedIn = ref(false)
const uin = ref('')
const statusLoading = ref(true)
const statusError = ref('')
const loggingOut = ref(false)

const description = computed(() => {
    if (statusLoading.value) return '正在读取登录状态…'
    if (statusError.value) return `读取登录状态失败：${statusError.value}`
    if (isLoggedIn.value) {
        return uin.value ? `已登录：${uin.value}` : '已登录'
    }
    return '未登录。本版本已移除登录入口，无法在应用内新增登录。'
})

async function refreshStatus() {
    statusLoading.value = true
    statusError.value = ''
    try {
        const status = await musicApi.getLoginStatus(PLATFORM)
        isLoggedIn.value = status.logged_in
        uin.value = status.uin
    } catch (error) {
        statusError.value =
            error instanceof Error ? error.message : String(error)
    } finally {
        statusLoading.value = false
    }
}

async function handleLogout() {
    loggingOut.value = true
    try {
        await musicApi.logout(PLATFORM)
        isLoggedIn.value = false
        uin.value = ''
        statusError.value = ''
    } catch (error) {
        statusError.value =
            error instanceof Error ? error.message : String(error)
    } finally {
        loggingOut.value = false
    }
}

/** 退出不可逆：清除凭据后本版本无法再登录，必须显式确认。 */
function confirmLogout() {
    dialog.warning({
        title: '退出登录',
        showIcon: false,
        content:
            '退出后将清除本机保存的 QQ 音乐凭据。本版本已移除登录入口，退出后无法在应用内重新登录。',
        positiveText: '退出登录',
        negativeText: '取消',
        positiveButtonProps: { type: 'error', text: true },
        negativeButtonProps: { text: true },
        onPositiveClick: () => {
            void handleLogout()
        },
    })
}

onMounted(() => {
    void refreshStatus()
})
</script>

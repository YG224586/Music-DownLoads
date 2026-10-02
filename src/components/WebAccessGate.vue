<template>
    <div class="access-page">
        <n-card class="access-card" title="连接下载服务">
            <template v-if="webSession.mode === 'password'">
                <p class="access-hint">请输入服务部署时配置的用户名和密码。</p>
                <form class="access-form" @submit.prevent="submit">
                    <n-input
                        v-model:value="username"
                        name="username"
                        placeholder="用户名"
                        autocomplete="username"
                        :input-props="{ 'aria-label': '用户名' }"
                    />
                    <div class="secret-row">
                        <n-input
                            v-model:value="password"
                            :type="showPassword ? 'text' : 'password'"
                            name="password"
                            placeholder="密码"
                            autocomplete="current-password"
                            class="secret-input"
                            :input-props="{ 'aria-label': '密码' }"
                        />
                        <n-button
                            class="secret-toggle"
                            :aria-label="showPassword ? '隐藏密码' : '显示密码'"
                            :aria-pressed="showPassword"
                            @click="showPassword = !showPassword"
                        >
                            {{ showPassword ? '隐藏' : '显示' }}
                        </n-button>
                    </div>
                    <n-button
                        type="primary"
                        :loading="webSession.checking"
                        class="access-button"
                        attr-type="submit"
                        >登录</n-button
                    >
                </form>
            </template>
            <template v-else-if="webSession.mode === 'token'">
                <p class="access-hint">请输入服务部署时配置的访问令牌。</p>
                <form class="access-form" @submit.prevent="submit">
                    <div class="secret-row">
                        <n-input
                            v-model:value="token"
                            :type="showToken ? 'text' : 'password'"
                            placeholder="访问令牌"
                            autocomplete="off"
                            class="secret-input"
                            :input-props="{ 'aria-label': '访问令牌' }"
                        />
                        <n-button
                            class="secret-toggle"
                            :aria-label="
                                showToken ? '隐藏访问令牌' : '显示访问令牌'
                            "
                            :aria-pressed="showToken"
                            @click="showToken = !showToken"
                        >
                            {{ showToken ? '隐藏' : '显示' }}
                        </n-button>
                    </div>
                    <n-button
                        type="primary"
                        :loading="webSession.checking"
                        class="access-button"
                        attr-type="submit"
                        >连接</n-button
                    >
                </form>
            </template>
            <template v-else>
                <p class="access-hint" role="status" aria-live="polite">
                    正在连接下载服务…
                </p>
                <n-button
                    v-if="webSession.error"
                    :loading="webSession.checking"
                    @click="authorizeWeb()"
                >
                    重试
                </n-button>
            </template>
            <n-alert
                v-if="webSession.error"
                type="error"
                class="access-error"
                role="alert"
            >
                {{ webSession.error }}
            </n-alert>
        </n-card>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NAlert, NButton, NCard, NInput } from 'naive-ui'
import {
    authorizeWeb,
    authorizeWebWithPassword,
    webSession,
} from '../api/webClient'

const token = ref('')
const username = ref('')
const password = ref('')

// 密码可见性由页面自己的按钮控制，比输入框内置图标更易点击且有可访问名。
const showPassword = ref(false)
const showToken = ref(false)

async function submit() {
    if (webSession.mode === 'password') {
        await authorizeWebWithPassword(username.value, password.value)
        password.value = ''
    } else {
        await authorizeWeb(token.value)
    }
}
</script>

<style scoped>
.access-page {
    /* 单独占满视口，并允许在内容超出（如软键盘弹出）时内部滚动 */
    box-sizing: border-box;
    display: grid;
    place-items: center;
    min-height: 100vh;
    min-height: 100dvh;
    max-height: 100dvh;
    overflow-y: auto;
    padding: calc(var(--md-space-6) + var(--safe-area-top))
        calc(var(--md-space-5) + var(--safe-area-right))
        calc(var(--md-space-6) + var(--safe-area-bottom))
        calc(var(--md-space-5) + var(--safe-area-left));
    background-color: var(--md-surface);
}

.access-card {
    width: min(440px, 100%);
    border-radius: var(--md-shape-xl);
}

.access-card :deep(.n-card-header__main) {
    font-size: var(--md-title-medium);
    line-height: var(--md-title-medium-line);
    font-weight: var(--md-weight-medium);
}

.access-hint {
    margin: 0 0 var(--md-space-4);
    color: var(--md-on-surface-variant);
    font-size: var(--md-body-medium);
    line-height: 1.6;
}

.access-form {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
}

/* 密码框与其可见性按钮并排，两者都不低于 48dp */
.secret-row {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
}

.secret-input {
    flex: 1 1 auto;
    min-width: 0;
}

.secret-toggle {
    flex: 0 0 auto;
}

.access-error {
    margin-top: var(--md-space-3);
}

.access-button {
    width: 100%;
    margin-top: var(--md-space-1);
}

@media (max-width: 599px) {
    .access-page {
        padding: calc(var(--md-space-5) + var(--safe-area-top))
            calc(var(--md-space-4) + var(--safe-area-right))
            calc(var(--md-space-5) + var(--safe-area-bottom))
            calc(var(--md-space-4) + var(--safe-area-left));
    }
}
</style>

<template>
    <div class="access-page">
        <n-card class="access-card" title="连接下载服务" :bordered="false">
            <template v-if="webSession.mode === 'password'">
                <p class="access-hint">请输入服务部署时配置的用户名和密码。</p>
                <form class="access-form" @submit.prevent="submit">
                    <n-input
                        v-model:value="username"
                        placeholder="用户名"
                        autocomplete="username"
                        aria-label="用户名"
                    />
                    <div class="secret-row">
                        <n-input
                            v-model:value="password"
                            class="secret-input"
                            :type="showPassword ? 'text' : 'password'"
                            placeholder="密码"
                            aria-label="密码"
                            autocomplete="current-password"
                        />
                        <button
                            type="button"
                            class="secret-toggle"
                            :aria-label="showPassword ? '隐藏密码' : '显示密码'"
                            :aria-pressed="showPassword"
                            @click="showPassword = !showPassword"
                        >
                            <span
                                class="secret-icon"
                                aria-hidden="true"
                                v-html="
                                    showPassword
                                        ? ICON_VISIBILITY_OFF
                                        : ICON_VISIBILITY
                                "
                            />
                        </button>
                    </div>
                    <n-button
                        class="access-button"
                        type="primary"
                        attr-type="submit"
                        :loading="webSession.checking"
                    >
                        登录
                    </n-button>
                </form>
            </template>

            <template v-else-if="webSession.mode === 'token'">
                <p class="access-hint">请输入服务部署时配置的访问令牌。</p>
                <form class="access-form" @submit.prevent="submit">
                    <div class="secret-row">
                        <n-input
                            v-model:value="token"
                            class="secret-input"
                            :type="showToken ? 'text' : 'password'"
                            placeholder="访问令牌"
                            aria-label="访问令牌"
                            autocomplete="off"
                        />
                        <button
                            type="button"
                            class="secret-toggle"
                            :aria-label="
                                showToken ? '隐藏访问令牌' : '显示访问令牌'
                            "
                            :aria-pressed="showToken"
                            @click="showToken = !showToken"
                        >
                            <span
                                class="secret-icon"
                                aria-hidden="true"
                                v-html="
                                    showToken
                                        ? ICON_VISIBILITY_OFF
                                        : ICON_VISIBILITY
                                "
                            />
                        </button>
                    </div>
                    <n-button
                        class="access-button"
                        type="primary"
                        attr-type="submit"
                        :loading="webSession.checking"
                    >
                        连接
                    </n-button>
                </form>
            </template>

            <template v-else>
                <div class="access-status">
                    <n-spin
                        v-if="webSession.checking && !webSession.error"
                        size="small"
                    />
                    <p
                        class="access-hint access-hint--status"
                        role="status"
                        aria-live="polite"
                    >
                        正在连接下载服务…
                    </p>
                </div>
                <n-button
                    v-if="webSession.error"
                    class="access-retry"
                    secondary
                    :loading="webSession.checking"
                    @click="authorizeWeb()"
                >
                    重试
                </n-button>
            </template>

            <n-alert
                v-if="webSession.error"
                class="access-error"
                type="error"
                role="alert"
            >
                {{ webSession.error }}
            </n-alert>
        </n-card>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NAlert, NButton, NCard, NInput, NSpin } from 'naive-ui'
import {
    authorizeWeb,
    authorizeWebWithPassword,
    webSession,
} from '../api/webClient'

// 图标只作装饰，可访问名由按钮的 aria-label 提供。
const ICON_VISIBILITY =
    '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path fill="currentColor" d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zm0 12.5c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z"/></svg>'
const ICON_VISIBILITY_OFF =
    '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path fill="currentColor" d="M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.16c0-1.66-1.34-3-3-3l-.17.01z"/></svg>'

const token = ref('')
const username = ref('')
const password = ref('')
const showPassword = ref(false)
const showToken = ref(false)

async function submit() {
    // 密码可见性由页面自己的按钮控制，比输入框内置图标更易点击且有可访问名。
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
    display: grid;
    place-items: center;
    min-height: 100vh;
    min-height: 100dvh;
    max-height: 100dvh;
    overflow-y: auto;
    /* 全屏视图：内容超出（例如软键盘顶起）时内部滚动。 */
    padding: calc(var(--md-space-6) + var(--safe-area-top))
        calc(var(--md-space-5) + var(--safe-area-right))
        calc(var(--md-space-6) + var(--safe-area-bottom))
        calc(var(--md-space-5) + var(--safe-area-left));
    background-color: var(--md-surface-container);
}

.access-card {
    width: min(440px, 100%);
    border-radius: var(--md-shape-lg);
    background-color: var(--md-surface-container-lowest);
    box-shadow: var(--md-elevation-1);
    /* M3 Expressive 入场：淡入 + 轻微上浮，走 emphasized-decelerate 曲线。
       fill 用 both，动画结束后停在终态（transform: none），不产生持续布局影响；
       prefers-reduced-motion 全局块会把时长压到 0.01ms。 */
    animation: access-card-enter var(--md-duration-medium)
        var(--md-easing-emphasized-decelerate) both;
}

@keyframes access-card-enter {
    from {
        opacity: 0;
        transform: translateY(8px);
    }
    to {
        opacity: 1;
        transform: none;
    }
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

.access-status {
    display: flex;
    align-items: center;
    gap: var(--md-space-3);
}

.access-hint--status {
    margin: 0;
}

.access-form {
    display: flex;
    flex-direction: column;
    gap: var(--md-space-3);
}

.secret-row {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
}

/* 密码框与其可见性按钮并排，两者都不低于 48dp。 */
.secret-input {
    flex: 1 1 0%;
    min-width: 0;
}

.secret-toggle {
    position: relative;
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    width: var(--md-target-min);
    height: var(--md-target-min);
    padding: 0;
    border: none;
    border-radius: var(--md-shape-full);
    background-color: transparent;
    color: var(--md-on-surface-variant);
    cursor: pointer;
}

.secret-toggle::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    transition: opacity var(--md-duration-short) var(--md-easing-standard);
}

@media (hover: hover) {
    .secret-toggle:hover::before {
        opacity: var(--md-state-hover);
    }
}

.secret-toggle:active::before {
    opacity: var(--md-state-pressed);
}

.secret-toggle:focus-visible {
    outline: 2px solid var(--md-primary);
    outline-offset: 2px;
}

.secret-icon {
    position: relative;
    display: inline-flex;
}

.secret-icon :deep(svg) {
    display: block;
}

.access-retry {
    margin-top: var(--md-space-3);
}

.access-error {
    margin-top: var(--md-space-3);
    border-radius: var(--md-shape-md);
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

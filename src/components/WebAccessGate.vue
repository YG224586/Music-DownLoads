<template>
    <div class="access-page">
        <n-card class="access-card" title="连接下载服务">
            <template v-if="webSession.mode === 'password'">
                <p>请输入服务部署时配置的用户名和密码。</p>
                <form @submit.prevent="submit">
                    <n-input v-model:value="username" name="username" placeholder="用户名" autocomplete="username" />
                    <n-input v-model:value="password" type="password" show-password-on="click"
                        name="password" placeholder="密码" autocomplete="current-password" class="password-input" />
                    <n-button type="primary" :loading="webSession.checking" class="access-button"
                        attr-type="submit">登录</n-button>
                </form>
            </template>
            <template v-else-if="webSession.mode === 'token'">
                <p>请输入服务部署时配置的访问令牌。</p>
                <form @submit.prevent="submit">
                    <n-input v-model:value="token" type="password" show-password-on="click"
                        placeholder="访问令牌" autocomplete="off" />
                    <n-button type="primary" :loading="webSession.checking" class="access-button"
                        attr-type="submit">连接</n-button>
                </form>
            </template>
            <template v-else>
                <p>正在连接下载服务…</p>
                <n-button v-if="webSession.error" :loading="webSession.checking" @click="authorizeWeb()">
                    重试
                </n-button>
            </template>
            <n-alert v-if="webSession.error" type="error" class="access-error">
                {{ webSession.error }}
            </n-alert>
        </n-card>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NAlert, NButton, NCard, NInput } from 'naive-ui'
import { authorizeWeb, authorizeWebWithPassword, webSession } from '../api/webClient'

const token = ref('')
const username = ref('')
const password = ref('')

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
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
}

.access-card {
    width: min(440px, 100%);
}

.access-card p {
    margin: 0 0 18px;
    line-height: 1.6;
}

.access-error {
    margin-top: 12px;
}

.password-input {
    margin-top: 12px;
}

.access-button {
    width: 100%;
    margin-top: 18px;
}
</style>

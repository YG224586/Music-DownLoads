<template>
    <div class="nav-layout" :class="{ 'is-compact': isCompact }">
        <!-- 中等及以上窗口：导航轨（目的地始终可见，不因宽度变化改变信息架构） -->
        <nav v-if="!isCompact" class="nav-rail" aria-label="主导航">
            <div class="rail-brand" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="24" height="24">
                    <path
                        d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6Z"
                        fill="currentColor"
                    />
                </svg>
            </div>

            <ul class="rail-destinations">
                <li v-for="item in destinations" :key="item.key">
                    <button
                        type="button"
                        class="destination"
                        :class="{ 'is-selected': item.key === currentRoute }"
                        :aria-current="
                            item.key === currentRoute ? 'page' : undefined
                        "
                        :aria-label="item.label"
                        @click="handleDestinationClick(item.key)"
                    >
                        <span class="destination-indicator">
                            <svg
                                viewBox="0 0 24 24"
                                width="24"
                                height="24"
                                aria-hidden="true"
                                v-html="item.icon"
                            />
                        </span>
                        <span class="destination-label">{{ item.label }}</span>
                    </button>
                </li>
            </ul>

            <!-- 连接状态在导航轨上只保留短标签，完整状态见 title 与屏幕阅读器文本 -->
            <p
                v-if="!native"
                class="rail-status"
                :class="taskStore.connectionStatus"
                :title="connectionHint"
            >
                <span class="connection-dot" aria-hidden="true"></span>
                <span>{{ shortConnectionLabel }}</span>
            </p>
        </nav>

        <div class="content-column">
            <!-- 顶部应用栏：屏幕标题 + 连接状态（缩略形式，不额外占用内容高度） -->
            <header class="top-app-bar">
                <h1 class="screen-title">{{ screenTitle }}</h1>

                <p
                    v-if="!native"
                    class="connection-chip"
                    :class="taskStore.connectionStatus"
                    :title="connectionHint"
                    role="status"
                >
                    <span class="connection-dot" aria-hidden="true"></span>
                    <span class="connection-label">{{ connectionLabel }}</span>
                    <span class="last-response"
                        >最近响应：{{ lastResponseLabel }}</span
                    >
                </p>
            </header>

            <!-- 内容区域 -->
            <main
                ref="mainContentRef"
                class="main-content"
                :class="{ 'has-bottom-nav': isCompact }"
            >
                <router-view v-slot="{ Component }">
                    <!-- 每个详情独立缓存，返回时恢复其分页、标签与勾选。 -->
                    <keep-alive>
                        <component :is="Component" :key="viewKey" />
                    </keep-alive>
                </router-view>
            </main>
        </div>

        <!-- 紧凑窗口：底部导航栏，正常文档流占位，回弹不会拉伸导航 -->
        <nav v-if="isCompact" class="bottom-nav" aria-label="主导航">
            <ul class="bottom-destinations">
                <li v-for="item in destinations" :key="item.key">
                    <button
                        type="button"
                        class="destination"
                        :class="{ 'is-selected': item.key === currentRoute }"
                        :aria-current="
                            item.key === currentRoute ? 'page' : undefined
                        "
                        @click="handleDestinationClick(item.key)"
                    >
                        <span class="destination-indicator">
                            <svg
                                viewBox="0 0 24 24"
                                width="24"
                                height="24"
                                aria-hidden="true"
                                v-html="item.icon"
                            />
                        </span>
                        <span class="destination-label">{{ item.label }}</span>
                    </button>
                </li>
            </ul>
        </nav>
    </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useNotification } from 'naive-ui'
import { useCloseGuard } from '../composables/useCloseGuard'
import { useCompactLayout } from '../composables/useCompactLayout'
import { isNativeRuntime } from '../api/runtimeApi'
import { useTaskStore } from '../stores/taskStore'

const router = useRouter()
const route = useRoute()
const native = isNativeRuntime()
const taskStore = useTaskStore()

type Destination = {
    key: string
    label: string
    /** 24dp M3 风格图标（描边路径，currentColor 跟随选中态） */
    icon: string
}

/** 顶层目的地：顺序与 key 保持稳定，跨断点只改变呈现形态。 */
const destinations: Destination[] = [
    {
        key: '/search',
        label: '搜索',
        icon: '<circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="m16.2 16.2 4.3 4.3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    },
    {
        key: '/playlist',
        label: '歌单',
        icon: '<path d="M4 6h16M4 12h16M4 18h9" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><circle cx="18" cy="18" r="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/>',
    },
    {
        key: '/task',
        label: '任务',
        icon: '<path d="M12 4v10" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><path d="m8.5 10.5 3.5 3.5 3.5-3.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M5 19h14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    },
    {
        key: '/settings',
        label: '设置',
        icon: '<path d="M4 8h10M18 8h2M4 16h4M12 16h8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><circle cx="16" cy="8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="10" cy="16" r="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/>',
    },
]

const connectionLabel = computed(
    () =>
        ({
            connecting: '正在连接服务',
            connected: '服务已连接',
            reconnecting: '连接中断，正在重连',
            disconnected: '服务连接已断开',
        })[taskStore.connectionStatus],
)

const shortConnectionLabel = computed(
    () =>
        ({
            connecting: '连接中',
            connected: '已连接',
            reconnecting: '重连中',
            disconnected: '已断开',
        })[taskStore.connectionStatus],
)

/** 状态不能只靠颜色表达，补充可读文本与提示。 */
const connectionHint = computed(
    () => `${connectionLabel.value} · 最近响应：${lastResponseLabel.value}`,
)

const lastResponseLabel = computed(() =>
    taskStore.lastServerActivityAt
        ? new Date(taskStore.lastServerActivityAt).toLocaleTimeString()
        : '尚未收到响应',
)

const viewKey = computed(() => {
    return ['/artist', '/album'].includes(route.path)
        ? route.fullPath
        : route.path
})

// 保存各路由页面的滚动位置，实现独立滚动记录
const mainContentRef = ref<HTMLElement | null>(null)
const scrollPositions: Record<string, number> = {}

let removeRouteGuard: (() => void) | null = null

// 恢复指定路由的滚动位置
async function restoreScrollPosition(path: string) {
    await nextTick()
    if (mainContentRef.value) {
        mainContentRef.value.scrollTop = scrollPositions[path] ?? 0
    }
}

// 监听路由变化，恢复新路由的滚动位置
watch(
    () => route.fullPath,
    (newPath) => {
        restoreScrollPosition(newPath)
    },
)

// 在 n-dialog-provider 内部调用，确保 useDialog 正常工作
useCloseGuard()

// 挂载通知实例到全局，供 store 使用
const notification = useNotification()
window.$notify = notification

// 紧凑窗口改用底部导航，更宽时改用导航轨；目的地与顺序保持不变。
const isCompact = useCompactLayout()

onMounted(() => {
    // 注册全局前置守卫，在离开当前路由前保存滚动位置
    removeRouteGuard = router.beforeEach((_to, from) => {
        if (mainContentRef.value) {
            scrollPositions[from.fullPath] = mainContentRef.value.scrollTop
        }
    })
    // 初始恢复当前路由的滚动位置（如果有保存过）
    restoreScrollPosition(route.fullPath)
})

onUnmounted(() => {
    // 移除路由守卫，避免内存泄漏
    if (removeRouteGuard) {
        removeRouteGuard()
        removeRouteGuard = null
    }
})

// 关于页属于设置入口，返回时继续保持设置导航高亮。
const currentRoute = computed(() => {
    if (route.path === '/album' || route.path === '/artist') return '/search'
    return route.path.startsWith('/settings/') ? '/settings' : route.path
})

/** 顶部应用栏标题取当前屏幕名，详情页保留自身标题。 */
const screenTitle = computed(() => {
    if (route.path === '/artist') return '歌手'
    if (route.path === '/album') return '专辑'
    if (route.path.startsWith('/settings/')) return '关于'
    return (
        destinations.find((item) => item.key === currentRoute.value)?.label ??
        'HotDownloader'
    )
})

function handleDestinationClick(key: string) {
    if (key !== route.path) {
        router.push(key)
    }
}
</script>

<style scoped>
/* 布局整体：窄窗口纵向（顶栏 + 内容 + 底栏），宽窗口横向（导航轨 + 内容） */
.nav-layout {
    --page-padding: var(--md-space-4);

    display: flex;
    height: 100%;
    min-height: 0;
    background-color: var(--md-surface);
}

.nav-layout.is-compact {
    flex-direction: column;

    --page-padding: var(--md-space-4);
}

.content-column {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
}

/* 顶部应用栏：小号 64dp 规格，标题用 title-large */
.top-app-bar {
    display: flex;
    align-items: center;
    gap: var(--md-space-3);
    flex-shrink: 0;
    min-height: var(--md-top-app-bar-height);
    padding: var(--md-space-2) var(--page-padding);
    padding-top: calc(var(--md-space-2) + var(--safe-area-top));
    padding-left: calc(var(--page-padding) + var(--safe-area-left));
    padding-right: calc(var(--page-padding) + var(--safe-area-right));
    background-color: var(--md-surface);
}

.screen-title {
    margin: 0;
    font-size: var(--md-title-large);
    line-height: var(--md-title-large-line);
    font-weight: var(--md-weight-medium);
    color: var(--md-on-surface);
}

/* 连接状态：与内容同层的信息条，不额外新增分区 */
.connection-chip {
    display: flex;
    align-items: center;
    gap: var(--md-space-2);
    margin: 0 0 0 auto;
    min-width: 0;
    padding: var(--md-space-1) var(--md-space-3);
    border-radius: var(--md-shape-full);
    background-color: var(--md-surface-container-low);
    font-size: var(--md-label-large);
    line-height: var(--md-label-large-line);
    color: var(--md-on-surface-variant);
}

.connection-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.connection-dot {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    border-radius: var(--md-shape-full);
    background-color: var(--md-warning);
}

.connection-chip.connected .connection-dot,
.rail-status.connected .connection-dot {
    background-color: var(--md-success);
}

.connection-chip.disconnected .connection-dot,
.rail-status.disconnected .connection-dot {
    background-color: var(--md-error);
}

/* 最近响应时间属于次要信息：紧凑窗口隐藏，避免标题与状态互相挤压 */
.last-response {
    white-space: nowrap;
    color: var(--md-on-surface-variant);
    display: none;
}

@media (min-width: 600px) {
    .last-response {
        display: inline;
    }
}

/* 主内容区：M3 surface，页面内容自行决定是否使用容器色 */
.main-content {
    flex: 1;
    /* 允许内容随窗口收缩，避免长列表撑开整个布局。 */
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    background-color: var(--md-surface);

    /* 用四个方向分别声明，让左右两侧都避开安全区 */
    padding: var(--page-padding);
    padding-left: calc(var(--page-padding) + var(--safe-area-left));
    padding-right: calc(var(--page-padding) + var(--safe-area-right));

    /* 将回弹限制在当前滚动容器内部，保留视觉回弹但阻断滚动链向上传播 */
    overscroll-behavior: contain;
}

/* 底部导航在正常流中占位，内容无需预留额外底部间距 */
.main-content.has-bottom-nav {
    padding-bottom: var(--md-space-4);
}

/* 导航轨：80dp 宽，项宽 56dp，选中态为 secondaryContainer 胶囊 */
.nav-rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--md-space-3);
    flex-shrink: 0;
    width: var(--md-rail-width);
    padding: var(--md-space-3) var(--md-space-2);
    padding-top: calc(var(--md-space-3) + var(--safe-area-top));
    padding-left: calc(var(--md-space-2) + var(--safe-area-left));
    background-color: var(--md-surface);
    overflow-y: auto;
}

.rail-brand {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border-radius: var(--md-shape-md);
    background-color: var(--md-primary-container);
    color: var(--md-on-primary-container);
}

.rail-destinations,
.bottom-destinations {
    display: flex;
    margin: 0;
    padding: 0;
    list-style: none;
}

.rail-destinations {
    flex-direction: column;
    gap: var(--md-space-1);
    width: 100%;
}

.rail-destinations li {
    width: 100%;
}

/* 目的地：图标 + 标签，命中区域不小于 48dp */
.destination {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    width: 100%;
    min-height: var(--md-target-min);
    padding: var(--md-space-1) 0;
    border: none;
    background: transparent;
    color: var(--md-on-surface-variant);
    font-family: inherit;
    font-size: var(--md-label-medium);
    line-height: var(--md-label-medium-line);
    font-weight: var(--md-weight-medium);
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
}

.destination-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 32px;
    border-radius: var(--md-shape-full);
    transition: background-color var(--md-duration-short)
        var(--md-easing-standard);
}

/* 状态层用现有容器色表达，避免依赖 color-mix 的浏览器支持 */
.destination:hover .destination-indicator {
    background-color: var(--md-surface-container-highest);
}

.destination.is-selected {
    color: var(--md-on-surface);
}

.destination.is-selected .destination-indicator {
    background-color: var(--md-secondary-container);
    color: var(--md-on-secondary-container);
}

.destination-label {
    white-space: nowrap;
}

/* 导航轨底部状态：仅短标签，完整信息由 title 提供 */
.rail-status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--md-space-1);
    margin: auto 0 0;
    padding: var(--md-space-2) 0;
    font-size: var(--md-label-small);
    line-height: var(--md-label-small-line);
    color: var(--md-on-surface-variant);
    text-align: center;
}

/* 底部导航栏：80dp，四个目的地平分宽度 */
.bottom-nav {
    width: 100%;
    flex-shrink: 0;
    min-height: var(--md-bottom-nav-height);
    padding-bottom: var(--safe-area-bottom);
    padding-left: var(--safe-area-left);
    padding-right: var(--safe-area-right);
    background-color: var(--md-surface-container);
}

.bottom-destinations {
    width: 100%;
    height: 100%;
    align-items: stretch;
}

.bottom-destinations li {
    flex: 1;
    min-width: 0;
}

.bottom-destinations .destination {
    height: 100%;
    padding-top: var(--md-space-3);
    padding-bottom: var(--md-space-3);
}

/* 键盘与辅助技术同样需要可见的焦点指示 */
.destination:focus-visible {
    outline: 3px solid var(--md-primary);
    outline-offset: -3px;
    border-radius: var(--md-shape-lg);
}

@media (prefers-reduced-motion: reduce) {
    .destination-indicator {
        transition: none;
    }
}
</style>

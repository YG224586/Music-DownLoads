import { onScopeDispose, readonly, ref } from 'vue'

/**
 * M3 宽度分级中的 compact（< 600dp）。
 * 仅表达“当前窗口放不下持久侧边导航”，因此用媒体查询而不是设备判断：
 * 手机横屏、折叠屏展开、桌面窗口缩放都会自然切换形态。
 */
export const COMPACT_LAYOUT_QUERY = '(max-width: 599px)'

/** 紧凑窗口：底部导航 + 顶部应用栏；更宽时改为导航轨。 */
export function useCompactLayout() {
    const mediaQuery =
        typeof window === 'undefined'
            ? null
            : window.matchMedia(COMPACT_LAYOUT_QUERY)
    const isCompact = ref(mediaQuery?.matches ?? false)

    function update(event: MediaQueryListEvent) {
        isCompact.value = event.matches
    }

    mediaQuery?.addEventListener('change', update)

    // 跟随组件作用域释放监听，避免反复挂载时累积事件处理函数。
    onScopeDispose(() => {
        mediaQuery?.removeEventListener('change', update)
    })

    return readonly(isCompact)
}

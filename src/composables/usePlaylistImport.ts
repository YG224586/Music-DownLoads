import { ref, computed } from 'vue'
import type { PlaylistInfo, SongInfo } from '../types'
import * as musicApi from '../api/musicApi'

/**
 * 歌单导入逻辑封装。
 * 管理歌单信息、歌曲列表和选择状态，并提供加载和重置方法。
 */
export function usePlaylistImport() {
    // 是否正在导入
    const loading = ref(false)
    // 错误信息
    const errorMsg = ref('')
    // 歌单基本信息
    const playlist = ref<PlaylistInfo | null>(null)
    // 歌曲列表
    const songs = ref<SongInfo[]>([])
    // 选中歌曲的 mid 集合
    const selectedIds = ref<string[]>([])
    // 路由切换时，旧请求仍可能返回；序号保证只有最新请求能写入页面。
    let requestId = 0

    // 是否全部选中
    const isAllSelected = computed(
        () =>
            songs.value.length > 0 &&
            selectedIds.value.length === songs.value.length,
    )
    // 是否部分选中
    const isIndeterminate = computed(
        () =>
            selectedIds.value.length > 0 &&
            selectedIds.value.length < songs.value.length,
    )

    /**
     * 切换全选状态。
     * @param checked 是否全选
     */
    function toggleAll(checked: boolean) {
        selectedIds.value = checked ? songs.value.map((s) => s.mid) : []
    }

    /**
     * 切换单首歌曲的选中状态。
     * @param songMid 歌曲唯一标识
     * @param selected 是否选中
     */
    function toggleSelect(songMid: string, selected: boolean) {
        if (selected) {
            if (!selectedIds.value.includes(songMid)) {
                selectedIds.value.push(songMid)
            }
        } else {
            selectedIds.value = selectedIds.value.filter((id) => id !== songMid)
        }
    }

    async function loadPlaylist(
        request: () => Promise<{ playlist: PlaylistInfo; songs: SongInfo[] }>,
    ) {
        const currentRequest = ++requestId
        loading.value = true
        errorMsg.value = ''
        playlist.value = null
        songs.value = []
        selectedIds.value = []

        try {
            const res = await request()
            if (currentRequest !== requestId) return

            playlist.value = res.playlist
            songs.value = res.songs
        } catch (e: unknown) {
            if (currentRequest !== requestId) return

            errorMsg.value = e instanceof Error ? e.message : String(e)
        } finally {
            if (currentRequest === requestId) {
                loading.value = false
            }
        }
    }

    /** 按链接或 ID 导入歌单。 */
    async function importPlaylist(platform: string, term: string) {
        const value = term.trim()
        if (!value) return
        await loadPlaylist(() => musicApi.fetchPlaylistSongs(platform, value))
    }

    async function loadCreatedPlaylist(id: string, dirid: string) {
        await loadPlaylist(() => musicApi.fetchCreatedPlaylistSongs(id, dirid))
    }

    /**
     * 重置所有状态（清空页面）。
     */
    function reset() {
        requestId++
        loading.value = false
        errorMsg.value = ''
        playlist.value = null
        songs.value = []
        selectedIds.value = []
    }

    return {
        loading,
        errorMsg,
        playlist,
        songs,
        selectedIds,
        isAllSelected,
        isIndeterminate,
        toggleAll,
        toggleSelect,
        importPlaylist,
        loadCreatedPlaylist,
        reset,
    }
}

// 平台配置列表，用于前端平台切换与 API 调用。
// key 必须与 Rust 侧 `Platform` 的 Display/serde 输出逐字一致（协议层唯一标识）。
export interface PlatformCapabilities {
    /** 歌单：歌单导入、歌单详情与「我的歌单」。 */
    playlist: boolean
    /** 专辑：专辑搜索与专辑详情。 */
    album: boolean
    /** 歌手：歌手搜索与歌手详情。 */
    artist: boolean
    /** 歌词：任务完成后写入歌词。 */
    lyric: boolean
    /** 搜索建议：搜索框下拉与热词。 */
    suggest: boolean
}

export interface PlatformOption {
    /** 平台标识，与 Rust `Platform` 一致。 */
    key: string
    /** 选择器里的短标签（窄屏空间有限，只能用 2–3 个字符）。 */
    label: string
    /** 完整名称，用于提示与说明文案。 */
    name: string
    /** 该平台实际支持的能力；未列出的能力一律按不支持处理并降级隐藏。 */
    capabilities: PlatformCapabilities
}

/** 用能力名列表构造能力表，避免把 `false` 写成 `true` 的手滑。 */
function caps(
    ...enabled: (keyof PlatformCapabilities)[]
): PlatformCapabilities {
    return {
        playlist: enabled.includes('playlist'),
        album: enabled.includes('album'),
        artist: enabled.includes('artist'),
        lyric: enabled.includes('lyric'),
        suggest: enabled.includes('suggest'),
    }
}

/** 完整能力：QQ 音乐与酷我音乐走自有接口，五项全支持。 */
const FULL_CAPS = caps('playlist', 'album', 'artist', 'lyric', 'suggest')
/** 仅歌曲搜索与取链：新增的四个内置平台与脚本音源都是这一形态。 */
export const SONG_SEARCH_CAPS: PlatformCapabilities = caps()

export const PLATFORMS: PlatformOption[] = [
    { key: 'qqmusic', label: 'QQ', name: 'QQ 音乐', capabilities: FULL_CAPS },
    { key: 'kuwo', label: '酷我', name: '酷我音乐', capabilities: FULL_CAPS },
    {
        key: 'kugou',
        label: '酷狗',
        name: '酷狗音乐',
        capabilities: SONG_SEARCH_CAPS,
    },
    {
        key: 'netease',
        label: '网易',
        name: '网易云音乐',
        capabilities: SONG_SEARCH_CAPS,
    },
    {
        key: 'bilibili',
        label: 'B站',
        name: '哔哩哔哩',
        capabilities: SONG_SEARCH_CAPS,
    },
    {
        key: 'migu',
        label: '咪咕',
        name: '咪咕音乐',
        capabilities: SONG_SEARCH_CAPS,
    },
]

// 默认平台 key
export const DEFAULT_PLATFORM = PLATFORMS[0].key

/** 按平台标识查找配置；未知标识（含 `script:<id>`）返回 undefined。 */
export function findPlatform(key: string): PlatformOption | undefined {
    return PLATFORMS.find((item) => item.key === key)
}

/** 平台完整名称；未知标识原样返回，避免界面出现空白。 */
export function platformName(key: string): string {
    return findPlatform(key)?.name ?? key
}

/**
 * 平台是否支持某项能力。
 *
 * 未知标识（例如脚本音源）一律返回 false：脚本音源当前只支持歌曲搜索，
 * 其余入口必须隐藏而不是点了才报错。
 */
export function supportsCapability(
    key: string,
    capability: keyof PlatformCapabilities,
): boolean {
    return findPlatform(key)?.capabilities[capability] ?? false
}

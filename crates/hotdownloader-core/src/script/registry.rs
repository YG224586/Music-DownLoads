//! 进程内脚本音源注册表。
//!
//! 注册表是脚本音源的唯一运行时真相：命令层做持久化，下载链路按 id 取脚本。
//! 锁都只在极短的临界区内持有，不会跨 await。

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use super::limits::{MAX_SONG_CLUES_TOTAL, MAX_SONG_CLUE_BYTES};
use super::model::{ScriptSong, ScriptSource, ScriptSourceState};

static SOURCES: OnceLock<Mutex<HashMap<u32, Arc<ScriptSource>>>> = OnceLock::new();
static SONG_CLUES: OnceLock<Mutex<ClueStore>> = OnceLock::new();
/// 下一个可用 id；只增不减，删除后的 id 不再复用（契约 §6）。
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// 取锁时容忍中毒：注册表只存纯数据，中毒不会让它进入不一致状态。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn sources() -> &'static Mutex<HashMap<u32, Arc<ScriptSource>>> {
    SOURCES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 歌曲线索：`search` 时记住脚本返回的原始对象，供 `getUrl` 原样回传。
#[derive(Default)]
struct ClueStore {
    entries: HashMap<(u32, String), ScriptSong>,
    order: VecDeque<(u32, String)>,
}

fn clues() -> &'static Mutex<ClueStore> {
    SONG_CLUES.get_or_init(|| Mutex::new(ClueStore::default()))
}

/// 分配一个新的音源 id（单调递增，不复用）。
pub fn allocate_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

/// 提升 id 水位，避免持久化恢复的记录与后续分配冲突。
fn bump_id(id: u32) {
    let next = id.saturating_add(1);
    let mut current = NEXT_ID.load(Ordering::SeqCst);
    while current < next {
        match NEXT_ID.compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

/// 按 id 升序列出全部音源。
pub fn list() -> Vec<Arc<ScriptSource>> {
    let mut items: Vec<Arc<ScriptSource>> = lock(sources()).values().cloned().collect();
    items.sort_by_key(|source| source.id);
    items
}

/// 前端条目快照。
pub fn snapshot() -> ScriptSourceState {
    ScriptSourceState {
        sources: list().iter().map(|source| source.item()).collect(),
    }
}

pub fn get(id: u32) -> Option<Arc<ScriptSource>> {
    lock(sources()).get(&id).cloned()
}

/// 按名称查找（安装时用于判断是新增还是更新）。
pub fn find_by_name(name: &str) -> Option<Arc<ScriptSource>> {
    lock(sources())
        .values()
        .find(|source| source.name == name)
        .cloned()
}

/// 写入（新增或覆盖同 id 记录）。
pub fn insert(mut record: ScriptSource) -> Arc<ScriptSource> {
    if record.id == 0 {
        record.id = allocate_id();
    }
    bump_id(record.id);
    let shared = Arc::new(record);
    lock(sources()).insert(shared.id, shared.clone());
    shared
}

/// 删除；同时清理该音源缓存的歌曲线索。
pub fn remove(id: u32) -> Option<Arc<ScriptSource>> {
    let removed = lock(sources()).remove(&id);
    if removed.is_some() {
        let mut store = lock(clues());
        store.entries.retain(|(source_id, _), _| *source_id != id);
        store.order.retain(|(source_id, _)| *source_id != id);
    }
    removed
}

/// 切换启用状态；返回更新后的记录。
pub fn set_enabled(id: u32, enabled: bool) -> Option<Arc<ScriptSource>> {
    let mut guard = lock(sources());
    let existing = guard.get(&id).cloned()?;
    let mut record = (*existing).clone();
    record.enabled = enabled;
    let shared = Arc::new(record);
    guard.insert(id, shared.clone());
    Some(shared)
}

/// 启动时用持久化记录替换内存表，返回恢复条数。
pub fn replace_all(records: Vec<ScriptSource>) -> usize {
    let mut guard = lock(sources());
    guard.clear();
    let mut count = 0;
    for mut record in records {
        if record.id == 0 {
            record.id = allocate_id();
        }
        bump_id(record.id);
        guard.insert(record.id, Arc::new(record));
        count += 1;
    }
    drop(guard);
    // 恢复的记录可能引用了已不存在的音源，清一次线索缓存。
    let mut store = lock(clues());
    store.entries.clear();
    store.order.clear();
    count
}

/// 记住指定音源返回的歌曲对象。
pub fn remember_source_song(source_id: u32, song: &ScriptSong) {
    // 音源脚本是不可信输入：raw 是脚本自己拼的 JSON 文本，可能极大。
    // 超过上限时只去掉 raw、保留归一化字段（`getUrl` 仍能用归一化字段重建入参）。
    let mut song = song.clone();
    if song
        .raw
        .as_deref()
        .is_some_and(|raw| raw.len() > MAX_SONG_CLUE_BYTES)
    {
        song.raw = None;
    }
    let mut store = lock(clues());
    let key = (source_id, song.id.clone());
    if store.entries.contains_key(&key) {
        store.entries.insert(key.clone(), song);
        // 已存在：刷新 LRU 位置。
        store.order.retain(|item| item != &key);
        store.order.push_back(key);
        return;
    }

    store.entries.insert(key.clone(), song);
    store.order.push_back(key);
    while store.order.len() > MAX_SONG_CLUES_TOTAL {
        if let Some(evicted) = store.order.pop_front() {
            store.entries.remove(&evicted);
        }
    }
}

/// 取回 `search` 时缓存的歌曲对象。
pub fn recall_song(source_id: u32, song_id: &str) -> Option<ScriptSong> {
    let key = (source_id, song_id.to_string());
    lock(clues()).entries.get(&key).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::model::ScriptSong;

    fn sample_source(id: u32, name: &str) -> ScriptSource {
        ScriptSource {
            id,
            name: name.to_string(),
            description: None,
            qualities: vec!["128kmp3".into()],
            script: "var source = {};".into(),
            enabled: true,
            installed_at: "2026-01-01T00:00:00Z".into(),
            source_url: None,
        }
    }

    fn sample_song(id: &str) -> ScriptSong {
        ScriptSong {
            id: id.to_string(),
            title: "标题".into(),
            artist: "歌手".into(),
            qualities: vec!["128kmp3".into()],
            raw: Some(format!("{{\"id\":\"{id}\"}}")),
            ..ScriptSong::default()
        }
    }

    #[test]
    fn ids_are_monotonic_and_not_reused() {
        let first = allocate_id();
        let second = allocate_id();
        assert!(second > first);
        let record = insert(sample_source(0, "临时音源"));
        assert!(record.id >= second);
        let removed = remove(record.id).expect("应删除刚插入的音源");
        assert_eq!(removed.id, record.id);
        let next = allocate_id();
        assert!(next > record.id);
    }

    #[test]
    fn enable_switch_keeps_script_body() {
        let record = insert(sample_source(0, "开关音源"));
        let updated = set_enabled(record.id, false).expect("音源应存在");
        assert!(!updated.enabled);
        assert_eq!(updated.script, record.script);
    }

    #[test]
    fn replace_all_restores_records_and_clears_clues() {
        let record = insert(sample_source(0, "恢复音源"));
        remember_source_song(record.id, &sample_song("s1"));
        assert!(recall_song(record.id, "s1").is_some());

        let restored = replace_all(vec![sample_source(9001, "恢复音源")]);
        assert_eq!(restored, 1);
        assert!(get(9001).is_some());
        assert!(recall_song(9001, "s1").is_none());
        // 恢复的记录 id 会抬高分配水位，避免后续冲突。
        assert!(allocate_id() > 9001);
    }

    #[test]
    fn clue_cache_evicts_oldest_per_source() {
        let source_id = 4242;
        remember_source_song(source_id, &sample_song("first"));
        for index in 0..MAX_SONG_CLUES_TOTAL {
            remember_source_song(source_id, &sample_song(&format!("song-{index}")));
        }
        assert!(recall_song(source_id, "first").is_none());
        assert!(recall_song(source_id, &format!("song-{}", MAX_SONG_CLUES_TOTAL - 1)).is_some());
    }

    #[test]
    fn oversized_raw_is_dropped_but_normalized_fields_survive() {
        let source_id = 777;
        let mut song = sample_song("big");
        song.raw = Some("x".repeat(MAX_SONG_CLUE_BYTES + 1));
        remember_source_song(source_id, &song);
        let recalled = recall_song(source_id, "big").expect("线索应仍被记住");
        assert!(recalled.raw.is_none(), "超大 raw 必须被丢弃");
        assert_eq!(recalled.title, song.title);
        assert_eq!(recalled.artist, song.artist);
    }
}

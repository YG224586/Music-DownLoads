//! `musics.fcg` 接口要求的 `zzc` 签名参数。
//!
//! QQ 音乐桌面端在请求里带上 `?sign=<zzc...>`。实测（2026-10-02）：
//! 未签名的 `musicu.fcg` 与 `musics.fcg` 分别返回 `code=2001`（限流）与
//! `code=2000`（要求签名），而携带签名的 `musics.fcg` 在同一下行时刻仍能正常返回结果。
//!
//! 算法（与 lx-music-desktop 的实现一致）：
//! 1. 取请求体 JSON 字符串的 SHA-1 十六进制摘要（40 个小写字符）；
//! 2. 按固定下标挑出两组字符拼成前后缀；
//! 3. 把摘要每两个字符当成一个字节，与固定常量表逐字节异或，得到 20 字节；
//! 4. 这 20 字节做 Base64，去掉 `\ / + =` 四种字符；
//! 5. `zzc` + 前缀 + Base64 + 后缀，整体转小写。
//!
//! 说明：下标表里出现的 `40` 超出 40 字符摘要的范围，参照实现里它取到 `undefined`、
//! 拼接时被丢弃，所以这里也用 `get()` 直接跳过，保持字节级一致。

/// 前缀取字符的下标表。
const PART_1_INDEXES: [usize; 8] = [23, 14, 6, 36, 16, 40, 7, 19];
/// 后缀取字符的下标表。
const PART_2_INDEXES: [usize; 8] = [16, 1, 32, 12, 19, 27, 8, 5];
/// 逐字节异或用的常量表。
const SCRAMBLE_VALUES: [u8; 20] = [
    89, 39, 179, 150, 218, 82, 58, 252, 177, 52, 186, 123, 120, 64, 242, 133, 143, 161, 121, 179,
];

/// 计算请求体的 `zzc` 签名，返回值直接拼在 `?sign=` 后面。
pub fn zzc_sign(text: &str) -> String {
    let hash = sha1_hex(text.as_bytes());

    let pick = |indexes: &[usize]| -> String {
        indexes
            .iter()
            .filter_map(|index| hash.as_bytes().get(*index))
            .map(|byte| *byte as char)
            .collect()
    };

    let mut scrambled = [0u8; 20];
    for (position, slot) in scrambled.iter_mut().enumerate() {
        let pair = &hash[position * 2..position * 2 + 2];
        let byte = u8::from_str_radix(pair, 16).unwrap_or(0);
        *slot = SCRAMBLE_VALUES[position] ^ byte;
    }

    let encoded: String = base64_encode(&scrambled)
        .chars()
        .filter(|c| !matches!(c, '\\' | '/' | '+' | '='))
        .collect();

    format!(
        "zzc{}{}{}",
        pick(&PART_1_INDEXES),
        encoded,
        pick(&PART_2_INDEXES)
    )
    .to_lowercase()
}

/// SHA-1 十六进制摘要（小写，40 字符）。
fn sha1_hex(data: &[u8]) -> String {
    let mut state: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];

    let mut message = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut words = [0u32; 80];
        for (index, word) in chunk.chunks(4).enumerate() {
            words[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in words.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
    }

    state.iter().map(|word| format!("{:08x}", word)).collect()
}

/// 标准 Base64 编码（带 `=` 补齐）。
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut encoded = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        encoded.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        encoded.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

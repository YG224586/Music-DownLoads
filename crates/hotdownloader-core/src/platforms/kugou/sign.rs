//! 酷狗接口签名辅助。
//!
//! `trackercdn.kugou.com/i/v2/` 取链接口要求 `key = md5(小写 hash + "kgcloudv2")`（实测）。
//! `hotdownloader-core` 没有可用的 md5 依赖：`app/libs/um_crypto/um_crypto/utils` 里虽有一份
//! `md5` crate 实现，但它属于另一个 crate（`umc_qmc` 未 re-export），平台契约又禁止新增 crate
//! 依赖，因此这里内置一份自包含 MD5（RFC 1321）。

/// `trackercdn` 签名盐值。
pub const TRACKER_SALT: &str = "kgcloudv2";

/// 计算 `trackercdn.kugou.com/i/v2/` 的 `key` 参数：`md5(小写 hash + "kgcloudv2")`。
///
/// 传入大写 hash 也会得到同一结果（内部统一转小写），与酷狗客户端行为一致。
pub fn tracker_key(hash: &str) -> String {
    let mut input = hash.to_ascii_lowercase();
    input.push_str(TRACKER_SALT);
    md5_hex(input.as_bytes())
}

/// 自包含 MD5，返回 32 位小写十六进制摘要。
pub fn md5_hex(data: &[u8]) -> String {
    let mut state: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];

    // 填充：0x80 + 若干 0，使长度 ≡ 56 (mod 64)，再附加 64 位小端比特长度。
    let mut message = Vec::with_capacity(data.len() + 72);
    message.extend_from_slice(data);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    let bit_len = (data.len() as u64).wrapping_mul(8);
    message.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in message.chunks_exact(64) {
        let mut words = [0u32; 16];
        for (index, word) in words.iter_mut().enumerate() {
            let start = index * 4;
            *word = u32::from_le_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }

        let [mut a, mut b, mut c, mut d] = state;
        for index in 0..64 {
            let (mix, word_index) = match index / 16 {
                0 => ((b & c) | (!b & d), index),
                1 => ((d & b) | (!d & c), (5 * index + 1) % 16),
                2 => (b ^ c ^ d, (3 * index + 5) % 16),
                _ => (c ^ (b | !d), (7 * index) % 16),
            };
            let rotated = a
                .wrapping_add(mix)
                .wrapping_add(K[index])
                .wrapping_add(words[word_index])
                .rotate_left(SHIFT[index]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(rotated);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }

    let mut digest = String::with_capacity(32);
    for word in state {
        for byte in word.to_le_bytes() {
            digest.push_str(&format!("{byte:02x}"));
        }
    }
    digest
}

/// 每轮左旋位数（RFC 1321 §3.4）。
const SHIFT: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// `K[i] = floor(2^32 × abs(sin(i + 1)))`（RFC 1321 §3.4）。
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

#[cfg(test)]
mod tests {
    use super::{md5_hex, tracker_key, TRACKER_SALT};

    /// RFC 1321 §A.5 附录测试向量。
    #[test]
    fn md5_matches_rfc1321_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5_hex(b"message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
        assert_eq!(
            md5_hex(b"abcdefghijklmnopqrstuvwxyz"),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
        assert_eq!(
            md5_hex(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            ),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    /// 分组边界：55/56/57/63/64/65 字节与多分组、UTF-8 多字节。
    #[test]
    fn md5_handles_block_boundaries() {
        assert_eq!(md5_hex(&[b'a'; 55]), "ef1772b6dff9a122358552954ad0df65");
        assert_eq!(md5_hex(&[b'a'; 56]), "3b0c8ac703f828b04c6c197006d17218");
        assert_eq!(md5_hex(&[b'a'; 57]), "652b906d60af96844ebd21b674f35e93");
        assert_eq!(md5_hex(&[b'a'; 63]), "b06521f39153d618550606be297466d5");
        assert_eq!(md5_hex(&[b'a'; 64]), "014842d480b571495a4a0363793f7367");
        assert_eq!(md5_hex(&[b'a'; 65]), "c743a45e0d2e6a95cb859adae0248435");
        assert_eq!(md5_hex(&[b'b'; 120]), "c8eb298682a7890e3136465d34d5ec25");
        assert_eq!(
            md5_hex("晴天".as_bytes()),
            "cbbe546304037478ce0c36437d036711"
        );
    }

    /// 实测 fixture：这两个 key 在探测中真的换回了 320k / 无损直链。
    #[test]
    fn tracker_key_matches_probed_values() {
        assert_eq!(
            tracker_key("28f83bbd8cab043895039e98366ad2b4"),
            "0d8fc4aa46dc995ef3e43cb260b81145"
        );
        assert_eq!(
            tracker_key("857879479e698d729640a5716dc3e56f"),
            "ced4966efac362219f9d884dfe8303ec"
        );
        assert_eq!(
            tracker_key("48c685f679ffc7cf08b8a8341ca9db44"),
            "5d62e4bf5de6256b12b771d2558caa28"
        );
    }

    /// hash 大小写不影响签名结果。
    #[test]
    fn tracker_key_normalizes_uppercase_hashes() {
        assert_eq!(
            tracker_key("28F83BBD8CAB043895039E98366AD2B4"),
            tracker_key("28f83bbd8cab043895039e98366ad2b4")
        );
        assert_eq!(TRACKER_SALT, "kgcloudv2");
    }
}

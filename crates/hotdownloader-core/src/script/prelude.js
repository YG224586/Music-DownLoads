// 音源脚本宿主前导（契约见 _dev/script-spec/API.md §2）。
//
// Rust 侧会在「用户脚本之前」和「用户脚本之后」各执行一次本文件：
//   第一次：定义 http / log / base64 / sleep 等宿主能力与 __hd_* 桥接函数；
//   第二次：恢复被用户脚本遮蔽（同名全局）的桥接函数。
//
// 所有对外暴露的全局一律通过 globalThis 属性赋值，且整份前导包在一个 IIFE 内：
// 这样用户脚本里写 `const http = ...` / `var log = ...` 也不会触发
// "Identifier 'http' has already been declared"，私有 helper 也不会污染全局。
(function () {
    'use strict';

    var root = globalThis;

    // 契约 §3 的音质取值 → 落盘扩展名（必须与 Rust 侧 script::quality_extension 一致）。
    var QUALITY_EXTENSIONS = {
        '48kaac': 'm4a',
        '96kogg': 'ogg',
        '128kmp3': 'mp3',
        '192kaac': 'm4a',
        '192kogg': 'ogg',
        '320kmp3': 'mp3',
        'flac': 'flac',
        'hires': 'flac',
        '臻品母带': 'flac'
    };

    var DEFAULT_QUALITIES = ['128kmp3'];
    var PENDING_RESULT = '__hd_pending_result';

    function toText(value) {
        if (value === null || value === undefined) {
            return '';
        }
        if (typeof value === 'string') {
            return value;
        }
        if (typeof value === 'object') {
            try {
                return JSON.stringify(value);
            } catch (error) {
                return String(value);
            }
        }
        try {
            return String(value);
        } catch (error) {
            return '';
        }
    }

    // 宿主原生函数由 Rust 注册为全局属性。
    function callHost(name, args) {
        var fn = root[name];
        if (typeof fn !== 'function') {
            throw new Error('宿主函数缺失: ' + name);
        }
        return fn.apply(null, args);
    }

    function describeError(error) {
        if (error && typeof error.message === 'string' && error.message.length > 0) {
            return error.message;
        }
        return toText(error) || '脚本执行失败';
    }

    function envelopeOk(value) {
        return JSON.stringify({ ok: true, value: value });
    }

    function envelopeError(error) {
        return JSON.stringify({ ok: false, error: describeError(error) });
    }

    function envelopePending() {
        return JSON.stringify({ ok: true, pending: true });
    }

    function isThenable(value) {
        return !!value && (typeof value === 'object' || typeof value === 'function') && typeof value.then === 'function';
    }

    // 异步结果写进全局，由 Rust 侧 run_jobs() 清空 Promise 队列后读取。
    function beginPending(promise, map) {
        root[PENDING_RESULT] = null;
        Promise.resolve(promise).then(
            function (value) {
                // map 内部（字段归一化、URL 校验）抛错也必须落进信封：
                // 否则 Promise 会被判为 rejected，全局结果保持 null，Rust 侧只能报“超时”，丢失真实原因。
                try {
                    root[PENDING_RESULT] = map(value);
                } catch (error) {
                    root[PENDING_RESULT] = envelopeError(error);
                }
            },
            function (error) {
                root[PENDING_RESULT] = envelopeError(error);
            }
        );
        return envelopePending();
    }

    // 契约允许 source 用 var / const / 最后一个表达式声明；const 只存在于全局词法环境，
    // 不会成为 globalThis 属性，因此这里必须用裸标识符 + typeof 探测（TDZ 会抛错，故包 try）。
    function readSource() {
        try {
            if (typeof source !== 'undefined' && source) {
                return source;
            }
        } catch (error) {
            return null;
        }
        return null;
    }

    function normalizeQualities(list, fallback) {
        var out = [];
        var index;
        var quality;
        if (Array.isArray(list)) {
            for (index = 0; index < list.length; index++) {
                quality = toText(list[index]).trim();
                if (quality.length > 0 && QUALITY_EXTENSIONS[quality] && out.indexOf(quality) < 0) {
                    out.push(quality);
                }
            }
        }
        if (out.length === 0 && Array.isArray(fallback)) {
            for (index = 0; index < fallback.length; index++) {
                quality = toText(fallback[index]).trim();
                if (quality.length > 0 && QUALITY_EXTENSIONS[quality] && out.indexOf(quality) < 0) {
                    out.push(quality);
                }
            }
        }
        if (out.length === 0) {
            out = DEFAULT_QUALITIES.slice();
        }
        return out;
    }

    function normalizeDuration(value) {
        if (value === null || value === undefined) {
            return null;
        }
        var number = Number(value);
        if (!isFinite(number) || number <= 0) {
            return null;
        }
        return Math.round(number);
    }

    function normalizeSong(song, fallbackQualities, position) {
        if (!song || typeof song !== 'object') {
            throw new Error('第 ' + (position + 1) + ' 首歌曲不是对象');
        }
        var id = toText(song.id).trim();
        if (id.length === 0) {
            throw new Error('第 ' + (position + 1) + ' 首歌曲缺少 id');
        }
        var title = toText(song.title).trim();
        if (title.length === 0) {
            throw new Error('第 ' + (position + 1) + ' 首歌曲缺少 title');
        }
        var artist = toText(song.artist).trim();
        if (artist.length === 0) {
            throw new Error('第 ' + (position + 1) + ' 首歌曲缺少 artist');
        }
        var normalized = {
            id: id,
            title: title,
            artist: artist,
            album: song.album === null || song.album === undefined ? null : toText(song.album),
            duration: normalizeDuration(song.duration),
            cover: song.cover === null || song.cover === undefined ? null : toText(song.cover),
            qualities: normalizeQualities(song.qualities, fallbackQualities)
        };
        try {
            normalized.raw = JSON.stringify(song);
        } catch (error) {
            normalized.raw = null;
        }
        return normalized;
    }

    function normalizeSearchResult(result, source) {
        var list = null;
        if (Array.isArray(result)) {
            list = result;
        } else if (result && typeof result === 'object' && Array.isArray(result.songs)) {
            list = result.songs;
        }
        if (!list) {
            throw new Error('search 必须返回歌曲数组');
        }
        var fallback = source && typeof source === 'object' ? source.qualities : null;
        var songs = [];
        for (var index = 0; index < list.length; index++) {
            songs.push(normalizeSong(list[index], fallback, index));
        }
        return songs;
    }

    function normalizeHeaders(headers) {
        if (!headers || typeof headers !== 'object') {
            return null;
        }
        var out = {};
        var keys = Object.keys(headers);
        for (var index = 0; index < keys.length; index++) {
            out[keys[index]] = toText(headers[keys[index]]);
        }
        return out;
    }

    function normalizeUrlResult(result, quality) {
        var url = '';
        var resolved = quality;
        var headers = null;
        if (typeof result === 'string') {
            url = result;
        } else if (result && typeof result === 'object') {
            url = toText(result.url);
            var declared = toText(result.quality).trim();
            if (declared.length > 0) {
                resolved = declared;
            }
            headers = normalizeHeaders(result.headers);
        }
        url = toText(url).trim();
        if (!/^https?:\/\//i.test(url)) {
            throw new Error('getUrl 未返回有效的 http(s) 直链');
        }
        return { url: url, quality: resolved, headers: headers };
    }

    // ---- 桥接函数：跨边界一律走 JSON 信封串 ----

    root.__hd_describe = function () {
        try {
            var source = readSource();
            var hasSource = !!source && typeof source === 'object';
            return envelopeOk({
                name: hasSource ? toText(source.name).trim() : '',
                description: hasSource && source.description !== null && source.description !== undefined
                    ? toText(source.description)
                    : null,
                qualities: hasSource ? normalizeQualities(source.qualities, null) : DEFAULT_QUALITIES.slice(),
                hasSource: hasSource,
                hasSearch: hasSource && typeof source.search === 'function',
                hasGetUrl: hasSource && typeof source.getUrl === 'function'
            });
        } catch (error) {
            return envelopeError(error);
        }
    };

    root.__hd_search = function (keyword, page, limit) {
        try {
            var source = readSource();
            if (!source || typeof source.search !== 'function') {
                throw new Error('脚本缺少 search 方法');
            }
            var result = source.search(toText(keyword), Number(page) || 1, Number(limit) || 20);
            if (isThenable(result)) {
                return beginPending(result, function (value) {
                    return envelopeOk(normalizeSearchResult(value, source));
                });
            }
            return envelopeOk(normalizeSearchResult(result, source));
        } catch (error) {
            return envelopeError(error);
        }
    };

    root.__hd_get_url = function (songJson, quality) {
        try {
            var source = readSource();
            if (!source || typeof source.getUrl !== 'function') {
                throw new Error('脚本缺少 getUrl 方法');
            }
            var song = JSON.parse(toText(songJson));
            var requested = toText(quality).trim();
            var result = source.getUrl(song, requested);
            if (isThenable(result)) {
                return beginPending(result, function (value) {
                    return envelopeOk(normalizeUrlResult(value, requested));
                });
            }
            return envelopeOk(normalizeUrlResult(result, requested));
        } catch (error) {
            return envelopeError(error);
        }
    };

    // ---- 契约 §2 宿主全局 ----

    function buildResponse(raw) {
        return JSON.parse(toText(raw));
    }

    root.http = {
        get: function (url, headers) {
            return buildResponse(callHost('__hd_request', [
                'GET',
                toText(url),
                JSON.stringify(normalizeHeaders(headers) || {}),
                ''
            ]));
        },
        post: function (url, body, headers) {
            return buildResponse(callHost('__hd_request', [
                'POST',
                toText(url),
                JSON.stringify(normalizeHeaders(headers) || {}),
                body === null || body === undefined ? '' : toText(body)
            ]));
        },
        request: function (options) {
            options = options || {};
            return buildResponse(callHost('__hd_request', [
                toText(options.method || 'GET').toUpperCase(),
                toText(options.url),
                JSON.stringify(normalizeHeaders(options.headers) || {}),
                options.body === null || options.body === undefined ? '' : toText(options.body)
            ]));
        }
    };

    root.log = function () {
        var parts = [];
        for (var index = 0; index < arguments.length; index++) {
            parts.push(toText(arguments[index]));
        }
        try {
            callHost('__hd_log', [parts.join(' ')]);
        } catch (error) {
            // 日志失败不应打断脚本。
        }
    };

    root.base64 = {
        encode: function (text) {
            return toText(callHost('__hd_b64', ['encode', toText(text)]));
        },
        decode: function (text) {
            return toText(callHost('__hd_b64', ['decode', toText(text)]));
        }
    };

    root.sleep = function (ms) {
        var value = Number(ms);
        if (!isFinite(value) || value < 0) {
            value = 0;
        }
        callHost('__hd_sleep', [String(Math.floor(value))]);
    };

    function unavailable(name) {
        return function () {
            throw new Error(name + ' 在音源脚本环境中不可用（请使用 http.get/post/request 与 sleep）');
        };
    }

    root.fetch = unavailable('fetch');
    root.XMLHttpRequest = unavailable('XMLHttpRequest');
    root.setTimeout = unavailable('setTimeout');
    root.setInterval = unavailable('setInterval');
    root.setImmediate = unavailable('setImmediate');
    root.require = unavailable('require');
    root.process = {
        platform: 'hdscript',
        argv: [],
        env: {},
        exit: unavailable('process.exit'),
        cwd: unavailable('process.cwd')
    };
})();

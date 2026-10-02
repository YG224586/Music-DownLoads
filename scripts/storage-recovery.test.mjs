import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'
import ts from 'typescript'

const source = readFileSync(
    new URL('../src/api/storageRecovery.ts', import.meta.url),
    'utf8',
)
const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2022,
    },
})

function loadNotifier(report, error) {
    const notices = []
    const errors = []
    const exports = {}
    vm.runInNewContext(outputText, {
        exports,
        require(name) {
            assert.equal(name, '@tauri-apps/api/core')
            return {
                async invoke(command) {
                    assert.equal(command, 'get_storage_recovery_report')
                    if (error) throw error
                    return report
                },
            }
        },
        window: {
            $notify: {
                warning: (notice) =>
                    notices.push({ level: 'warning', ...notice }),
                error: (notice) => notices.push({ level: 'error', ...notice }),
            },
        },
        console: { error: (...args) => errors.push(args) },
    })
    return { show: exports.showStorageRecoveryReport, notices, errors }
}

function report(overrides = {}) {
    return {
        recoveredCount: 2,
        migratedCount: 0,
        errors: ['第 3 条任务记录: missing field `filename`'],
        backupPath: 'D:/AppData/recovery/data-test.json.bak',
        readOnly: false,
        ...overrides,
    }
}

test('normal startup has no recovery notification', async () => {
    const notifier = loadNotifier(null)
    await notifier.show()
    assert.equal(notifier.notices.length, 0)
})

test('partial recovery shows retained task count and backup location until dismissed', async () => {
    const notifier = loadNotifier(report())
    await notifier.show()
    const notice = notifier.notices[0]
    assert.equal(notice.level, 'warning')
    assert.match(notice.description, /已恢复 2 条任务记录/)
    assert.match(notice.description, /missing field `filename`/)
    assert.match(
        notice.description,
        /D:\/AppData\/recovery\/data-test.json.bak/,
    )
    assert.equal(notice.duration, 0)
    assert.equal(notice.closable, true)
})

test('failed backup clearly reports read-only storage and unchanged original', async () => {
    const notifier = loadNotifier(report({ readOnly: true, backupPath: null }))
    await notifier.show()
    const notice = notifier.notices[0]
    assert.equal(notice.level, 'error')
    assert.match(notice.description, /原始文件保持不变/)
    assert.match(notice.description, /任务、设置和历史记录暂时无法保存/)
    assert.match(notice.description, /重启应用/)
})

test('successful migration reports platform completion without corruption wording', async () => {
    const notifier = loadNotifier(report({ migratedCount: 2, errors: [] }))
    await notifier.show()
    const notice = notifier.notices[0]
    assert.match(notice.description, /补全 2 条旧任务的平台信息/)
    assert.match(notice.description, /历史任务已完成升级/)
    assert.doesNotMatch(notice.description, /发现 .*项异常/)
})

test('report command failure is logged without rejecting startup', async () => {
    const notifier = loadNotifier(null, new Error('IPC unavailable'))
    await notifier.show()
    assert.equal(notifier.notices.length, 0)
    assert.equal(notifier.errors.length, 1)
})

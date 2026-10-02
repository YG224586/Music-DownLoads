import { invoke } from '@tauri-apps/api/core'

interface StorageRecoveryReport {
    recoveredCount: number
    migratedCount: number
    errors: string[]
    backupPath: string | null
    readOnly: boolean
}

/** 挂载后读取报告，确保启动恢复提示不会因前端尚未监听而丢失。 */
export async function showStorageRecoveryReport() {
    try {
        const report = await invoke<StorageRecoveryReport | null>(
            'get_storage_recovery_report',
        )
        if (!report) return

        const description = [
            `已恢复 ${report.recoveredCount} 条任务记录，补全 ${report.migratedCount} 条旧任务的平台信息。`,
            report.errors.length
                ? `发现 ${report.errors.length} 项异常：${report.errors[0]}`
                : '',
            report.backupPath
                ? `原始数据备份：${report.backupPath}`
                : '原始文件保持不变。',
            report.readOnly
                ? '当前处于只读恢复模式，任务、设置和历史记录暂时无法保存。请检查磁盘权限及可用空间后重启应用。'
                : report.errors.length
                  ? '异常数据保存在备份中，正常任务可继续使用。'
                  : '历史任务已完成升级，可继续使用。',
        ]
            .filter(Boolean)
            .join('\n')
        window.$notify?.[report.readOnly ? 'error' : 'warning']({
            title: report.readOnly ? '本地数据恢复需要处理' : '本地数据已恢复',
            description,
            duration: 0,
            closable: true,
        })
    } catch (error) {
        console.error('读取本地数据恢复报告失败:', error)
    }
}

/**
 * IPC 错误 → 可展示文案。
 *
 * bridge 抛出的形态是 `IPC invoke <cmd> failed: <原因>`：直接展示会把命令名与
 * 平台实现细节泄露到界面上（例如「Command get_project_dir not found」），对用户
 * 没有任何可操作性。这里统一做三件事：
 *
 * 1. 有可读原因 → 只取原因部分（去掉 `IPC invoke xxx failed:` 前缀）；
 * 2. 命令未注册（前端已更新、后端未重启）→ 给出「重启应用」的可操作指引；
 * 3. 原因本身不可读（透传的平台错误）→ 使用调用方提供的兜底文案。
 *
 * 注：`MobilePage.tsx` 内有一个同类实现（`cleanIpcError`，仅去前缀）。后续统一
 * 到本模块时需同时改其调用点，当前保持不动以免波及该页文案。
 */
export function friendlyIpcError(e: unknown, fallback = '操作失败，请稍后重试'): string {
  const raw = e instanceof Error ? e.message : String(e)
  const reason = raw.match(/failed:\s*(.+)$/)?.[1]?.trim() ?? raw

  // 前端命令已更新、后端仍是旧进程（最常见：更新后未重启应用）
  if (/not found|unknown command|not allowed|not defined/i.test(reason)) {
    return '当前后端尚未加载该功能，请重启应用后重试'
  }

  // 原因缺失或仍是原始 IPC 文本 → 兜底文案（细节留在 console）
  if (!reason.trim() || /IPC invoke/i.test(reason)) return fallback
  return reason
}

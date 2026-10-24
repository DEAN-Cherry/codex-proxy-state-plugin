import type { OverviewAccount } from './api'

export type RowStatus = 'matched' | 'mismatch' | 'observed' | 'unobserved' | 'expired' | 'unavailable'
export const statusNames: Record<RowStatus, string> = {
  matched: '匹配', mismatch: '不匹配', observed: '已观测',
  unobserved: '未观测', expired: '已过期', unavailable: '读取失败',
}

export function accountLabel(account: Pick<OverviewAccount, 'accountId' | 'accountName'>): string {
  return account.accountName.trim() || account.accountId
}

export function modelStatuses(model: OverviewAccount['models'][number]): RowStatus[] {
  const statuses: RowStatus[] = []
  if (model.mismatched > 0) statuses.push('mismatch')
  if (model.matched > 0) statuses.push('matched')
  if (model.observations > model.matched + model.mismatched) statuses.push('observed')
  if (model.expired) statuses.push('expired')
  return statuses.length ? statuses : ['observed']
}

export type AccountGroupView = OverviewAccount & { archived: number }

/** 过期超过展示期限的模型收入详情的「历史 State」；筛选「已过期」时不收起。 */
export function isArchived(model: OverviewAccount['models'][number], nowMs: number, graceMs: number): boolean {
  return model.expired && nowMs - model.expiresAtMs >= graceMs
}

export function filterAccountGroups(
  accounts: OverviewAccount[],
  filters: { account: string, model: string, status: RowStatus | '' },
  display: { nowMs: number, graceMs: number } = { nowMs: 0, graceMs: Infinity },
): AccountGroupView[] {
  const accountNeedle = filters.account.trim().toLowerCase()
  const modelNeedle = filters.model.trim().toLowerCase()
  return accounts.flatMap(account => {
    if (accountNeedle && ![account.accountName, account.accountId].some(value => value.toLowerCase().includes(accountNeedle))) return []
    if (!account.models.length) {
      const status = account.error ? 'unavailable' : 'unobserved'
      return !modelNeedle && (!filters.status || filters.status === status) ? [{ ...account, archived: 0 }] : []
    }
    const shown = filters.status === 'expired' ? account.models : account.models.filter(model => !isArchived(model, display.nowMs, display.graceMs))
    const archived = account.models.length - shown.length
    const models = shown.filter(model =>
      (!modelNeedle || model.model.toLowerCase().includes(modelNeedle))
      && (!filters.status || modelStatuses(model).includes(filters.status)),
    )
    // 全部模型已收起且未筛选时保留账号组，提示到详情查看历史 State。
    if (!models.length && !shown.length && !modelNeedle && !filters.status) return [{ ...account, models, archived }]
    return models.length ? [{ ...account, models, archived }] : []
  })
}

export function clampPage(currentPage: number, total: number, pageSize: number): number {
  return Math.min(Math.max(1, currentPage), Math.max(1, Math.ceil(total / pageSize)))
}

export function pageGroups<T>(accounts: T[], currentPage: number, pageSize: number): T[] {
  const page = clampPage(currentPage, accounts.length, pageSize)
  return accounts.slice((page - 1) * pageSize, page * pageSize)
}

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

export function filterAccountGroups(
  accounts: OverviewAccount[],
  filters: { account: string, model: string, status: RowStatus | '' },
): OverviewAccount[] {
  const accountNeedle = filters.account.trim().toLowerCase()
  const modelNeedle = filters.model.trim().toLowerCase()
  return accounts.flatMap(account => {
    if (accountNeedle && ![account.accountName, account.accountId].some(value => value.toLowerCase().includes(accountNeedle))) return []
    if (!account.models.length) {
      const status = account.error ? 'unavailable' : 'unobserved'
      return !modelNeedle && (!filters.status || filters.status === status) ? [account] : []
    }
    const models = account.models.filter(model =>
      (!modelNeedle || model.model.toLowerCase().includes(modelNeedle))
      && (!filters.status || modelStatuses(model).includes(filters.status)),
    )
    return models.length ? [{ ...account, models }] : []
  })
}

export function clampPage(currentPage: number, total: number, pageSize: number): number {
  return Math.min(Math.max(1, currentPage), Math.max(1, Math.ceil(total / pageSize)))
}

export function pageGroups(accounts: OverviewAccount[], currentPage: number, pageSize: number): OverviewAccount[] {
  const page = clampPage(currentPage, accounts.length, pageSize)
  return accounts.slice((page - 1) * pageSize, page * pageSize)
}

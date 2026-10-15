import type {OperationProgressState, PackHydrateProgress, PlanOperationPreview} from './types'

export const REPOSITORY_SETUP_PROGRESS_INDEX = 0
export const REPOSITORY_SETUP_PROGRESS_TARGET = 'Create or verify repository'

export function hydrateProgressStatus(status: PackHydrateProgress['status']): OperationProgressState {
  if (status === 'started') return 'running'
  if (status === 'completed') return 'done'
  if (status === 'skipped') return 'skipped'
  return 'failed'
}

export function pendingWriteProgress(operations: PlanOperationPreview[]) {
  const progress: Record<number, OperationProgressState> = {
    [REPOSITORY_SETUP_PROGRESS_INDEX]: 'running',
  }
  for (const operation of operations) {
    if (operation.writesToGithub) progress[operation.index] = 'pending'
  }
  return progress
}

export function completeRemainingProgress(
  operations: PlanOperationPreview[],
  current: Record<number, OperationProgressState>,
) {
  const progress: Record<number, OperationProgressState> = {
    ...current,
    [REPOSITORY_SETUP_PROGRESS_INDEX]: 'done',
  }
  for (const operation of operations) {
    if (!operation.writesToGithub) continue
    const state = current[operation.index]
    progress[operation.index] = state === 'failed' ? state : 'done'
  }
  return progress
}

export function newHydrateRunId() {
  return window.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

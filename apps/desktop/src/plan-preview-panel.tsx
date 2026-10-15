import {Box, Text} from '@primer/react'
import {listen} from '@tauri-apps/api/event'
import {Github, Play, SlidersHorizontal, TriangleAlert} from 'lucide-react'
import React from 'react'
import {CLOSE_TRANSIENT_POPUPS_EVENT, closeTransientPopups, invoke} from './app-runtime'
import {
  REPOSITORY_SETUP_PROGRESS_INDEX,
  REPOSITORY_SETUP_PROGRESS_TARGET,
  completeRemainingProgress,
  hydrateProgressStatus,
  newHydrateRunId,
  pendingWriteProgress,
} from './hydrate-progress'
import {HydrateConfirmationDialog} from './hydrate-confirmation-dialog'
import {RowsIcon} from './icons'
import {OperationPreviewLightbox, OperationRow} from './operation-preview'
import {TargetRepositoryPicker} from './target-repository-picker'
import type {
  GitHubTargetRepositoryStatus,
  HydrateMode,
  OperationProgressState,
  PackDryRunReport,
  PackHydrateProgress,
  PackHydrateReport,
  PackListItem,
  PackPlanPreview,
  PackReconciliationPreview,
  PlanOperationPreview,
  PlanOperationReconciliationPreview,
} from './types'

export function PlanPreviewPanel({
  packSource,
  selectedPack,
  repoSource,
  repo,
  onRepoChange,
}: {
  packSource: string
  selectedPack: PackListItem | null
  repoSource: string
  repo: string
  onRepoChange: (value: string) => void
}) {
  const [hydrateMode, setHydrateMode] = React.useState<HydrateMode>('exact')
  const allowNonEmpty = hydrateMode === 'existing'
  const [targetStatus, setTargetStatus] = React.useState<GitHubTargetRepositoryStatus | null>(null)
  const [preview, setPreview] = React.useState<PackPlanPreview | null>(null)
  const [reconciliation, setReconciliation] = React.useState<PackReconciliationPreview | null>(null)
  const [reconciliationLoading, setReconciliationLoading] = React.useState(false)
  const [reconciliationError, setReconciliationError] = React.useState<string | null>(null)
  const [reconciliationRefreshKey, setReconciliationRefreshKey] = React.useState(0)
  const [error, setError] = React.useState<string | null>(null)
  const [loading, setLoading] = React.useState(false)
  const [running, setRunning] = React.useState(false)
  const [completedSteps, setCompletedSteps] = React.useState(0)
  const [dryRunReport, setDryRunReport] = React.useState<PackDryRunReport | null>(null)
  const [dryRunError, setDryRunError] = React.useState<string | null>(null)
  const [hydrating, setHydrating] = React.useState(false)
  const [hydrateReport, setHydrateReport] = React.useState<PackHydrateReport | null>(null)
  const [hydrateError, setHydrateError] = React.useState<string | null>(null)
  const [hydrateConfirming, setHydrateConfirming] = React.useState(false)
  const [hydrateProgress, setHydrateProgress] = React.useState<Record<number, OperationProgressState>>({})
  const [repositorySetupTarget, setRepositorySetupTarget] = React.useState(REPOSITORY_SETUP_PROGRESS_TARGET)
  const hydrateRunIdRef = React.useRef<string | null>(null)
  const [operationScope, setOperationScope] = React.useState<'writes' | 'warmups'>('writes')
  const [previewOperation, setPreviewOperation] = React.useState<PlanOperationPreview | null>(null)
  const requestId = React.useRef(0)
  const reconciliationRequestId = React.useRef(0)
  const planOptionsRef = React.useRef<HTMLDetailsElement | null>(null)

  const previewPlan = React.useCallback(() => {
    const currentRequest = requestId.current + 1
    const startedAt = window.performance.now()
    requestId.current = currentRequest
    setLoading(true)
    setError(null)
    setDryRunError(null)
    setDryRunReport(null)
    setHydrateError(null)
    setHydrateReport(null)
    setReconciliation(null)
    setReconciliationError(null)
    setHydrateProgress({})
    setRepositorySetupTarget(REPOSITORY_SETUP_PROGRESS_TARGET)
    hydrateRunIdRef.current = null
    invoke<PackPlanPreview>('preview_pack_plan', {packSource, repoSource, repo, allowNonEmpty})
      .then(result => {
        if (requestId.current === currentRequest) setPreview(result)
      })
      .catch((cause: unknown) => {
        if (requestId.current !== currentRequest) return
        setPreview(null)
        setError(cause instanceof Error ? cause.message : String(cause))
      })
      .finally(() => {
        const finishLoading = () => {
          if (requestId.current === currentRequest) setLoading(false)
        }
        window.setTimeout(finishLoading, Math.max(0, 220 - (window.performance.now() - startedAt)))
      })
  }, [allowNonEmpty, packSource, repo, repoSource])

  React.useEffect(() => {
    previewPlan()
  }, [previewPlan])

  React.useEffect(() => {
    if (hydrateMode !== 'existing' || !preview) {
      reconciliationRequestId.current += 1
      setReconciliation(null)
      setReconciliationError(null)
      setReconciliationLoading(false)
      return
    }

    if (targetStatus?.exists === false) {
      reconciliationRequestId.current += 1
      setReconciliation(missingRepositoryReconciliation(preview))
      setReconciliationError(null)
      setReconciliationLoading(false)
      return
    }

    if (targetStatus?.exists !== true) {
      reconciliationRequestId.current += 1
      setReconciliation(null)
      setReconciliationError(null)
      setReconciliationLoading(true)
      return
    }

    const currentRequest = reconciliationRequestId.current + 1
    reconciliationRequestId.current = currentRequest
    setReconciliationLoading(true)
    setReconciliationError(null)
    const timer = window.setTimeout(() => {
      invoke<PackReconciliationPreview>('preview_pack_reconciliation', {packSource, repoSource, repo, allowNonEmpty})
        .then(result => {
          if (reconciliationRequestId.current === currentRequest) setReconciliation(result)
        })
        .catch((cause: unknown) => {
          if (reconciliationRequestId.current !== currentRequest) return
          setReconciliation(null)
          setReconciliationError(cause instanceof Error ? cause.message : String(cause))
        })
        .finally(() => {
          if (reconciliationRequestId.current === currentRequest) setReconciliationLoading(false)
        })
    }, 600)
    return () => window.clearTimeout(timer)
  }, [allowNonEmpty, hydrateMode, packSource, preview, reconciliationRefreshKey, repo, repoSource, targetStatus?.exists])

  React.useEffect(() => {
    let unlisten: (() => void) | null = null
    let disposed = false
    void listen<PackHydrateProgress>('pack-hydrate-progress', event => {
      const progress = event.payload
      if (progress.runId !== hydrateRunIdRef.current) return
      if (progress.index === REPOSITORY_SETUP_PROGRESS_INDEX) setRepositorySetupTarget(progress.target)
      setHydrateProgress(current => ({
        ...current,
        [progress.index]: hydrateProgressStatus(progress.status),
      }))
    }).then(cleanup => {
      if (disposed) {
        cleanup()
        return
      }
      unlisten = cleanup
    })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  React.useEffect(() => {
    const close = () => {
      planOptionsRef.current?.removeAttribute('open')
    }
    const handlePointerDown = (event: PointerEvent) => {
      const menu = planOptionsRef.current
      if (!menu?.open || menu.contains(event.target as Node)) return
      menu.removeAttribute('open')
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') planOptionsRef.current?.removeAttribute('open')
    }
    window.addEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
    window.addEventListener('pointerdown', handlePointerDown)
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
      window.removeEventListener('pointerdown', handlePointerDown)
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [])

  const writeOperations = React.useMemo(
    () => preview?.operations.filter(operation => operation.writesToGithub) ?? [],
    [preview],
  )
  const warmupOperations = React.useMemo(
    () => preview?.operations.filter(operation => !operation.writesToGithub) ?? [],
    [preview],
  )
  const visibleOperations = operationScope === 'writes' ? writeOperations : warmupOperations

  React.useEffect(() => {
    if (operationScope === 'warmups' && warmupOperations.length === 0) setOperationScope('writes')
    if (operationScope === 'writes' && writeOperations.length === 0 && warmupOperations.length > 0) setOperationScope('warmups')
  }, [operationScope, warmupOperations.length, writeOperations.length])

  const handleSubmit = React.useCallback((event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    previewPlan()
  }, [previewPlan])

  const executeDryRun = React.useCallback(() => {
    if (!preview) return
    setRunning(true)
    setCompletedSteps(0)
    setDryRunError(null)
    setDryRunReport(null)
    setHydrateError(null)
    setHydrateReport(null)
    setHydrateProgress({})
    hydrateRunIdRef.current = null
    setPreviewOperation(null)
    invoke<PackDryRunReport>('execute_pack_dry_run', {packSource, repoSource, repo, allowNonEmpty})
      .then(result => {
        setCompletedSteps(result.totalOperations)
        setDryRunReport(result)
      })
      .catch((cause: unknown) => {
        setCompletedSteps(0)
        setDryRunError(cause instanceof Error ? cause.message : String(cause))
      })
      .finally(() => setRunning(false))
  }, [allowNonEmpty, packSource, preview, repo, repoSource])

  const executeHydrate = React.useCallback(() => {
    if (!preview) return
    setHydrating(true)
    setHydrateError(null)
    setHydrateReport(null)
    setHydrateProgress(pendingWriteProgress(preview.operations))
    setRepositorySetupTarget(REPOSITORY_SETUP_PROGRESS_TARGET)
    setDryRunError(null)
    setPreviewOperation(null)
    const runId = newHydrateRunId()
    hydrateRunIdRef.current = runId
    invoke<PackHydrateReport>('execute_pack_hydrate', {packSource, repoSource, repo, allowNonEmpty, hydrateMode, runId})
      .then(result => {
        setHydrateReport(result)
        setHydrateProgress(current => completeRemainingProgress(preview.operations, current))
      })
      .catch((cause: unknown) => setHydrateError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => {
        setHydrating(false)
        if (hydrateMode === 'existing') setReconciliationRefreshKey(key => key + 1)
      })
  }, [allowNonEmpty, hydrateMode, packSource, preview, repo, repoSource])

  const completed = Boolean(dryRunReport) && preview ? completedSteps >= preview.totalOperations : false
  const hydrated = Boolean(hydrateReport)
  const loadingMessage = selectedPack ? `Loading ${selectedPack.name} preview...` : 'Loading operation preview...'
  const existingReconciliationBlocked = hydrateMode === 'existing' && (
    reconciliationLoading ||
    Boolean(reconciliationError) ||
    !reconciliation ||
    reconciliation.conflicts > 0
  )
  const liveWriteDisabled = !preview || loading || running || hydrating || existingReconciliationBlocked
  const hydrateDisabledReason = hydrateMode === 'existing'
    ? reconciliationLoading
      ? 'Checking existing repository reconciliation'
      : reconciliationError
        ? 'Reconciliation check failed'
        : reconciliation?.conflicts
          ? 'Resolve user-owned conflicts before hydrating'
          : !reconciliation
            ? 'Waiting for repository reconciliation'
            : null
    : null
  const hydrateButtonTitle = hydrating ? 'Hydrating repository' : hydrateDisabledReason ?? 'Hydrate repository'
  const reconciliationByIndex = React.useMemo(() => {
    const byIndex = new Map<number, PlanOperationReconciliationPreview>()
    for (const operation of reconciliation?.operations ?? []) byIndex.set(operation.index, operation)
    return byIndex
  }, [reconciliation])
  const hydrateWriteProgress = React.useMemo(() => {
    const repositorySetupState = hydrateProgress[REPOSITORY_SETUP_PROGRESS_INDEX]
    const repositorySetupProcessed = repositorySetupState === 'done' || repositorySetupState === 'skipped' || repositorySetupState === 'failed'
    const processedWrites = writeOperations.filter(operation => {
      const state = hydrateProgress[operation.index]
      return state === 'done' || state === 'skipped' || state === 'failed'
    }).length
    const totalSteps = writeOperations.length + 1
    const processedSteps = (repositorySetupProcessed ? 1 : 0) + processedWrites
    const runningOperation = repositorySetupState === 'running'
      ? {target: repositorySetupTarget}
      : writeOperations.find(operation => hydrateProgress[operation.index] === 'running') ?? null
    return {
      processedSteps: hydrated ? totalSteps : processedSteps,
      totalSteps,
      runningOperation,
    }
  }, [hydrateProgress, hydrated, repositorySetupTarget, writeOperations])
  const hydrateProgressPercent = hydrateWriteProgress.totalSteps > 0
    ? Math.round((hydrateWriteProgress.processedSteps / hydrateWriteProgress.totalSteps) * 100)
    : 0
  const hydrateProgressTitle = hydrateError ? 'Hydrate failed' : hydrated ? 'Hydrate complete' : 'Hydrating repository'
  const hydrateProgressDetail = hydrateError
    ? 'Stopped before completing all writes.'
    : hydrateWriteProgress.runningOperation
      ? `Current: ${hydrateWriteProgress.runningOperation.target}`
      : hydrated
      ? 'All GitHub writes completed.'
      : 'Starting hydrate... Creating or checking the repository before the first write can take a moment.'
  const exactHydrateUnavailable = Boolean(targetStatus?.exists && !targetStatus.canExactHydrate)
  React.useEffect(() => {
    if (exactHydrateUnavailable && hydrateMode === 'exact') setHydrateMode('existing')
  }, [exactHydrateUnavailable, hydrateMode])
  const handleTargetStatusChange = React.useCallback((status: GitHubTargetRepositoryStatus | null) => {
    setTargetStatus(status)
  }, [])

  return (
    <Box className="preview-panel">
      <Box className="detail-header">
        <Box className="detail-actions" as="form" onSubmit={handleSubmit} autoComplete="off">
          <TargetRepositoryPicker repo={repo} onRepoChange={onRepoChange} onStatusChange={handleTargetStatusChange} />
          <details ref={planOptionsRef} className="plan-options">
            <summary
              aria-label="Plan options"
              title="Plan options"
              onClick={() => {
                if (!planOptionsRef.current?.open) closeTransientPopups()
              }}
            >
              <SlidersHorizontal size={16} />
            </summary>
            <Box className="plan-options-menu" role="group" aria-label="Hydrate behavior">
              <Text className="plan-options-heading">Hydrate behavior</Text>
              <label className="plan-radio-option">
                <input
                  type="radio"
                  name="hydrate-mode"
                  checked={hydrateMode === 'exact'}
                  disabled={exactHydrateUnavailable}
                  onChange={() => setHydrateMode('exact')}
                />
                <span>
                  <strong>Exact hydrate</strong>
                  <small>
                    {exactHydrateUnavailable
                      ? 'Needs repository admin rights and a token with delete_repo scope.'
                      : 'Delete and recreate the repo first. Best for a clean, exact starter state.'}
                  </small>
                </span>
              </label>
              <label className="plan-radio-option">
                <input
                  type="radio"
                  name="hydrate-mode"
                  checked={hydrateMode === 'existing'}
                  onChange={() => setHydrateMode('existing')}
                />
                <span>
                  <strong>Hydrate into existing</strong>
                  <small>Keep the repo, add missing pack data, and stop before overwriting unmarked conflicts.</small>
                </span>
              </label>
            </Box>
          </details>
          <button
            type="submit"
            className="plan-icon-button"
            aria-label="Update operation preview"
            title="Update operation preview"
            disabled={loading}
          >
            <RowsIcon size={16} />
          </button>
          <button
            type="button"
            className="plan-icon-button plan-icon-button-primary"
            aria-label={running ? 'Running no-write dry-run' : dryRunReport ? 'Run dry-run again' : 'Run no-write dry-run'}
            title={running ? 'Running no-write dry-run' : dryRunReport ? 'Run dry-run again' : 'Run no-write dry-run'}
            disabled={!preview || loading || running || hydrating}
            onClick={executeDryRun}
          >
            <Play size={16} />
          </button>
          <button
            type="button"
            className="plan-icon-button plan-hydrate-button"
            aria-label={hydrateButtonTitle}
            title={hydrateButtonTitle}
            disabled={liveWriteDisabled}
            onClick={() => setHydrateConfirming(true)}
          >
            <Github size={16} />
          </button>
        </Box>
      </Box>

      {loading ? (
        <Box className="plan-loading-banner" role="status" aria-live="polite">
          <span className="plan-loading-spinner" aria-hidden="true" />
          <Text sx={{fontSize: 0}}>{loadingMessage}</Text>
        </Box>
      ) : null}

      {!loading && error ? (
        <Box sx={{border: '1px solid', borderColor: 'danger.muted', borderRadius: 2, p: 2, mt: 3}}>
          <Text sx={{color: 'danger.fg'}}>{error}</Text>
        </Box>
      ) : null}
      {dryRunError ? (
        <Box className="pane-message pane-message-danger plan-dry-run-message" role="alert">
          <TriangleAlert size={16} />
          <Text sx={{fontSize: 0}}>{dryRunError}</Text>
        </Box>
      ) : null}
      {hydrateError ? (
        <Box className="pane-message pane-message-danger plan-dry-run-message" role="alert">
          <TriangleAlert size={16} />
          <Text sx={{fontSize: 0}}>{hydrateError}</Text>
        </Box>
      ) : null}
      {hydrateMode === 'existing' ? (
        <ReconciliationPanel
          reconciliation={reconciliation}
          loading={reconciliationLoading}
          error={reconciliationError}
        />
      ) : null}

      {preview ? (
        <Box className={loading ? 'plan-results plan-results-loading' : 'plan-results'} aria-busy={loading}>
          <Box className="operation-queue">
            <Box className="queue-toolbar">
              <Box className="operation-tabs" role="tablist" aria-label="Operation scope">
                <button
                  type="button"
                  role="tab"
                  aria-selected={operationScope === 'writes'}
                  className={operationScope === 'writes' ? 'operation-tab selected' : 'operation-tab'}
                  onClick={() => setOperationScope('writes')}
                >
                  <span>GitHub writes</span>
                  <strong>{writeOperations.length}</strong>
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={operationScope === 'warmups'}
                  className={operationScope === 'warmups' ? 'operation-tab selected' : 'operation-tab'}
                  onClick={() => setOperationScope('warmups')}
                >
                  <span>Local warmups</span>
                  <strong>{warmupOperations.length}</strong>
                </button>
              </Box>
              <Box className="queue-actions">
                {running || completed || hydrating || hydrated ? (
                  <Text className="queue-run-status" sx={{color: 'fg.muted', fontSize: 0}}>
                    {hydrating
                      ? `Hydrating repository · ${hydrateWriteProgress.processedSteps}/${hydrateWriteProgress.totalSteps} steps`
                      : hydrated
                        ? `Hydrate complete · ${hydrateReport?.writeOperations ?? preview.writeOperations} writes · ${hydrateReport?.localWarmups ?? preview.localWarmups} warmups`
                        : running
                          ? 'Running no-write dry-run...'
                          : `Dry-run complete · ${dryRunReport?.writeOperations ?? preview.writeOperations} writes · ${dryRunReport?.localWarmups ?? preview.localWarmups} warmups`}
                  </Text>
                ) : null}
              </Box>
            </Box>
            <Box className="operation-table" role="table" aria-label="Pack operation queue">
              {visibleOperations.map(operation => (
                <OperationRow
                  key={`${operation.index}-${operation.id}`}
                  operation={operation}
                  progress={hydrateProgress[operation.index]}
                  reconciliation={reconciliationByIndex.get(operation.index)}
                  selected={previewOperation?.index === operation.index && previewOperation.id === operation.id}
                  onSelect={() => setPreviewOperation(operation)}
                />
              ))}
            </Box>
          </Box>
        </Box>
      ) : null}
      {!preview && loading ? (
        <Box className="plan-loading-empty" aria-hidden="true">
          <span className="plan-loading-spinner" />
        </Box>
      ) : null}
      <HydrateConfirmationDialog
        open={hydrateConfirming}
        repo={repo}
        preview={preview}
        hydrateMode={hydrateMode}
        allowNonEmpty={allowNonEmpty}
        hydrating={hydrating}
        hydrated={hydrated}
        hydrateError={hydrateError}
        progressTitle={hydrateProgressTitle}
        progressDetail={hydrateProgressDetail}
        processedSteps={hydrateWriteProgress.processedSteps}
        totalSteps={hydrateWriteProgress.totalSteps}
        progressPercent={hydrateProgressPercent}
        onClose={() => setHydrateConfirming(false)}
        onConfirm={executeHydrate}
      />
      <OperationPreviewLightbox operation={previewOperation} onClose={() => setPreviewOperation(null)} />
    </Box>
  )
}

function ReconciliationPanel({
  reconciliation,
  loading,
  error,
}: {
  reconciliation: PackReconciliationPreview | null
  loading: boolean
  error: string | null
}) {
  if (loading) {
    return (
      <Box className="reconciliation-panel" role="status">
        <span className="plan-loading-spinner" aria-hidden="true" />
        <Text sx={{fontSize: 0}}>Checking existing repository for pack-owned resources and user-owned conflicts...</Text>
      </Box>
    )
  }

  if (error) {
    return (
      <Box className="reconciliation-panel reconciliation-panel-danger" role="alert">
        <TriangleAlert size={16} />
        <Box>
          <Text sx={{display: 'block', fontWeight: 600, fontSize: 0}}>Reconciliation check failed</Text>
          <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0}}>{error}</Text>
        </Box>
      </Box>
    )
  }

  if (!reconciliation) return null

  const totalSafe = reconciliation.missing + reconciliation.packOwned + reconciliation.alreadyPresent
  const conflicts = reconciliation.operations.filter(operation => operation.blocksHydrate)
  const title = reconciliation.conflicts > 0
    ? `${reconciliation.conflicts} user-owned conflict${reconciliation.conflicts === 1 ? '' : 's'} must be resolved before hydrate.`
    : 'Hydrate into existing can proceed without overwriting user-owned resources.'

  return (
    <Box className={reconciliation.conflicts > 0 ? 'reconciliation-panel reconciliation-panel-danger' : 'reconciliation-panel'}>
      {reconciliation.conflicts > 0 ? <TriangleAlert size={16} /> : null}
      <Box className="reconciliation-copy">
        <Text sx={{display: 'block', fontWeight: 600, fontSize: 0}}>{title}</Text>
        <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0}}>
          {totalSafe} GitHub resource{totalSafe === 1 ? '' : 's'} will be created, reused, or skipped by marker; {reconciliation.skipped + reconciliation.localOnly} item{reconciliation.skipped + reconciliation.localOnly === 1 ? '' : 's'} will not write to GitHub.
        </Text>
      </Box>
      <Box className="reconciliation-summary" aria-label="Reconciliation summary">
        <span className="reconciliation-chip reconciliation-chip-missing">{reconciliation.missing} missing</span>
        <span className="reconciliation-chip reconciliation-chip-pack-owned">{reconciliation.packOwned} pack-owned</span>
        <span className="reconciliation-chip reconciliation-chip-present">{reconciliation.alreadyPresent} already present</span>
        <span className="reconciliation-chip reconciliation-chip-conflict">{reconciliation.conflicts} user-owned</span>
      </Box>
      {conflicts.length > 0 ? (
        <Box as="ul" className="reconciliation-conflicts" aria-label="User-owned conflicts">
          {conflicts.map(conflict => (
            <li key={`${conflict.index}-${conflict.kind}-${conflict.target}`}>
              <strong>{conflict.kind}: {conflict.target}</strong>
              <span>{conflict.summary}</span>
            </li>
          ))}
        </Box>
      ) : null}
    </Box>
  )
}

function missingRepositoryReconciliation(preview: PackPlanPreview): PackReconciliationPreview {
  const operations: PlanOperationReconciliationPreview[] = preview.operations.map(operation => {
    if (!operation.writesToGithub) {
      return {
        index: operation.index,
        kind: operation.kind,
        target: operation.target,
        status: 'localOnly',
        summary: 'Warmup guidance is local-only and does not write to GitHub.',
        blocksHydrate: false,
      }
    }
    if (operation.kind === 'Workflow dispatch') {
      return {
        index: operation.index,
        kind: operation.kind,
        target: operation.target,
        status: 'skipped',
        summary: 'Workflow dispatch is skipped because idempotent dispatch markers are not available yet.',
        blocksHydrate: false,
      }
    }
    return {
      index: operation.index,
      kind: operation.kind,
      target: operation.target,
      status: 'missing',
      summary: 'Repository does not exist yet; this resource will be created after repository setup.',
      blocksHydrate: false,
    }
  })
  return summarizeReconciliation(preview.repo, operations)
}

function summarizeReconciliation(repo: string, operations: PlanOperationReconciliationPreview[]): PackReconciliationPreview {
  return {
    repo,
    operations,
    conflicts: operations.filter(operation => operation.blocksHydrate).length,
    missing: operations.filter(operation => operation.status === 'missing').length,
    packOwned: operations.filter(operation => operation.status === 'packOwned').length,
    alreadyPresent: operations.filter(operation => operation.status === 'alreadyPresent').length,
    skipped: operations.filter(operation => operation.status === 'skipped').length,
    localOnly: operations.filter(operation => operation.status === 'localOnly').length,
  }
}

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
  PlanOperationPreview,
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
  const hydrateRunIdRef = React.useRef<string | null>(null)
  const [operationScope, setOperationScope] = React.useState<'writes' | 'warmups'>('writes')
  const [previewOperation, setPreviewOperation] = React.useState<PlanOperationPreview | null>(null)
  const requestId = React.useRef(0)
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
    setHydrateProgress({})
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
    let unlisten: (() => void) | null = null
    void listen<PackHydrateProgress>('pack-hydrate-progress', event => {
      const progress = event.payload
      if (progress.runId !== hydrateRunIdRef.current) return
      setHydrateProgress(current => ({
        ...current,
        [REPOSITORY_SETUP_PROGRESS_INDEX]: 'done',
        [progress.index]: hydrateProgressStatus(progress.status),
      }))
    }).then(cleanup => {
      unlisten = cleanup
    })

    return () => {
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
      })
  }, [allowNonEmpty, hydrateMode, packSource, preview, repo, repoSource])

  const completed = Boolean(dryRunReport) && preview ? completedSteps >= preview.totalOperations : false
  const hydrated = Boolean(hydrateReport)
  const loadingMessage = selectedPack ? `Loading ${selectedPack.name} preview...` : 'Loading operation preview...'
  const liveWriteDisabled = !preview || loading || running || hydrating
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
      ? {target: REPOSITORY_SETUP_PROGRESS_TARGET}
      : writeOperations.find(operation => hydrateProgress[operation.index] === 'running') ?? null
    return {
      processedSteps: hydrated ? totalSteps : processedSteps,
      totalSteps,
      runningOperation,
    }
  }, [hydrateProgress, hydrated, writeOperations])
  const hydrateProgressPercent = hydrateWriteProgress.totalSteps > 0
    ? Math.round((hydrateWriteProgress.processedSteps / hydrateWriteProgress.totalSteps) * 100)
    : 0
  const hydrateProgressTitle = hydrateError ? 'Hydrate failed' : hydrated ? 'Hydrate complete' : 'Hydrating repository'
  const hydrateProgressDetail = hydrateWriteProgress.runningOperation
    ? `Current: ${hydrateWriteProgress.runningOperation.target}`
    : hydrated
      ? 'All GitHub writes completed.'
      : hydrateError
        ? 'Stopped before completing all writes.'
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
            aria-label={hydrating ? 'Hydrating repository' : 'Hydrate repository'}
            title={hydrating ? 'Hydrating repository' : 'Hydrate repository'}
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

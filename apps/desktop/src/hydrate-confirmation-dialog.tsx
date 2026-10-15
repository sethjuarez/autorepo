import {Box, Button, Heading, Text, TextInput} from '@primer/react'
import {Copy, TriangleAlert, X} from 'lucide-react'
import React from 'react'
import type {HydrateMode, PackPlanPreview} from './types'
export function HydrateConfirmationDialog({
  open,
  repo,
  preview,
  hydrateMode,
  allowNonEmpty,
  hydrating,
  hydrated,
  hydrateError,
  progressTitle,
  progressDetail,
  processedSteps,
  totalSteps,
  progressPercent,
  onClose,
  onConfirm,
}: {
  open: boolean
  repo: string
  preview: PackPlanPreview | null
  hydrateMode: HydrateMode
  allowNonEmpty: boolean
  hydrating: boolean
  hydrated: boolean
  hydrateError: string | null
  progressTitle: string
  progressDetail: string
  processedSteps: number
  totalSteps: number
  progressPercent: number
  onClose: () => void
  onConfirm: () => void
}) {
  const [typedRepo, setTypedRepo] = React.useState('')
  const [copiedRepo, setCopiedRepo] = React.useState(false)
  const confirmInputRef = React.useRef<HTMLInputElement | null>(null)
  React.useEffect(() => {
    if (open) {
      setTypedRepo('')
      setCopiedRepo(false)
    }
  }, [open])
  React.useEffect(() => {
    if (!open) return
    const frame = window.requestAnimationFrame(() => confirmInputRef.current?.focus())
    return () => window.cancelAnimationFrame(frame)
  }, [open])
  const copyRepoName = React.useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault()
    event.stopPropagation()
    setCopiedRepo(true)
    void copyText(repo)
  }, [repo])

  if (!open || !preview) return null

  const canConfirm = typedRepo.trim() === repo.trim()
  const showingProgress = hydrating || hydrated || Boolean(hydrateError)
  const canClickOut = !hydrating

  return (
    <Box className="operation-preview-lightbox" role="presentation" onMouseDown={(event: React.MouseEvent<HTMLElement>) => {
      if (event.target === event.currentTarget && canClickOut) onClose()
    }}>
      <Box
        className="operation-preview-dialog hydrate-confirmation-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="hydrate-confirmation-title"
      >
        <Box className="operation-preview-header">
          <Box>
            <Text sx={{display: 'block', color: 'danger.fg', fontSize: 0, fontWeight: 600}}>
              Live GitHub writes
            </Text>
            <Heading id="hydrate-confirmation-title" as="h2" sx={{fontSize: 3, mt: 1, mb: 0}}>
              Hydrate {repo}
            </Heading>
          </Box>
          <button type="button" className="dialog-close" aria-label="Close hydrate" onClick={onClose}>
            <X size={16} />
          </button>
        </Box>
        <Box className="hydrate-confirmation-body">
          {showingProgress ? (
            <>
              <Box className="hydrate-progress-panel hydrate-progress-panel-dialog" role="status" aria-live="polite">
                <Box className="hydrate-progress-copy">
                  <Text className="hydrate-progress-title">{progressTitle}</Text>
                  <Text className="hydrate-progress-current">{progressDetail}</Text>
                </Box>
                <Box
                  className="hydrate-progress"
                  role="progressbar"
                  aria-label={`${processedSteps} of ${totalSteps} Hydrate steps processed`}
                  aria-valuemin={0}
                  aria-valuemax={totalSteps}
                  aria-valuenow={processedSteps}
                >
                  <span style={{width: `${progressPercent}%`}} />
                </Box>
                <Text className="hydrate-progress-count">
                  {processedSteps} of {totalSteps} steps
                </Text>
              </Box>
              {hydrateError ? (
                <Box className="pane-message pane-message-danger hydrate-confirmation-warning" role="alert">
                  <TriangleAlert size={16} />
                  <Text sx={{fontSize: 0}}>{hydrateError}</Text>
                </Box>
              ) : null}
              <Box className="hydrate-confirmation-actions">
                <Button type="button" onClick={onClose}>
                  {hydrating ? 'Cancel' : 'Close'}
                </Button>
              </Box>
            </>
          ) : (
            <>
          <Text sx={{display: 'block', color: 'fg.muted', fontSize: 1}}>
            {hydrateMode === 'exact'
              ? `This will delete and recreate ${repo} if it already exists, then run the pack against a clean repository with ${preview.writeOperations} write operations.`
              : `This will keep ${repo} in place, add missing pack-managed resources, and skip resources that already contain autorepo markers.`}
          </Text>
          {hydrateMode === 'exact' ? (
            <Box className="pane-message pane-message-danger hydrate-confirmation-warning">
              <TriangleAlert size={16} />
              <Text sx={{fontSize: 0}}>
                Exact hydrate is destructive for existing repositories. It removes the current repo before recreating pack data.
              </Text>
            </Box>
          ) : null}
          {allowNonEmpty ? (
            <Box className="pane-message pane-message-danger hydrate-confirmation-warning">
              <TriangleAlert size={16} />
              <Text sx={{fontSize: 0}}>
                Non-empty repositories are allowed for this run. Existing unmarked files, branches, issues, or pull requests with conflicting names can still stop the hydrate.
              </Text>
            </Box>
          ) : null}
          <Box className="hydrate-confirmation-label-row">
            <Text as="label" htmlFor="hydrate-confirm-repo" sx={{display: 'block', fontSize: 0, fontWeight: 600}}>
              Type {repo} to hydrate
            </Text>
            <button
              type="button"
              className="hydrate-copy-button"
              onClick={copyRepoName}
              onMouseDown={copyRepoName}
              aria-label={`Copy ${repo}`}
              title={`Copy ${repo}`}
            >
              <Copy size={14} />
              <span>{copiedRepo ? 'Copied' : 'Copy'}</span>
            </button>
          </Box>
          <Box className="hydrate-confirmation-input-row">
            <TextInput
              ref={confirmInputRef}
              id="hydrate-confirm-repo"
              value={typedRepo}
              onChange={(event: React.ChangeEvent<HTMLInputElement>) => setTypedRepo(event.target.value)}
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
              aria-label="Confirm target repository"
              sx={{width: '100%', fontFamily: 'mono'}}
            />
          </Box>
          <Text sx={{display: 'block', color: canConfirm ? 'success.fg' : 'fg.muted', fontSize: 0}}>
            {canConfirm
              ? 'Ready to hydrate.'
              : `The button unlocks after this exactly matches ${repo}.`}
          </Text>
          <Box className="hydrate-confirmation-actions">
            <Button type="button" onClick={onClose}>
              Cancel
            </Button>
            <Button type="button" variant="danger" disabled={!canConfirm} onClick={onConfirm}>
              Hydrate repository
            </Button>
          </Box>
            </>
          )}
        </Box>
      </Box>
    </Box>
  )
}


async function copyText(value: string) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value)
      return
    } catch {
      // Fall back to the textarea path below.
    }
  }

  const textarea = document.createElement('textarea')
  textarea.value = value
  textarea.setAttribute('readonly', 'true')
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.append(textarea)
  textarea.select()
  document.execCommand('copy')
  textarea.remove()
}


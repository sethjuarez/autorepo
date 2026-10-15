import {Box, Heading, Label, Text} from '@primer/react'
import {X} from 'lucide-react'
import React from 'react'
import {operationAction, operationIcon} from './icons'
import type {OperationProgressState, PlanOperationPreview} from './types'
export function OperationPreviewLightbox({
  operation,
  onClose,
}: {
  operation: PlanOperationPreview | null
  onClose: () => void
}) {
  React.useEffect(() => {
    if (!operation) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose, operation])

  if (!operation) return null

  const {contentPreview} = operation
  const title = contentPreview?.title ?? operation.target
  const subtitle = contentPreview?.subtitle ?? operationAction(operation.kind)

  return (
    <Box className="operation-preview-lightbox" role="presentation" onMouseDown={(event: React.MouseEvent<HTMLElement>) => {
      if (event.target === event.currentTarget) onClose()
    }}>
      <Box
        className="operation-preview-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="operation-preview-title"
        aria-describedby="operation-preview-subtitle"
      >
        <Box className="operation-preview-header">
          <Box>
            <Text id="operation-preview-subtitle" sx={{display: 'block', color: 'fg.muted', fontSize: 0}}>
              {subtitle}
            </Text>
            <Heading id="operation-preview-title" as="h2" sx={{fontSize: 3, mt: 1, mb: 0}}>
              {title}
            </Heading>
          </Box>
          <button type="button" className="dialog-close" aria-label="Close preview" onClick={onClose}>
            <X size={16} />
          </button>
        </Box>
        <Box className="github-preview-card">
          <Box className="github-preview-meta">
            <Label>{operation.kind}</Label>
            <Text sx={{color: 'fg.muted', fontSize: 0}}>Step {operation.index}</Text>
          </Box>
          {operation.details.length > 0 ? (
            <Box as="ul" className="operation-detail-list operation-preview-details">
              {operation.details.map(detail => (
                <li key={detail}>{detail}</li>
              ))}
            </Box>
          ) : null}
          {!contentPreview ? (
            <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0, px: 3, pb: 3}}>
              No generated content preview is available for this operation.
            </Text>
          ) : contentPreview.format === 'code' || contentPreview.format === 'json' ? (
            <pre className="operation-preview-code"><code>{contentPreview.body}</code></pre>
          ) : (
            <Box className="operation-preview-markdown">
              {renderPreviewMarkdown(contentPreview.body)}
            </Box>
          )}
        </Box>
      </Box>
    </Box>
  )
}

function renderPreviewMarkdown(body: string) {
  return body.split(/\r?\n/).map((line, index) => {
    const key = `${index}-${line}`
    if (!line.trim()) return <br key={key} />
    if (line.startsWith('### ')) return <h3 key={key}>{line.slice(4)}</h3>
    if (line.startsWith('## ')) return <h2 key={key}>{line.slice(3)}</h2>
    if (line.startsWith('# ')) return <h1 key={key}>{line.slice(2)}</h1>
    if (/^\d+\.\s/.test(line) || /^[-*]\s/.test(line)) {
      return <p key={key} className="operation-preview-list-line">{line}</p>
    }
    if (line.trim().startsWith('<!--')) {
      return <p key={key} className="operation-preview-comment">{line}</p>
    }
    return <p key={key}>{line}</p>
  })
}

export function OperationRow({
  operation,
  progress,
  selected,
  onSelect,
}: {
  operation: PlanOperationPreview
  progress?: OperationProgressState
  selected: boolean
  onSelect: () => void
}) {
  const Icon = operationIcon(operation.kind)
  const action = operationAction(operation.kind)
  return (
    <button
      type="button"
      className={['operation-row', selected ? 'selected' : null, progress ? `operation-row-${progress}` : null].filter(Boolean).join(' ')}
      onClick={onSelect}
      role="row"
    >
      <Box className="operation-index">{operation.index}</Box>
      <Box className="operation-icon" aria-hidden="true">
        <Icon size={16} />
      </Box>
      <Box className="operation-body">
        <Box className="operation-line">
          <Text className="operation-target">{operation.target}</Text>
          <Label>{operation.kind}</Label>
          <Text className="operation-action">{action}</Text>
        </Box>
      </Box>
    </button>
  )
}



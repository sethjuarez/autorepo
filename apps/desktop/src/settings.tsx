import {Box, Button, Heading, Label, Text, TextInput} from '@primer/react'
import {CheckCircle, Github, KeyRound, Settings, ShieldCheck, TriangleAlert, X} from 'lucide-react'
import React from 'react'
import {invoke} from './app-runtime'
import {KeyIcon, ShieldCheckIcon} from './icons'
import type {GithubAuthStatus} from './types'

export function SettingsLightbox({
  onClose,
  onStatusChange,
}: {
  onClose: () => void
  onStatusChange: (status: GithubAuthStatus | null) => void
}) {
  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [onClose])

  return (
    <Box className="settings-lightbox" role="presentation" onMouseDown={onClose}>
      <Box
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        data-testid="settings-lightbox"
        onMouseDown={(event: React.MouseEvent<HTMLElement>) => event.stopPropagation()}
      >
        <Box className="settings-dialog-header">
          <Box sx={{display: 'flex', alignItems: 'center', gap: 2}}>
            <Settings size={20} />
            <Box>
              <Heading id="settings-title" as="h2" sx={{fontSize: 3, mb: 1}}>
                Settings
              </Heading>
              <Text sx={{color: 'fg.muted'}}>Configure local tools before running repository workflows.</Text>
            </Box>
          </Box>
          <button type="button" className="dialog-close" aria-label="Close settings" onClick={onClose}>
            <X size={18} />
          </button>
        </Box>
        <SettingsPanel onStatusChange={onStatusChange} />
      </Box>
    </Box>
  )
}

function SettingsPanel({onStatusChange}: {onStatusChange: (status: GithubAuthStatus | null) => void}) {
  const [status, setStatus] = React.useState<GithubAuthStatus | null>(null)
  const [loading, setLoading] = React.useState(true)
  const [copied, setCopied] = React.useState(false)
  const authCommand = 'gh auth login --hostname github.com --web --scopes repo'

  const refresh = React.useCallback(() => {
    setLoading(true)
    invoke<GithubAuthStatus>('github_auth_status')
      .then(result => {
        setStatus(result)
        onStatusChange(result)
      })
      .catch((cause: unknown) => {
        const fallback = {
          installed: false,
          authenticated: false,
          login: null,
          avatarUrl: null,
          errorMessage: cause instanceof Error ? cause.message : String(cause),
          apiAuthenticated: false,
          apiLogin: null,
          apiAvatarUrl: null,
          apiErrorMessage: null,
          apiTokenSource: null,
        }
        setStatus(fallback)
        onStatusChange(fallback)
      })
      .finally(() => setLoading(false))
  }, [onStatusChange])

  React.useEffect(() => {
    refresh()
  }, [refresh])

  const copyAuthCommand = async () => {
    await navigator.clipboard?.writeText(authCommand).catch(() => undefined)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  const tone = status?.apiAuthenticated || status?.authenticated ? 'success' : status?.installed ? 'attention' : 'danger'

  return (
    <Box className="settings-panel">
      <Box sx={{border: '1px solid', borderColor: 'border.default', borderRadius: 2, p: 3, bg: 'canvas.subtle'}}>
        <Box sx={{display: 'flex', alignItems: 'start', justifyContent: 'space-between', gap: 3}}>
          <Box sx={{display: 'flex', gap: 2}}>
            <Box className="settings-icon">
              <Github size={22} />
            </Box>
            <Box>
              <Box as="h3" sx={{fontSize: 2, fontWeight: 600, m: 0, mb: 1}}>
                GitHub authentication
              </Box>
              <Text sx={{color: 'fg.muted'}}>
                Autorepo uses your GitHub sign-in to inspect repositories and prepare safe write workflows.
              </Text>
            </Box>
          </Box>
          <Label variant={tone}>
            {loading ? 'Checking' : status?.apiAuthenticated ? 'API connected' : status?.authenticated ? 'CLI connected' : 'Action needed'}
          </Label>
        </Box>

        <Box sx={{display: 'grid', gap: 3, mt: 3}}>
          <Box sx={{display: 'flex', alignItems: 'center', gap: 2}}>
            {status?.apiAuthenticated ? (
              <CheckCircle size={16} className="status-success" />
            ) : (
              <TriangleAlert size={16} className="status-warning" />
            )}
            <Text sx={{fontSize: 1}}>
              {loading
                ? 'Checking GitHub API access...'
                : status?.apiAuthenticated
                  ? `GitHub API calls are ready${status.apiLogin ? ` as ${status.apiLogin}` : ''}${
                      status.apiTokenSource ? ` via ${status.apiTokenSource}` : ''
                    }.`
                  : status?.apiErrorMessage}
            </Text>
          </Box>

          <Box sx={{display: 'flex', alignItems: 'center', gap: 2}}>
            {status?.authenticated ? (
              <CheckCircle size={16} className="status-success" />
            ) : (
              <TriangleAlert size={16} className="status-warning" />
            )}
            <Text sx={{fontSize: 1}}>
              {loading
                ? 'Checking GitHub CLI authentication...'
                : status?.authenticated
                  ? `GitHub CLI sign-in is available${status.login ? ` for ${status.login}` : ''}.`
                  : status?.installed
                    ? 'GitHub CLI is installed, but GitHub.com auth is not ready.'
                    : 'GitHub CLI is not available on PATH.'}
            </Text>
          </Box>

          <Box>
            <Text sx={{display: 'block', fontSize: 1, fontWeight: 600, mb: 2}}>Developer override</Text>
            <Text sx={{display: 'block', color: 'fg.muted', fontSize: 1}}>
              For local testing only, developers can start the app with <code>AUTOREPO_GITHUB_TOKEN</code>. Normal users
              should not need to create or paste tokens.
            </Text>
          </Box>

          <Box>
            <Text as="label" htmlFor="github-auth-command" sx={{display: 'block', fontSize: 1, fontWeight: 600, mb: 2}}>
              GitHub CLI sign-in command
            </Text>
            <Box sx={{display: 'flex', gap: 2}}>
              <TextInput
                id="github-auth-command"
                value={authCommand}
                readOnly
                leadingVisual={KeyIcon}
                aria-label="GitHub CLI sign-in command"
                sx={{flex: 1, fontFamily: 'mono'}}
              />
              <Button onClick={copyAuthCommand}>{copied ? 'Copied' : 'Copy'}</Button>
            </Box>
            <Text as="p" sx={{color: 'fg.muted', fontSize: 0, mt: 2}}>
              Run this in a terminal when you want to connect. The app does not start OAuth or submit credentials for you.
            </Text>
          </Box>

          {status?.errorMessage ? (
            <Box sx={{border: '1px solid', borderColor: 'border.muted', borderRadius: 2, p: 2, bg: 'canvas.default'}}>
              <Text sx={{fontSize: 0, color: 'fg.muted'}}>{status.errorMessage}</Text>
            </Box>
          ) : null}

          <Box sx={{display: 'flex', gap: 2}}>
            <Button leadingVisual={ShieldCheckIcon} onClick={refresh} disabled={loading}>
              {loading ? 'Checking...' : 'Refresh status'}
            </Button>
          </Box>
        </Box>
      </Box>
    </Box>
  )
}

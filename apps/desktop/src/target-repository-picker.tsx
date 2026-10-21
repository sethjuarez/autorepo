import {Box, Label, Text, TextInput} from '@primer/react'
import React from 'react'
import {CLOSE_TRANSIENT_POPUPS_EVENT, closeTransientPopups, invoke} from './app-runtime'
import {dropdownRectFor, dropdownStyle} from './dropdown'
import {FolderGitIcon} from './icons'
import {normalizedRepositoryParts, repoNameFromRepositorySource} from './repository-source'
import {readLastSelectedOwner, saveLastSelectedOwner} from './storage'
import type {DropdownRect, GitHubRepositoryListItem, GitHubRepositoryOwner, GitHubTargetRepositoryStatus} from './types'
export function TargetRepositoryPicker({
  repo,
  onRepoChange,
  onStatusChange,
}: {
  repo: string
  onRepoChange: (value: string) => void
  onStatusChange?: (status: GitHubTargetRepositoryStatus | null) => void
}) {
  const initialParts = React.useMemo(() => normalizedRepositoryParts(repo), [repo])
  const [targetOwners, setTargetOwners] = React.useState<GitHubRepositoryOwner[]>([])
  const [targetOwnersLoading, setTargetOwnersLoading] = React.useState(false)
  const [selectedTargetOwner, setSelectedTargetOwner] = React.useState(() => initialParts?.owner ?? readLastSelectedOwner() ?? 'sethjuarez')
  const [targetOwnerFilter, setTargetOwnerFilter] = React.useState(() => initialParts?.owner ?? readLastSelectedOwner() ?? 'sethjuarez')
  const [targetRepoName, setTargetRepoName] = React.useState(() => initialParts?.repo ?? '')
  const [targetRepositories, setTargetRepositories] = React.useState<GitHubRepositoryListItem[]>([])
  const [targetRepositoriesLoading, setTargetRepositoriesLoading] = React.useState(false)
  const [targetOwnerOpen, setTargetOwnerOpen] = React.useState(false)
  const [targetRepoOpen, setTargetRepoOpen] = React.useState(false)
  const [targetError, setTargetError] = React.useState<string | null>(null)
  const [targetStatus, setTargetStatus] = React.useState<GitHubTargetRepositoryStatus | null>(null)
  const [targetChecking, setTargetChecking] = React.useState(false)
  const targetOwnerRef = React.useRef<HTMLElement | null>(null)
  const targetRepoRef = React.useRef<HTMLElement | null>(null)
  const [targetOwnerRect, setTargetOwnerRect] = React.useState<DropdownRect | null>(null)
  const [targetRepoRect, setTargetRepoRect] = React.useState<DropdownRect | null>(null)

  React.useEffect(() => {
    const close = () => {
      setTargetOwnerOpen(false)
      setTargetRepoOpen(false)
    }
    window.addEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
    return () => window.removeEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
  }, [])

  const positionTargetOwnerDropdown = React.useCallback(() => {
    setTargetOwnerRect(dropdownRectFor(targetOwnerRef.current))
  }, [])

  const positionTargetRepoDropdown = React.useCallback(() => {
    setTargetRepoRect(dropdownRectFor(targetRepoRef.current))
  }, [])

  React.useEffect(() => {
    const parts = normalizedRepositoryParts(repo)
    if (!parts) return
    setSelectedTargetOwner(parts.owner)
    setTargetOwnerFilter(parts.owner)
    setTargetRepoName(parts.repo)
  }, [repo])

  React.useEffect(() => {
    if (targetOwners.length > 0 || targetOwnersLoading) return
    setTargetOwnersLoading(true)
    invoke<GitHubRepositoryOwner[]>('list_github_repository_owners')
      .then(owners => {
        setTargetOwners(owners)
        if (!owners.some(owner => owner.login === selectedTargetOwner) && owners[0]) {
          const userOwner = owners.find(owner => owner.kind === 'User') ?? owners[0]
          setSelectedTargetOwner(userOwner.login)
          setTargetOwnerFilter(userOwner.login)
          if (targetRepoName.trim()) onRepoChange(`${userOwner.login}/${targetRepoName.trim()}`)
        }
      })
      .catch((cause: unknown) => setTargetError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => setTargetOwnersLoading(false))
  }, [onRepoChange, selectedTargetOwner, targetOwners.length, targetOwnersLoading, targetRepoName])

  React.useEffect(() => {
    if (!selectedTargetOwner) return
    setTargetRepositoriesLoading(true)
    setTargetRepositories([])
    invoke<GitHubRepositoryListItem[]>('list_github_owner_repositories', {owner: selectedTargetOwner})
      .then(repositories => setTargetRepositories(repositories))
      .catch((cause: unknown) => setTargetError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => setTargetRepositoriesLoading(false))
  }, [selectedTargetOwner])

  React.useEffect(() => {
    const repoName = targetRepoName.trim()
    if (!selectedTargetOwner || !repoName) {
      setTargetStatus(null)
      onStatusChange?.(null)
      setTargetChecking(false)
      return
    }

    setTargetChecking(true)
    setTargetError(null)
    const timer = window.setTimeout(() => {
      invoke<GitHubTargetRepositoryStatus>('check_github_target_repository', {
        owner: selectedTargetOwner,
        repo: repoName,
      })
        .then(status => {
          setTargetStatus(status)
          onStatusChange?.(status)
        })
        .catch((cause: unknown) => {
          setTargetStatus(null)
          onStatusChange?.(null)
          setTargetError(cause instanceof Error ? cause.message : String(cause))
        })
        .finally(() => setTargetChecking(false))
    }, 220)

    return () => window.clearTimeout(timer)
  }, [onStatusChange, selectedTargetOwner, targetRepoName])

  const filteredTargetOwners = React.useMemo(() => {
    const query = targetOwnerFilter.trim().toLowerCase()
    return targetOwners
      .filter(owner => !query || owner.login.toLowerCase().includes(query) || owner.kind.toLowerCase().includes(query))
      .slice(0, 12)
  }, [targetOwnerFilter, targetOwners])

  const filteredTargetRepositories = React.useMemo(() => {
    const query = targetRepoName.trim().toLowerCase()
    return targetRepositories
      .filter(repository =>
        !query ||
        repository.name.toLowerCase().includes(query) ||
        repository.fullName.toLowerCase().includes(query) ||
        Boolean(repository.description?.toLowerCase().includes(query)),
      )
      .slice(0, 24)
  }, [targetRepoName, targetRepositories])

  const updateTargetRepoName = React.useCallback((repoName: string) => {
    setTargetRepoName(repoName)
    setTargetStatus(null)
    if (selectedTargetOwner && repoName.trim()) onRepoChange(`${selectedTargetOwner}/${repoName.trim()}`)
  }, [onRepoChange, selectedTargetOwner])

  const selectTargetOwner = React.useCallback((owner: GitHubRepositoryOwner) => {
    setSelectedTargetOwner(owner.login)
    setTargetOwnerFilter(owner.login)
    setTargetOwnerOpen(false)
    setTargetStatus(null)
    saveLastSelectedOwner(owner.login)
    if (targetRepoName.trim()) onRepoChange(`${owner.login}/${targetRepoName.trim()}`)
  }, [onRepoChange, targetRepoName])

  return (
    <Box className="target-repo-picker">
      <Text as="label" htmlFor="target-repo-name" sx={{fontSize: 0, color: 'fg.muted'}}>
        Target
      </Text>
      <Box className="target-repo-fields">
        <Box ref={targetOwnerRef} className="source-owner-combobox target-owner-combobox">
          <TextInput
            value={targetOwnerFilter}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) => {
              closeTransientPopups()
              setTargetOwnerFilter(event.target.value)
              setTargetOwnerOpen(true)
              setTargetRepoOpen(false)
              positionTargetOwnerDropdown()
            }}
            onFocus={() => {
              closeTransientPopups()
              setTargetOwnerOpen(true)
              setTargetRepoOpen(false)
              positionTargetOwnerDropdown()
            }}
            onClick={() => {
              closeTransientPopups()
              setTargetOwnerOpen(true)
              setTargetRepoOpen(false)
              positionTargetOwnerDropdown()
            }}
            onBlur={() => window.setTimeout(() => setTargetOwnerOpen(false), 120)}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            aria-label="Filter target owners and organizations"
            aria-expanded={targetOwnerOpen}
            aria-controls="target-owner-results"
            placeholder={targetOwnersLoading ? 'Loading orgs...' : 'Owner'}
            sx={{width: '100%'}}
          />
          {targetOwnerOpen && filteredTargetOwners.length > 0 ? (
            <Box id="target-owner-results" className="source-owner-results" role="listbox" style={dropdownStyle(targetOwnerRect)}>
              {filteredTargetOwners.map(owner => (
                <button
                  key={owner.login}
                  type="button"
                  className={owner.login === selectedTargetOwner ? 'source-owner-result selected' : 'source-owner-result'}
                  onMouseDown={event => event.preventDefault()}
                  onClick={() => selectTargetOwner(owner)}
                >
                  {owner.avatarUrl ? <img src={owner.avatarUrl} alt="" /> : <span className="source-owner-fallback" />}
                  <span>
                    <strong>{owner.login}</strong>
                    <small>{owner.kind}</small>
                  </span>
                </button>
              ))}
            </Box>
          ) : null}
        </Box>
        <Box ref={targetRepoRef} className="source-repo-combobox target-repo-combobox">
          <TextInput
            id="target-repo-name"
            value={targetRepoName}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) => {
              updateTargetRepoName(event.target.value)
              closeTransientPopups()
              setTargetRepoOpen(true)
              setTargetOwnerOpen(false)
              positionTargetRepoDropdown()
            }}
            onFocus={() => {
              closeTransientPopups()
              setTargetRepoOpen(true)
              setTargetOwnerOpen(false)
              positionTargetRepoDropdown()
            }}
            onClick={() => {
              closeTransientPopups()
              setTargetRepoOpen(true)
              setTargetOwnerOpen(false)
              positionTargetRepoDropdown()
            }}
            onBlur={() => window.setTimeout(() => setTargetRepoOpen(false), 120)}
            leadingVisual={FolderGitIcon}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            aria-label="Filter or name target repository"
            aria-expanded={targetRepoOpen}
            aria-controls="target-repo-results"
            placeholder="Repository"
            sx={{width: '100%', fontFamily: 'mono'}}
          />
          {targetRepoOpen && (targetRepositoriesLoading || filteredTargetRepositories.length > 0) ? (
            <Box id="target-repo-results" className="repo-search-results repo-search-dropdown" role="listbox" aria-label="Target repositories" style={dropdownStyle(targetRepoRect)}>
              {targetRepositoriesLoading ? (
                <Box className="repo-search-empty" role="status">
                  Loading repositories...
                </Box>
              ) : null}
              {filteredTargetRepositories.map(repository => (
                <button
                  key={repository.fullName}
                  type="button"
                  className={repository.fullName === targetStatus?.fullName ? 'repo-search-result selected' : 'repo-search-result'}
                  onMouseDown={event => event.preventDefault()}
                  onClick={() => {
                    setSelectedTargetOwner(repository.fullName.split('/')[0] ?? selectedTargetOwner)
                    setTargetOwnerFilter(repository.fullName.split('/')[0] ?? selectedTargetOwner)
                    updateTargetRepoName(repository.name)
                    setTargetRepoOpen(false)
                  }}
                >
                  <span>
                    <strong>{repository.fullName}</strong>
                    {repository.description ? <small>{repository.description}</small> : null}
                    <small>{repository.private ? 'Private' : 'Public'} · {repository.defaultBranch}</small>
                  </span>
                  <Label>{repository.private ? 'Private' : 'Public'}</Label>
                </button>
              ))}
            </Box>
          ) : null}
        </Box>
      </Box>
      {targetChecking ? (
        <Text className="target-repo-status" role="status">
          Checking target repository...
        </Text>
      ) : null}
      {!targetChecking && targetStatus?.exists ? (
        <Text className={targetStatus.canPush ? 'target-repo-status target-repo-status-reset' : 'target-repo-status target-repo-status-warning'} role="status">
          {targetStatus.fullName} exists{targetStatus.defaultBranch ? ` · ${targetStatus.defaultBranch}` : ''}.{' '}
          {targetStatus.canPush
            ? targetStatus.canExactHydrate
              ? 'Choose Exact hydrate to recreate it, or Hydrate into existing to keep it.'
              : 'Hydrate into existing is available. Exact hydrate needs repository admin rights and delete_repo scope.'
            : 'Your token does not report push access, so reset/hydrate may fail.'}
        </Text>
      ) : null}
      {!targetChecking && targetError ? (
        <Text className="target-repo-status target-repo-status-warning" role="alert">
          {targetError}
        </Text>
      ) : null}
    </Box>
  )
}


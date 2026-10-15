import {Box, Button, Label, Text, TextInput} from '@primer/react'
import {open as openDialog} from '@tauri-apps/plugin-dialog'
import {FileText, FolderGit2, ListChecks, TriangleAlert} from 'lucide-react'
import React from 'react'
import {CLOSE_TRANSIENT_POPUPS_EVENT, closeTransientPopups, hasTauriBridge, invoke} from './app-runtime'
import {dropdownRectFor, dropdownStyle} from './dropdown'
import {FolderGitIcon, RowsIcon} from './icons'
import {PackBrowser, PaneFooter} from './pack-browser'
import {isRepositorySource, normalizedRepositoryParts, repoNameFromRepositorySource} from './repository-source'
import {readLastCollectionSource, readLastSelectedOwner, saveLastSelectedOwner} from './storage'
import type {DropdownRect, GithubAuthStatus, GitHubRepositoryListItem, GitHubRepositoryOwner, GitHubRepositorySuggestion} from './types'
export function HomePage({
  repoSource,
  targetRepo,
  loading,
  error,
  onLoadCollection,
  colorMode,
  githubStatus,
  onToggleColorMode,
  onOpenSettings,
}: {
  repoSource: string
  targetRepo: string
  loading: boolean
  error: string | null
  onLoadCollection: (repoSource: string, targetRepo: string) => void
  colorMode: 'day' | 'night'
  githubStatus: GithubAuthStatus | null
  onToggleColorMode: () => void
  onOpenSettings: () => void
}) {
  const initialCollectionSource = React.useMemo(() => readLastCollectionSource(), [])
  const initialRepoSource = initialCollectionSource?.source ?? repoSource
  const initialRepoParts = normalizedRepositoryParts(initialRepoSource)
  const [source, setSource] = React.useState(() => initialRepoParts?.repo ?? initialRepoSource)
  const [sourceKind, setSourceKind] = React.useState<'repo' | 'folder'>(() => initialCollectionSource?.kind ?? (initialRepoParts ? 'repo' : 'folder'))
  const [pickingFolder, setPickingFolder] = React.useState(false)
  const [repositoryOptions, setRepositoryOptions] = React.useState<GitHubRepositoryListItem[]>([])
  const [repositoriesLoading, setRepositoriesLoading] = React.useState(false)
  const [repoError, setRepoError] = React.useState<string | null>(null)
  const [packCheckLoading, setPackCheckLoading] = React.useState(false)
  const [packCheckError, setPackCheckError] = React.useState<string | null>(null)
  const [selectedRepositoryPack, setSelectedRepositoryPack] = React.useState<GitHubRepositorySuggestion | null>(null)
  const [repoOwners, setRepoOwners] = React.useState<GitHubRepositoryOwner[]>([])
  const [repoOwnersLoading, setRepoOwnersLoading] = React.useState(false)
  const [selectedRepoOwner, setSelectedRepoOwner] = React.useState(() => initialRepoParts?.owner ?? readLastSelectedOwner() ?? 'sethjuarez')
  const [ownerFilter, setOwnerFilter] = React.useState(() => initialRepoParts?.owner ?? readLastSelectedOwner() ?? 'sethjuarez')
  const [ownerPickerOpen, setOwnerPickerOpen] = React.useState(false)
  const [repoPickerOpen, setRepoPickerOpen] = React.useState(false)
  const [selectedRepository, setSelectedRepository] = React.useState<string | null>(() => initialRepoParts ? `${initialRepoParts.owner}/${initialRepoParts.repo}` : null)
  const ownerPickerRef = React.useRef<HTMLElement | null>(null)
  const repoPickerRef = React.useRef<HTMLElement | null>(null)
  const [ownerDropdownRect, setOwnerDropdownRect] = React.useState<DropdownRect | null>(null)
  const [repoDropdownRect, setRepoDropdownRect] = React.useState<DropdownRect | null>(null)

  React.useEffect(() => {
    const close = () => {
      setOwnerPickerOpen(false)
      setRepoPickerOpen(false)
    }
    window.addEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
    return () => window.removeEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
  }, [])

  const positionOwnerDropdown = React.useCallback(() => {
    setOwnerDropdownRect(dropdownRectFor(ownerPickerRef.current))
  }, [])

  const positionRepoDropdown = React.useCallback(() => {
    setRepoDropdownRect(dropdownRectFor(repoPickerRef.current))
  }, [])

  const submit = React.useCallback((event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (sourceKind === 'repo') {
      if (!selectedRepositoryPack) return
      onLoadCollection(selectedRepositoryPack.fullName, targetRepo)
      return
    }
    onLoadCollection(source, targetRepo)
  }, [onLoadCollection, selectedRepositoryPack, source, sourceKind, targetRepo])

  const pickLocalFolder = React.useCallback(() => {
    if (!hasTauriBridge() || pickingFolder) return
    setPickingFolder(true)
    openDialog({
      directory: true,
      multiple: false,
      title: 'Choose pack or collection folder',
    })
      .then(selected => {
        if (typeof selected === 'string' && selected.trim()) setSource(selected)
      })
      .catch(error => {
        console.warn('Local folder picker failed', error)
      })
      .finally(() => setPickingFolder(false))
  }, [pickingFolder])

  const selectSourceKind = React.useCallback((kind: 'repo' | 'folder') => {
    setSourceKind(kind)
    setRepoError(null)
    setPackCheckError(null)
    setSelectedRepositoryPack(null)
    setOwnerPickerOpen(false)
    setRepoPickerOpen(false)
    setSelectedRepository(null)
    setSource(current => {
      if (kind === 'folder') return '.'
      if (kind === 'repo' && !isRepositorySource(current)) return ''
      return current
    })
  }, [])

  React.useEffect(() => {
    if (sourceKind !== 'repo' || repoOwners.length > 0 || repoOwnersLoading) return
    setRepoOwnersLoading(true)
    invoke<GitHubRepositoryOwner[]>('list_github_repository_owners')
      .then(owners => {
        setRepoOwners(owners)
        if (!owners.some(owner => owner.login === selectedRepoOwner) && owners[0]) {
          const userOwner = owners.find(owner => owner.kind === 'User') ?? owners[0]
          setSelectedRepoOwner(userOwner.login)
          setOwnerFilter(userOwner.login)
        }
      })
      .catch((cause: unknown) => {
        setRepoError(cause instanceof Error ? cause.message : String(cause))
      })
      .finally(() => setRepoOwnersLoading(false))
  }, [repoOwners.length, repoOwnersLoading, selectedRepoOwner, sourceKind])

  React.useEffect(() => {
    if (sourceKind !== 'repo' || !selectedRepoOwner) return
    saveLastSelectedOwner(selectedRepoOwner)
    setRepositoriesLoading(true)
    setRepoError(null)
    setRepositoryOptions([])
    setSelectedRepository(current => current?.startsWith(`${selectedRepoOwner}/`) ? current : null)
    setSelectedRepositoryPack(null)
    setPackCheckError(null)
    invoke<GitHubRepositoryListItem[]>('list_github_owner_repositories', {owner: selectedRepoOwner})
      .then(repositories => setRepositoryOptions(repositories))
      .catch((cause: unknown) => {
        setRepositoryOptions([])
        setRepoError(cause instanceof Error ? cause.message : String(cause))
      })
      .finally(() => setRepositoriesLoading(false))
  }, [selectedRepoOwner, sourceKind])

  React.useEffect(() => {
    if (sourceKind !== 'repo' || !selectedRepository) return
    const repo = repoNameFromRepositorySource(selectedRepository) ?? source
    setPackCheckLoading(true)
    setPackCheckError(null)
    setSelectedRepositoryPack(null)
    invoke<GitHubRepositorySuggestion>('check_github_repository_packs', {owner: selectedRepoOwner, repo})
      .then(result => setSelectedRepositoryPack(result))
      .catch((cause: unknown) => {
        setSelectedRepositoryPack(null)
        setPackCheckError(cause instanceof Error ? cause.message : String(cause))
      })
      .finally(() => setPackCheckLoading(false))
  }, [selectedRepoOwner, selectedRepository, source, sourceKind])

  const filteredRepoOwners = React.useMemo(() => {
    const query = ownerFilter.trim().toLowerCase()
    return repoOwners
      .filter(owner => !query || owner.login.toLowerCase().includes(query) || owner.kind.toLowerCase().includes(query))
      .slice(0, 12)
  }, [ownerFilter, repoOwners])

  const filteredRepositories = React.useMemo(() => {
    const query = source.trim().toLowerCase()
    return repositoryOptions
      .filter(repository =>
        !query ||
        repository.name.toLowerCase().includes(query) ||
        repository.fullName.toLowerCase().includes(query) ||
        Boolean(repository.description?.toLowerCase().includes(query)),
      )
      .slice(0, 24)
  }, [repositoryOptions, source])

  const repoOpenDisabled =
    sourceKind === 'repo' &&
    (!selectedRepository || packCheckLoading || !selectedRepositoryPack || Boolean(packCheckError))

  return (
    <Box className="home-workbench">
      <Box as="aside" className="home-sidebar pack-browser">
        <Box className="home-lane-list" aria-label="Pack lanes">
          <button type="button" className="home-lane-item selected" aria-current="page">
            <ListChecks size={16} />
            <span>Use packs</span>
          </button>
          <button type="button" className="home-lane-item" disabled title="Coming next">
            <FileText size={16} />
            <span>Edit packs</span>
          </button>
        </Box>
        <PaneFooter
          collapsed={false}
          colorMode={colorMode}
          githubStatus={githubStatus}
          onToggleColorMode={onToggleColorMode}
          onOpenSettings={onOpenSettings}
        />
      </Box>
      <Box className="home-shell">
        <Box className="lane-grid">
          <Box className="lane-card lane-card-primary">
            <Box className="lane-card-header">
              <Box>
                <Label variant="accent">Consumption lane</Label>
                <Box as="h1" sx={{fontSize: 3, fontWeight: 600, mt: 2, mb: 0}}>
                  Open a pack collection
                </Box>
              </Box>
            </Box>

            <Box as="form" className="collection-form" onSubmit={submit} autoComplete="off">
            <Box>
              <Box className="source-picker-heading">
                <Text as="label" htmlFor="home-source" sx={{display: 'block', fontSize: 0, fontWeight: 600}}>
                  Collection source
                </Text>
                <Box className="source-kind-toggle" role="radiogroup" aria-label="Collection source type">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={sourceKind === 'repo'}
                    className={sourceKind === 'repo' ? 'selected' : undefined}
                    onClick={() => selectSourceKind('repo')}
                  >
                    Repo
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={sourceKind === 'folder'}
                    className={sourceKind === 'folder' ? 'selected' : undefined}
                    onClick={() => selectSourceKind('folder')}
                  >
                    Folder
                  </button>
                </Box>
              </Box>
              <Box className={`source-picker-row source-picker-row-${sourceKind}`}>
                {sourceKind === 'repo' ? (
                  <Box ref={ownerPickerRef} className="source-owner-combobox">
                    <TextInput
                      value={ownerFilter}
                      onChange={(event: React.ChangeEvent<HTMLInputElement>) => {
                        closeTransientPopups()
                        setOwnerFilter(event.target.value)
                        setOwnerPickerOpen(true)
                        setRepoPickerOpen(false)
                        positionOwnerDropdown()
                      }}
                      onFocus={() => {
                        closeTransientPopups()
                        setOwnerPickerOpen(true)
                        setRepoPickerOpen(false)
                        positionOwnerDropdown()
                      }}
                      onClick={() => {
                        closeTransientPopups()
                        setOwnerPickerOpen(true)
                        setRepoPickerOpen(false)
                        positionOwnerDropdown()
                      }}
                      onBlur={() => window.setTimeout(() => setOwnerPickerOpen(false), 120)}
                      autoComplete="off"
                      autoCorrect="off"
                      spellCheck={false}
                      aria-label="Filter GitHub owners and organizations"
                      aria-expanded={ownerPickerOpen}
                      aria-controls="source-owner-results"
                      placeholder={repoOwnersLoading ? 'Loading orgs...' : 'Filter orgs'}
                      sx={{width: '100%'}}
                    />
                    {ownerPickerOpen && filteredRepoOwners.length > 0 ? (
                      <Box id="source-owner-results" className="source-owner-results" role="listbox" style={dropdownStyle(ownerDropdownRect)}>
                        {filteredRepoOwners.map(owner => (
                          <button
                            key={owner.login}
                            type="button"
                            className={owner.login === selectedRepoOwner ? 'source-owner-result selected' : 'source-owner-result'}
                            onMouseDown={event => event.preventDefault()}
                            onClick={() => {
                              setSelectedRepoOwner(owner.login)
                              setOwnerFilter(owner.login)
                              setOwnerPickerOpen(false)
                              setSource('')
                              setRepoPickerOpen(false)
                              setSelectedRepository(null)
                              setSelectedRepositoryPack(null)
                              setPackCheckError(null)
                            }}
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
                ) : null}
                <Box ref={repoPickerRef} className={sourceKind === 'repo' ? 'source-repo-combobox' : undefined}>
                  <TextInput
                    id="home-source"
                    value={source}
                    onChange={(event: React.ChangeEvent<HTMLInputElement>) => {
                      setSource(event.target.value)
                      setSelectedRepository(null)
                      setSelectedRepositoryPack(null)
                      setPackCheckError(null)
                      if (sourceKind === 'repo') {
                        closeTransientPopups()
                        setRepoPickerOpen(true)
                        setOwnerPickerOpen(false)
                        positionRepoDropdown()
                      }
                    }}
                    onFocus={() => {
                      if (sourceKind === 'repo') {
                        closeTransientPopups()
                        setRepoPickerOpen(true)
                        setOwnerPickerOpen(false)
                        positionRepoDropdown()
                      }
                    }}
                    onClick={() => {
                      if (sourceKind === 'repo') {
                        closeTransientPopups()
                        setRepoPickerOpen(true)
                        setOwnerPickerOpen(false)
                        positionRepoDropdown()
                      }
                    }}
                    onBlur={() => window.setTimeout(() => setRepoPickerOpen(false), 120)}
                    leadingVisual={FolderGitIcon}
                    autoComplete="off"
                    autoCorrect="off"
                    spellCheck={false}
                    aria-label={sourceKind === 'repo' ? 'Filter repositories' : 'Local pack collection folder'}
                    aria-expanded={sourceKind === 'repo' ? repoPickerOpen : undefined}
                    aria-controls={sourceKind === 'repo' ? 'source-repo-results' : undefined}
                    placeholder={sourceKind === 'repo' ? 'Filter repositories' : '., current, or C:\\path\\to\\repo'}
                    sx={{width: '100%', fontFamily: 'mono'}}
                  />
                  {sourceKind === 'repo' && repoPickerOpen ? (
                    <Box id="source-repo-results" className="repo-search-results repo-search-dropdown" role="listbox" aria-label="GitHub repositories" style={dropdownStyle(repoDropdownRect)}>
                      {repositoriesLoading ? (
                        <Box className="repo-search-empty" role="status">
                          Loading repositories...
                        </Box>
                      ) : null}
                      {!repositoriesLoading && filteredRepositories.length === 0 ? (
                        <Box className="repo-search-empty" role="status">
                          No repositories found for this filter.
                        </Box>
                      ) : null}
                      {filteredRepositories.map(repository => (
                        <button
                          key={repository.fullName}
                          type="button"
                          className={repository.fullName === selectedRepository ? 'repo-search-result selected' : 'repo-search-result'}
                          onMouseDown={event => event.preventDefault()}
                          onClick={() => {
                            setSelectedRepoOwner(repository.fullName.split('/')[0] ?? selectedRepoOwner)
                            setOwnerFilter(repository.fullName.split('/')[0] ?? selectedRepoOwner)
                            setSource(repository.name)
                            setSelectedRepository(repository.fullName)
                            setPackCheckError(null)
                            setSelectedRepositoryPack(null)
                            setRepoPickerOpen(false)
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
                {sourceKind === 'folder' ? (
                  <button
                    type="button"
                    className="source-picker-button"
                    onClick={pickLocalFolder}
                    disabled={pickingFolder || !hasTauriBridge()}
                    aria-label="Browse for local pack collection folder"
                    title="Browse for local pack collection folder"
                  >
                    <FolderGit2 size={16} />
                    <span>{pickingFolder ? 'Opening...' : 'Browse'}</span>
                  </button>
                ) : null}
              </Box>
              <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0, mt: 1}}>
                {sourceKind === 'repo'
                  ? 'Choose an organization, then choose a repository that contains pack manifests.'
                  : 'Browse to a local pack folder or collection folder, or enter a path manually.'}
              </Text>
                {sourceKind === 'repo' && repoError ? (
                <Box className="pane-message pane-message-danger source-search-message">
                  <TriangleAlert size={16} />
                    <Text sx={{fontSize: 0}}>{repoError}</Text>
                </Box>
              ) : null}
                {sourceKind === 'repo' && packCheckLoading ? (
                  <Box className="repo-selection-status" role="status">
                    Checking selected repository for packs...
                  </Box>
                ) : null}
                {sourceKind === 'repo' && selectedRepositoryPack ? (
                  <Box className="repo-selection-status repo-selection-status-success" role="status">
                    {selectedRepositoryPack.packCount} pack {selectedRepositoryPack.packCount === 1 ? 'manifest' : 'manifests'} found ·{' '}
                    {selectedRepositoryPack.packPaths.slice(0, 2).join(', ')}
                  </Box>
                ) : null}
                {sourceKind === 'repo' && packCheckError ? (
                  <Box className="pane-message pane-message-danger source-search-message">
                    <TriangleAlert size={16} />
                    <Text sx={{fontSize: 0}}>{packCheckError}</Text>
                  </Box>
                ) : null}
            </Box>
            {error ? (
              <Box className="pane-message pane-message-danger">
                <TriangleAlert size={16} />
                <Text sx={{fontSize: 0}}>{error}</Text>
              </Box>
            ) : null}
            <Button type="submit" variant="primary" leadingVisual={RowsIcon} disabled={loading || repoOpenDisabled}>
              {loading ? 'Loading collection...' : 'Open collection'}
            </Button>
          </Box>
        </Box>
      </Box>
      </Box>
    </Box>
  )
}


import './styles.css'

import {initAuditaur, type AuditaurClient} from '@auditaur/api'
import {BaseStyles, Box, Button, Heading, Label, PageLayout, Text, TextInput, ThemeProvider} from '@primer/react'
import {invoke as rawInvoke} from '@tauri-apps/api/core'
import {listen} from '@tauri-apps/api/event'
import {open as openDialog} from '@tauri-apps/plugin-dialog'
import {getCurrentWindow} from '@tauri-apps/api/window'
import {
  CheckCircle,
  CircleDot,
  Copy,
  FileText,
  FolderGit2,
  GitBranch,
  GitPullRequest,
  Github,
  KeyRound,
  ListChecks,
  MessageCircle,
  Maximize2,
  Minimize2,
  Minus,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Play,
  Rows3,
  Search,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Sun,
  Tag,
  TriangleAlert,
  X,
} from 'lucide-react'
import React from 'react'
import {createRoot} from 'react-dom/client'

type GithubAuthStatus = {
  installed: boolean
  authenticated: boolean
  login: string | null
  avatarUrl: string | null
  errorMessage: string | null
  apiAuthenticated: boolean
  apiLogin: string | null
  apiAvatarUrl: string | null
  apiErrorMessage: string | null
  apiTokenSource: string | null
}

type PackPlanPreview = {
  repo: string
  packId: string
  packName: string
  packDescription: string | null
  allowNonEmpty: boolean
  maxWrites: number
  totalOperations: number
  writeOperations: number
  localWarmups: number
  operations: PlanOperationPreview[]
}

type PackDryRunReport = {
  repo: string
  packId: string
  packName: string
  totalOperations: number
  writeOperations: number
  localWarmups: number
  operations: PlanOperationPreview[]
}

type PackHydrateReport = {
  repo: string
  packId: string
  packName: string
  totalOperations: number
  writeOperations: number
  localWarmups: number
}

type PackHydrateProgress = {
  runId: string
  index: number
  total: number
  id: string
  kind: string
  target: string
  status: 'started' | 'completed' | 'skipped' | 'failed'
}

type OperationProgressState = 'pending' | 'running' | 'done' | 'skipped' | 'failed'
type HydrateMode = 'exact' | 'existing'

const REPOSITORY_SETUP_PROGRESS_INDEX = -1
const REPOSITORY_SETUP_PROGRESS_TARGET = 'Create or verify repository'

type PackListItem = {
  id: string
  name: string
  description: string | null
  source: string
  location: string
  writeOperations: number
  warmups: number
  valid: boolean
  status: string
}

type GitHubRepositorySuggestion = {
  fullName: string
  description: string | null
  private: boolean
  defaultBranch: string
  url: string
  packCount: number
  packPaths: string[]
}

type GitHubRepositoryListItem = {
  name: string
  fullName: string
  description: string | null
  private: boolean
  defaultBranch: string
  url: string
}

type GitHubRepositoryOwner = {
  login: string
  kind: 'User' | 'Organization'
  avatarUrl: string | null
}

type GitHubTargetRepositoryStatus = {
  owner: string
  name: string
  fullName: string
  exists: boolean
  private: boolean | null
  defaultBranch: string | null
  url: string | null
  canPush: boolean
  canAdmin: boolean
  canExactHydrate: boolean
}

type DropdownRect = {
  top: number
  left: number
  width: number
}

type LastCollectionSource = {
  source: string
  kind: 'repo' | 'folder'
}

type PlanOperationPreview = {
  index: number
  kind: string
  id: string
  target: string
  details: string[]
  writesToGithub: boolean
  contentPreview: OperationContentPreview | null
}

type OperationContentPreview = {
  title: string
  subtitle: string
  body: string
  format: 'markdown' | 'code' | 'json' | 'text'
}

let auditaurPromise: Promise<AuditaurClient | null> | null = null

function hasTauriBridge() {
  return '__TAURI_INTERNALS__' in window || '__TAURI__' in window
}

function initializeAuditaur() {
  if (!hasTauriBridge()) return Promise.resolve(null)

  if (!auditaurPromise) {
    auditaurPromise = initAuditaur({
      serviceName: 'autorepo-frontend',
      instrumentConsole: true,
      instrumentErrors: true,
      instrumentTauriInvoke: true,
      instrumentTauriEvents: true,
      captureFullPayloads: false,
      driveBridge: {windowLabel: 'main'},
      onExportError(failure) {
        console.warn('Auditaur export failed', failure.error)
      },
    }).catch(error => {
      console.warn('Auditaur initialization failed', error)
      return null
    })
  }

  return auditaurPromise
}

async function invoke<T>(command: string, args?: Record<string, unknown>) {
  const auditaur = await initializeAuditaur()
  return auditaur ? auditaur.invoke<T>(command, args) : rawInvoke<T>(command, args)
}

const CLOSE_TRANSIENT_POPUPS_EVENT = 'autorepo:close-transient-popups'

function closeTransientPopups() {
  window.dispatchEvent(new Event(CLOSE_TRANSIENT_POPUPS_EVENT))
}

function App() {
  const [settingsOpen, setSettingsOpen] = React.useState(false)
  const [colorMode, setColorMode] = React.useState<'day' | 'night'>('day')
  const [githubStatus, setGithubStatus] = React.useState<GithubAuthStatus | null>(null)
  const [packPaneWidth, setPackPaneWidth] = React.useState(280)
  const [packPaneCollapsed, setPackPaneCollapsed] = React.useState(false)
  const [repoSource, setRepoSource] = React.useState(() => readLastCollectionSource()?.source ?? '.')
  const [targetRepo, setTargetRepo] = React.useState('sethjuarez/fictional-pancake')
  const [packs, setPacks] = React.useState<PackListItem[]>([])
  const [selectedPackSource, setSelectedPackSource] = React.useState('builtin')
  const [mode, setMode] = React.useState<'home' | 'consume'>('home')
  const [loadingCollection, setLoadingCollection] = React.useState(false)
  const [collectionError, setCollectionError] = React.useState<string | null>(null)
  const selectedPack = React.useMemo(
    () => packs.find(pack => pack.source === selectedPackSource) ?? null,
    [packs, selectedPackSource],
  )
  const refreshGithubStatus = React.useCallback(() => {
    invoke<GithubAuthStatus>('github_auth_status')
      .then(setGithubStatus)
      .catch(() => setGithubStatus(null))
  }, [])

  React.useEffect(() => {
    if (hasTauriBridge()) {
      void initializeAuditaur().then(client => {
        if (client) console.info('autorepo.home.ready')
      })
      return
    }

    const timer = window.setInterval(() => {
      if (!hasTauriBridge()) return
      window.clearInterval(timer)
      void initializeAuditaur().then(client => {
        if (client) console.info('autorepo.home.ready')
      })
    }, 100)

    return () => window.clearInterval(timer)
  }, [])

  React.useEffect(() => {
    refreshGithubStatus()
  }, [refreshGithubStatus])

  const handlePacksLoaded = React.useCallback((loadedPacks: PackListItem[]) => {
    const uniquePacks = dedupePacks(loadedPacks)
    setPacks(uniquePacks)
    setSelectedPackSource(current => {
      if (uniquePacks.some(pack => pack.source === current)) return current
      return uniquePacks[0]?.source ?? 'builtin'
    })
  }, [])

  const loadCollection = React.useCallback((source: string, target: string) => {
    setRepoSource(source)
    setTargetRepo(target)
    setLoadingCollection(true)
    setCollectionError(null)
    invoke<PackListItem[]>('list_repo_packs', {repoSource: source})
      .then(loadedPacks => {
        saveLastCollectionSource(source)
        handlePacksLoaded(loadedPacks)
        setMode('consume')
      })
      .catch((cause: unknown) => {
        setPacks([])
        setCollectionError(cause instanceof Error ? cause.message : String(cause))
      })
      .finally(() => setLoadingCollection(false))
  }, [handlePacksLoaded])

  return (
    <ThemeProvider colorMode={colorMode} dayScheme="light" nightScheme="dark">
      <BaseStyles>
        <div className="app-shell" data-theme-mode={colorMode}>
          <TitleBar
          />
          <Box className="app-content">
            <PageLayout containerWidth="full" padding="none">
              <PageLayout.Content width="full" sx={{p: 0}}>
                {mode === 'home' ? (
                  <HomePage
                    repoSource={repoSource}
                    targetRepo={targetRepo}
                    loading={loadingCollection}
                    error={collectionError}
                    onLoadCollection={loadCollection}
                    colorMode={colorMode}
                    githubStatus={githubStatus}
                    onToggleColorMode={() => setColorMode(mode => (mode === 'day' ? 'night' : 'day'))}
                    onOpenSettings={() => setSettingsOpen(true)}
                  />
                ) : (
                  <Box
                    className={packPaneCollapsed ? 'pack-workbench pack-workbench-collapsed' : 'pack-workbench'}
                    style={{'--pack-pane-width': `${packPaneCollapsed ? 52 : packPaneWidth}px`} as React.CSSProperties}
                  >
                    <PackBrowser
                      packs={packs}
                      selectedPackSource={selectedPackSource}
                      onSelectPack={setSelectedPackSource}
                      onChangeCollection={() => setMode('home')}
                      collapsed={packPaneCollapsed}
                      onToggleCollapsed={() => setPackPaneCollapsed(collapsed => !collapsed)}
                      colorMode={colorMode}
                      githubStatus={githubStatus}
                      onToggleColorMode={() => setColorMode(mode => (mode === 'day' ? 'night' : 'day'))}
                      onOpenSettings={() => setSettingsOpen(true)}
                      onResize={setPackPaneWidth}
                    />
                    <PlanPreviewPanel
                      packSource={selectedPackSource}
                      selectedPack={selectedPack}
                      repoSource={repoSource}
                      repo={targetRepo}
                      onRepoChange={setTargetRepo}
                    />
                  </Box>
                )}
              </PageLayout.Content>
            </PageLayout>
          </Box>
          {settingsOpen ? (
            <SettingsLightbox
              onClose={() => setSettingsOpen(false)}
              onStatusChange={setGithubStatus}
            />
          ) : null}
        </div>
      </BaseStyles>
    </ThemeProvider>
  )
}

function HomePage({
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

function PackBrowser({
  packs,
  selectedPackSource,
  onSelectPack,
  onChangeCollection,
  collapsed,
  onToggleCollapsed,
  colorMode,
  githubStatus,
  onToggleColorMode,
  onOpenSettings,
  onResize,
}: {
  packs: PackListItem[]
  selectedPackSource: string
  onSelectPack: (value: string) => void
  onChangeCollection: () => void
  collapsed: boolean
  onToggleCollapsed: () => void
  colorMode: 'day' | 'night'
  githubStatus: GithubAuthStatus | null
  onToggleColorMode: () => void
  onOpenSettings: () => void
  onResize: (value: number) => void
}) {
  const [query, setQuery] = React.useState('')
  const [searchOpen, setSearchOpen] = React.useState(false)
  const searchInputRef = React.useRef<HTMLInputElement | null>(null)
  const searchResults = React.useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return packs
    return packs.filter(pack =>
      [pack.name, pack.description, pack.location, pack.status].some(value => value?.toLowerCase().includes(normalized)),
    )
  }, [packs, query])
  const resizing = React.useRef(false)

  React.useEffect(() => {
    if (!searchOpen) return
    const frame = window.requestAnimationFrame(() => searchInputRef.current?.focus())
    return () => window.cancelAnimationFrame(frame)
  }, [searchOpen])

  React.useEffect(() => {
    const close = () => setSearchOpen(false)
    window.addEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
    return () => window.removeEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
  }, [])

  React.useEffect(() => {
    if (collapsed) return
    const handleMove = (event: PointerEvent) => {
      if (!resizing.current) return
      onResize(Math.min(420, Math.max(220, event.clientX)))
    }
    const handleUp = () => {
      resizing.current = false
      document.body.classList.remove('is-resizing-pane')
    }
    window.addEventListener('pointermove', handleMove)
    window.addEventListener('pointerup', handleUp)
    return () => {
      window.removeEventListener('pointermove', handleMove)
      window.removeEventListener('pointerup', handleUp)
    }
  }, [collapsed, onResize])

  return (
    <Box as="aside" className={collapsed ? 'pack-browser pack-browser-collapsed' : 'pack-browser'}>
      <Box className="pane-header">
        <Box className="pane-header-main">
          <button
            type="button"
            className="pane-icon-button"
            aria-label={collapsed ? 'Show pack browser' : 'Hide pack browser'}
            title={collapsed ? 'Show pack browser' : 'Hide pack browser'}
            onClick={onToggleCollapsed}
          >
            {collapsed ? <PanelLeftOpen size={17} /> : <PanelLeftClose size={17} />}
          </button>
          {!collapsed ? (
            <Box className="pane-context-actions" aria-label="Pack actions">
              <button
                type="button"
                className="pane-icon-button"
                aria-label="Search packs"
                title="Search packs"
                onClick={() => {
                  closeTransientPopups()
                  setSearchOpen(true)
                }}
              >
                <Search size={16} />
              </button>
              <button
                type="button"
                className="pane-icon-button"
                aria-label="Change collection"
                title="Change collection"
                onClick={onChangeCollection}
              >
                <FolderGit2 size={16} />
              </button>
            </Box>
          ) : null}
        </Box>
      </Box>

      {!collapsed ? (
        <Box className="pack-list" role="listbox" aria-label="Repository packs">
        {packs.map(pack => (
          <button
            key={pack.source}
            type="button"
            className={pack.source === selectedPackSource ? 'pack-list-item selected' : 'pack-list-item'}
            onClick={() => onSelectPack(pack.source)}
            aria-selected={pack.source === selectedPackSource}
          >
            <Box className="pack-list-body">
              <Box className="pack-list-title">
                <span>{pack.name}</span>
                <Label variant={pack.valid ? 'success' : 'danger'}>{pack.status}</Label>
              </Box>
              <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0, fontFamily: 'mono'}}>
                {pack.location}
              </Text>
              <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0}}>
                {pack.writeOperations} writes · {pack.warmups} warmups
              </Text>
            </Box>
          </button>
        ))}
        </Box>
      ) : null}
      {searchOpen ? (
        <Box className="pack-search-scrim" role="presentation" onClick={() => setSearchOpen(false)}>
          <Box
            className="pack-search-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="Search packs"
            onClick={(event: React.MouseEvent) => event.stopPropagation()}
          >
            <Box className="pack-search-field">
              <Search size={18} />
              <input
                ref={searchInputRef}
                value={query}
                onChange={event => setQuery(event.target.value)}
                onKeyDown={(event: React.KeyboardEvent<HTMLInputElement>) => {
                  if (event.key === 'Escape') setSearchOpen(false)
                }}
                placeholder="Search packs..."
                aria-label="Search packs"
              />
              <button type="button" className="pane-icon-button" aria-label="Close search" title="Close search" onClick={() => setSearchOpen(false)}>
                <X size={16} />
              </button>
            </Box>
            <Box className="pack-search-results" role="listbox" aria-label="Pack search results">
              {searchResults.map(pack => (
                <button
                  key={pack.source}
                  type="button"
                  className={pack.source === selectedPackSource ? 'pack-search-result selected' : 'pack-search-result'}
                  onClick={() => {
                    onSelectPack(pack.source)
                    setSearchOpen(false)
                  }}
                >
                  <Box>
                    <Text sx={{fontWeight: 600}}>{pack.name}</Text>
                    <Text sx={{display: 'block', color: 'fg.muted', fontSize: 0}}>{pack.location}</Text>
                  </Box>
                  <Label variant={pack.valid ? 'success' : 'danger'}>{pack.status}</Label>
                </button>
              ))}
              {searchResults.length === 0 ? (
                <Text sx={{display: 'block', color: 'fg.muted', fontSize: 1, px: 3, py: 2}}>No packs found.</Text>
              ) : null}
            </Box>
          </Box>
        </Box>
      ) : null}
      <PaneFooter
        collapsed={collapsed}
        colorMode={colorMode}
        githubStatus={githubStatus}
        onToggleColorMode={onToggleColorMode}
        onOpenSettings={onOpenSettings}
      />
      {!collapsed ? (
        <div
          className="pane-resizer"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize pack browser"
          onPointerDown={event => {
            event.preventDefault()
            resizing.current = true
            document.body.classList.add('is-resizing-pane')
          }}
        />
      ) : null}
    </Box>
  )
}

function PaneFooter({
  collapsed,
  colorMode,
  githubStatus,
  onToggleColorMode,
  onOpenSettings,
}: {
  collapsed: boolean
  colorMode: 'day' | 'night'
  githubStatus: GithubAuthStatus | null
  onToggleColorMode: () => void
  onOpenSettings: () => void
}) {
  const [menuOpen, setMenuOpen] = React.useState(false)
  const menuRef = React.useRef<HTMLElement | null>(null)
  const currentUser = React.useMemo(() => {
    const login = githubStatus?.apiLogin ?? githubStatus?.login
    if (!login) return null
    return {
      login,
      displayName: login === 'sethjuarez' ? 'Seth Juarez' : login,
      initials: login.slice(0, 1).toUpperCase(),
      avatarUrl: githubStatus?.apiAvatarUrl ?? githubStatus?.avatarUrl,
    }
  }, [githubStatus])
  const githubConnected = Boolean(githubStatus?.apiAuthenticated || githubStatus?.authenticated)
  const connectionText = githubConnected
    ? githubStatus?.apiTokenSource
      ? `Connected with ${githubStatus.apiTokenSource}`
      : 'GitHub connected'
    : 'GitHub not connected'
  const openSettings = React.useCallback(() => {
    setMenuOpen(false)
    onOpenSettings()
  }, [onOpenSettings])
  const toggleTheme = React.useCallback(() => {
    setMenuOpen(false)
    onToggleColorMode()
  }, [onToggleColorMode])
  const openFeedback = React.useCallback(() => {
    setMenuOpen(false)
    window.open('https://github.com/sethjuarez/autorepo/issues/new', '_blank', 'noopener,noreferrer')
  }, [])
  const openProject = React.useCallback(() => {
    setMenuOpen(false)
    window.open('https://github.com/sethjuarez/autorepo', '_blank', 'noopener,noreferrer')
  }, [])

  React.useEffect(() => {
    if (!menuOpen) return
    const handlePointerDown = (event: PointerEvent) => {
      if (menuRef.current?.contains(event.target as Node)) return
      setMenuOpen(false)
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setMenuOpen(false)
    }
    window.addEventListener('pointerdown', handlePointerDown)
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('pointerdown', handlePointerDown)
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [menuOpen])

  React.useEffect(() => {
    const close = () => setMenuOpen(false)
    window.addEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
    return () => window.removeEventListener(CLOSE_TRANSIENT_POPUPS_EVENT, close)
  }, [])

  if (collapsed) {
    return (
      <Box className="pane-footer pane-footer-collapsed">
        <button
          type="button"
          className="pane-icon-button"
          aria-label={colorMode === 'day' ? 'Switch to dark mode' : 'Switch to light mode'}
          title={colorMode === 'day' ? 'Switch to dark mode' : 'Switch to light mode'}
          onClick={onToggleColorMode}
        >
          {colorMode === 'day' ? <Moon size={16} /> : <Sun size={16} />}
        </button>
        <button
          type="button"
          className="pane-icon-button"
          aria-label="Send feedback"
          title="Send feedback"
          onClick={openFeedback}
        >
          <MessageCircle size={16} />
        </button>
        <button type="button" className="pane-icon-button" aria-label="Open settings" title="Settings" onClick={onOpenSettings}>
          <Settings size={16} />
        </button>
      </Box>
    )
  }

  return (
    <Box ref={menuRef} className="pane-footer">
      <button
        type="button"
        className="pane-current-user"
        title={currentUser ? `Signed in as ${currentUser.login}` : 'GitHub not connected'}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onClick={() => {
          if (!menuOpen) closeTransientPopups()
          setMenuOpen(open => !open)
        }}
      >
        <span className="pane-avatar" aria-hidden="true">
          {currentUser?.avatarUrl ? <img src={currentUser.avatarUrl} alt="" /> : currentUser?.initials ?? '?'}
        </span>
        <span className="pane-current-user-name">{currentUser?.displayName ?? 'Not signed in'}</span>
      </button>
      {menuOpen ? (
        <Box className="pane-account-menu" role="menu" aria-label="User and app menu">
          <Box className="pane-account-card">
            <span className="pane-avatar pane-account-avatar" aria-hidden="true">
              {currentUser?.avatarUrl ? <img src={currentUser.avatarUrl} alt="" /> : currentUser?.initials ?? '?'}
            </span>
            <span>
              <strong>{currentUser ? `@${currentUser.login}` : 'GitHub account'}</strong>
              <small>{connectionText}</small>
            </span>
          </Box>
          <Box className="pane-account-section">
            <button type="button" role="menuitem" onClick={openSettings}>
              <KeyRound size={15} />
              <span>Manage GitHub connection</span>
            </button>
            <button type="button" role="menuitem" onClick={toggleTheme}>
              {colorMode === 'day' ? <Moon size={15} /> : <Sun size={15} />}
              <span>{colorMode === 'day' ? 'Switch to dark mode' : 'Switch to light mode'}</span>
            </button>
          </Box>
          <Box className="pane-account-section">
            <button type="button" role="menuitem" onClick={openSettings}>
              <ShieldCheck size={15} />
              <span>Health check</span>
            </button>
            <button type="button" role="menuitem" onClick={openProject}>
              <Github size={15} />
              <span>About Autorepo</span>
            </button>
            <button type="button" role="menuitem" onClick={openFeedback}>
              <MessageCircle size={15} />
              <span>Send feedback</span>
            </button>
          </Box>
        </Box>
      ) : null}
      <Box className="pane-footer-actions">
        <button
          type="button"
          className="pane-icon-button"
          aria-label={colorMode === 'day' ? 'Switch to dark mode' : 'Switch to light mode'}
          title={colorMode === 'day' ? 'Switch to dark mode' : 'Switch to light mode'}
          onClick={onToggleColorMode}
        >
          {colorMode === 'day' ? <Moon size={16} /> : <Sun size={16} />}
        </button>
        <button
          type="button"
          className="pane-icon-button"
          aria-label="Send feedback"
          title="Send feedback"
          onClick={openFeedback}
        >
          <MessageCircle size={16} />
        </button>
        <button type="button" className="pane-icon-button" aria-label="Open settings" title="Settings" onClick={onOpenSettings}>
          <Settings size={16} />
        </button>
      </Box>
    </Box>
  )
}

function TargetRepositoryPicker({
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

function PlanPreviewPanel({
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
            <Rows3 size={16} />
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

function HydrateConfirmationDialog({
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

function OperationPreviewLightbox({
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

function OperationRow({
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

function dedupePacks(packs: PackListItem[]) {
  const byId = new Map<string, PackListItem>()
  for (const pack of packs) {
    const existing = byId.get(pack.id)
    if (!existing || (!existing.valid && pack.valid)) {
      byId.set(pack.id, pack)
    }
  }
  return [...byId.values()]
}

function hydrateProgressStatus(status: PackHydrateProgress['status']): OperationProgressState {
  if (status === 'started') return 'running'
  if (status === 'completed') return 'done'
  if (status === 'skipped') return 'skipped'
  return 'failed'
}

function pendingWriteProgress(operations: PlanOperationPreview[]) {
  const progress: Record<number, OperationProgressState> = {
    [REPOSITORY_SETUP_PROGRESS_INDEX]: 'running',
  }
  for (const operation of operations) {
    if (operation.writesToGithub) progress[operation.index] = 'pending'
  }
  return progress
}

function completeRemainingProgress(
  operations: PlanOperationPreview[],
  current: Record<number, OperationProgressState>,
) {
  const progress: Record<number, OperationProgressState> = {
    [REPOSITORY_SETUP_PROGRESS_INDEX]: 'done',
  }
  for (const operation of operations) {
    if (!operation.writesToGithub) continue
    const state = current[operation.index]
    progress[operation.index] = state === 'failed' ? state : 'done'
  }
  return progress
}

function newHydrateRunId() {
  return window.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
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

function isRepositorySource(value: string) {
  const trimmed = value.trim()
  return /^[^\\/:]+\/[^\\/:]+$/.test(trimmed) || trimmed.startsWith('github:') || trimmed.startsWith('https://github.com/')
}

function ownerFromRepositorySource(value: string) {
  const normalized = normalizedRepositoryParts(value)
  return normalized?.owner ?? null
}

function repoNameFromRepositorySource(value: string) {
  const normalized = normalizedRepositoryParts(value)
  return normalized?.repo ?? null
}

function readLastSelectedOwner() {
  try {
    return window.localStorage.getItem('autorepo:last-selected-owner')
  } catch {
    return null
  }
}

function saveLastSelectedOwner(owner: string) {
  try {
    window.localStorage.setItem('autorepo:last-selected-owner', owner)
  } catch {
    // Ignore storage failures; the picker still works without persistence.
  }
}

function readLastCollectionSource(): LastCollectionSource | null {
  try {
    const raw = window.localStorage.getItem('autorepo:last-collection-source')
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<LastCollectionSource>
    if (!parsed.source?.trim()) return null
    const kind = parsed.kind === 'repo' || parsed.kind === 'folder' ? parsed.kind : isRepositorySource(parsed.source) ? 'repo' : 'folder'
    return {source: parsed.source, kind}
  } catch {
    return null
  }
}

function saveLastCollectionSource(source: string) {
  const normalized = source.trim()
  if (!normalized) return
  try {
    window.localStorage.setItem(
      'autorepo:last-collection-source',
      JSON.stringify({
        source: normalized,
        kind: isRepositorySource(normalized) ? 'repo' : 'folder',
      } satisfies LastCollectionSource),
    )
  } catch {
    // Ignore storage failures; collection loading still works without persistence.
  }
}

function dropdownRectFor(element: HTMLElement | null): DropdownRect | null {
  if (!element) return null
  const rect = element.getBoundingClientRect()
  return {
    top: rect.bottom + 4,
    left: rect.left,
    width: rect.width,
  }
}

function dropdownStyle(rect: DropdownRect | null): React.CSSProperties | undefined {
  if (!rect) return undefined
  const margin = 16
  const width = Math.min(Math.max(rect.width, 420), window.innerWidth - margin * 2)
  const left = Math.min(Math.max(rect.left, margin), window.innerWidth - width - margin)
  return {
    position: 'fixed',
    top: rect.top,
    left,
    width,
  }
}

function normalizedRepositoryParts(value: string) {
  const trimmed = value.trim()
  const candidate = trimmed.startsWith('github:')
    ? trimmed.slice('github:'.length)
    : trimmed.startsWith('https://github.com/')
      ? trimmed.slice('https://github.com/'.length)
      : trimmed
  const [owner, repo] = candidate.trimEnd().replace(/\/$/, '').split('/')
  if (!owner || !repo) return null
  return {owner, repo}
}

function operationIcon(kind: string) {
  if (kind === 'Label') return TagIcon
  if (kind === 'Milestone') return CircleDotIcon
  if (kind === 'File') return FileTextIcon
  if (kind === 'Branch') return GitBranchIcon
  if (kind === 'Issue') return CircleDotIcon
  if (kind === 'Pull request') return GitPullRequestIcon
  if (kind === 'Workflow dispatch') return PlayIcon
  return ListChecksIcon
}

function operationAction(kind: string) {
  if (kind === 'Label') return 'Create'
  if (kind === 'Milestone') return 'Create'
  if (kind === 'File') return 'Create'
  if (kind === 'Branch') return 'Create'
  if (kind === 'Issue') return 'Open'
  if (kind === 'Pull request') return 'Open'
  if (kind === 'Workflow dispatch') return 'Dispatch'
  if (kind === 'Warmup app link') return 'Open link'
  if (kind === 'Warmup app session') return 'Open session'
  if (kind.toLowerCase().includes('warmup')) return 'Prepare locally'
  return kind
}

function TitleBar() {
  const appWindow = React.useMemo(() => {
    try {
      return getCurrentWindow()
    } catch {
      return null
    }
  }, [])
  const [maximized, setMaximized] = React.useState(false)

  React.useEffect(() => {
    if (!appWindow) return
    void appWindow.isMaximized().then(setMaximized)
    const unlisten = appWindow.onResized(() => {
      void appWindow.isMaximized().then(setMaximized)
    })

    return () => {
      void unlisten.then(dispose => dispose())
    }
  }, [appWindow])

  const handleMouseDown = React.useCallback(
    (event: React.MouseEvent<HTMLElement>) => {
      if (event.button !== 0 || !appWindow) return
      const target = event.target as HTMLElement
      if (target.closest('button, input, textarea, select, a, [role="button"]')) return

      if (event.detail === 2) {
        event.preventDefault()
        void appWindow.toggleMaximize()
        return
      }

      void appWindow.startDragging()
    },
    [appWindow],
  )

  return (
    <Box as="header" className="titlebar" onMouseDown={handleMouseDown}>
      <Box className="titlebar-brand">
        <Box className="titlebar-logo" aria-hidden="true">
          <Github size={18} strokeWidth={2.2} />
        </Box>
        <Box>
          <Text sx={{display: 'block', fontSize: 1, fontWeight: 700, lineHeight: 'condensed'}}>Autorepo</Text>
        </Box>
      </Box>
      <Box className="titlebar-center">
        <Text sx={{color: 'fg.muted', fontSize: 0}}>Preview mode</Text>
      </Box>
      <Box className="titlebar-actions">
        <WindowControl label="Minimize" onClick={() => void appWindow?.minimize()}>
          <Minus size={16} />
        </WindowControl>
        <WindowControl label={maximized ? 'Restore' : 'Maximize'} onClick={() => void appWindow?.toggleMaximize()}>
          {maximized ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
        </WindowControl>
        <WindowControl label="Close" danger onClick={() => void appWindow?.close()}>
          <X size={16} />
        </WindowControl>
      </Box>
    </Box>
  )
}

function WindowControl({
  label,
  danger = false,
  onClick,
  children,
}: {
  label: string
  danger?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      className={danger ? 'window-control window-control-danger' : 'window-control'}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      {children}
    </button>
  )
}

function SettingsLightbox({
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

function KeyIcon(props: {size?: number; className?: string}) {
  return <KeyRound {...props} />
}

function CircleDotIcon(props: {size?: number; className?: string}) {
  return <CircleDot {...props} />
}

function FileTextIcon(props: {size?: number; className?: string}) {
  return <FileText {...props} />
}

function FolderGitIcon(props: {size?: number; className?: string}) {
  return <FolderGit2 {...props} />
}

function GitBranchIcon(props: {size?: number; className?: string}) {
  return <GitBranch {...props} />
}

function GitPullRequestIcon(props: {size?: number; className?: string}) {
  return <GitPullRequest {...props} />
}

function ListChecksIcon(props: {size?: number; className?: string}) {
  return <ListChecks {...props} />
}

function PlayIcon(props: {size?: number; className?: string}) {
  return <Play {...props} />
}

function RowsIcon(props: {size?: number; className?: string}) {
  return <Rows3 {...props} />
}

function ShieldCheckIcon(props: {size?: number; className?: string}) {
  return <ShieldCheck {...props} />
}

function TagIcon(props: {size?: number; className?: string}) {
  return <Tag {...props} />
}

const root = createRoot(document.getElementById('root')!)
root.render(<App />)

if (import.meta.hot) {
  import.meta.hot.dispose(() => root.unmount())
}

export type GithubAuthStatus = {
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

export type PackPlanPreview = {
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

export type PackDryRunReport = {
  repo: string
  packId: string
  packName: string
  totalOperations: number
  writeOperations: number
  localWarmups: number
  operations: PlanOperationPreview[]
}

export type PackHydrateReport = {
  repo: string
  packId: string
  packName: string
  totalOperations: number
  writeOperations: number
  localWarmups: number
}

export type PackHydrateProgress = {
  runId: string
  index: number
  total: number
  id: string
  kind: string
  target: string
  status: 'started' | 'completed' | 'skipped' | 'failed'
}

export type OperationProgressState = 'pending' | 'running' | 'done' | 'skipped' | 'failed'
export type HydrateMode = 'exact' | 'existing'

export type PackListItem = {
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

export type GitHubRepositorySuggestion = {
  fullName: string
  description: string | null
  private: boolean
  defaultBranch: string
  url: string
  packCount: number
  packPaths: string[]
}

export type GitHubRepositoryListItem = {
  name: string
  fullName: string
  description: string | null
  private: boolean
  defaultBranch: string
  url: string
}

export type GitHubRepositoryOwner = {
  login: string
  kind: 'User' | 'Organization'
  avatarUrl: string | null
}

export type GitHubTargetRepositoryStatus = {
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

export type DropdownRect = {
  top: number
  left: number
  width: number
}

export type LastCollectionSource = {
  source: string
  kind: 'repo' | 'folder'
}

export type PlanOperationPreview = {
  index: number
  kind: string
  id: string
  target: string
  details: string[]
  writesToGithub: boolean
  contentPreview: OperationContentPreview | null
}

export type OperationContentPreview = {
  title: string
  subtitle: string
  body: string
  format: 'markdown' | 'code' | 'json' | 'text'
}

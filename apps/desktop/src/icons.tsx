import {
  CircleDot,
  FileText,
  FolderGit2,
  GitBranch,
  GitPullRequest,
  KeyRound,
  ListChecks,
  Play,
  Rows3,
  ShieldCheck,
  Tag,
} from 'lucide-react'

type IconProps = {
  size?: number
  className?: string
}

export function KeyIcon(props: IconProps) {
  return <KeyRound {...props} />
}

export function CircleDotIcon(props: IconProps) {
  return <CircleDot {...props} />
}

export function FileTextIcon(props: IconProps) {
  return <FileText {...props} />
}

export function FolderGitIcon(props: IconProps) {
  return <FolderGit2 {...props} />
}

export function GitBranchIcon(props: IconProps) {
  return <GitBranch {...props} />
}

export function GitPullRequestIcon(props: IconProps) {
  return <GitPullRequest {...props} />
}

export function ListChecksIcon(props: IconProps) {
  return <ListChecks {...props} />
}

export function PlayIcon(props: IconProps) {
  return <Play {...props} />
}

export function RowsIcon(props: IconProps) {
  return <Rows3 {...props} />
}

export function ShieldCheckIcon(props: IconProps) {
  return <ShieldCheck {...props} />
}

export function TagIcon(props: IconProps) {
  return <Tag {...props} />
}

export function operationIcon(kind: string) {
  if (kind === 'Label') return TagIcon
  if (kind === 'Milestone') return CircleDotIcon
  if (kind === 'File') return FileTextIcon
  if (kind === 'Branch') return GitBranchIcon
  if (kind === 'Issue') return CircleDotIcon
  if (kind === 'Pull request') return GitPullRequestIcon
  if (kind === 'Workflow dispatch') return PlayIcon
  return ListChecksIcon
}

export function operationAction(kind: string) {
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

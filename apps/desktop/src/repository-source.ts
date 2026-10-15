export function isRepositorySource(value: string) {
  const trimmed = value.trim()
  return /^[^\\/:]+\/[^\\/:]+$/.test(trimmed) || trimmed.startsWith('github:') || trimmed.startsWith('https://github.com/')
}

export function repoNameFromRepositorySource(value: string) {
  const normalized = normalizedRepositoryParts(value)
  return normalized?.repo ?? null
}

export function normalizedRepositoryParts(value: string) {
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

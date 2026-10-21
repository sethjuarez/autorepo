import './styles.css'

import {BaseStyles, Box, PageLayout, ThemeProvider} from '@primer/react'
import React from 'react'
import {createRoot} from 'react-dom/client'
import {hasTauriBridge, initializeAuditaur, invoke} from './app-runtime'
import {HomePage} from './home-page'
import {PackBrowser} from './pack-browser'
import {PlanPreviewPanel} from './plan-preview-panel'
import {SettingsLightbox} from './settings'
import {readLastCollectionSource, saveLastCollectionSource} from './storage'
import {TitleBar} from './title-bar'
import type {
  GithubAuthStatus,
  PackListItem,
} from './types'

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

const root = createRoot(document.getElementById('root')!)
root.render(<App />)

if (import.meta.hot) {
  import.meta.hot.dispose(() => root.unmount())
}

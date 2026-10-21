import {Box, Label, Text} from '@primer/react'
import {FolderGit2, Github, MessageCircle, Moon, PanelLeftClose, PanelLeftOpen, Search, Settings, ShieldCheck, Sun, X} from 'lucide-react'
import React from 'react'
import {CLOSE_TRANSIENT_POPUPS_EVENT, closeTransientPopups} from './app-runtime'
import {KeyIcon} from './icons'
import type {GithubAuthStatus, PackListItem} from './types'
export function PackBrowser({
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

export function PaneFooter({
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
              <KeyIcon size={15} />
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


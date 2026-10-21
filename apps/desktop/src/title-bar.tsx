import {Box, Text} from '@primer/react'
import {getCurrentWindow} from '@tauri-apps/api/window'
import {Github, Maximize2, Minimize2, Minus, X} from 'lucide-react'
import React from 'react'

export function TitleBar() {
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

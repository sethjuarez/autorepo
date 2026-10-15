import {initAuditaur, type AuditaurClient} from '@auditaur/api'
import {invoke as rawInvoke} from '@tauri-apps/api/core'

let auditaurPromise: Promise<AuditaurClient | null> | null = null

export function hasTauriBridge() {
  return '__TAURI_INTERNALS__' in window || '__TAURI__' in window
}

export function initializeAuditaur() {
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

export async function invoke<T>(command: string, args?: Record<string, unknown>) {
  const auditaur = await initializeAuditaur()
  return auditaur ? auditaur.invoke<T>(command, args) : rawInvoke<T>(command, args)
}

export const CLOSE_TRANSIENT_POPUPS_EVENT = 'autorepo:close-transient-popups'

export function closeTransientPopups() {
  window.dispatchEvent(new Event(CLOSE_TRANSIENT_POPUPS_EVENT))
}

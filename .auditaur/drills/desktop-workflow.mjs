import {execFileSync} from 'node:child_process'
import {readFileSync, rmSync} from 'node:fs'
import path from 'node:path'

const sessionFile = path.join('.auditaur', 'session.json')
const packSource = path.resolve('packs')
let started = null

function auditaur(args, options = {}) {
  const command = process.platform === 'win32' ? 'auditaur.exe' : 'auditaur'
  const stdout = execFileSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 1024 * 1024 * 10,
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout: 90_000,
    ...options,
  })
  return stdout.trim()
}

function auditaurQuiet(args, options = {}) {
  const command = process.platform === 'win32' ? 'auditaur.exe' : 'auditaur'
  execFileSync(command, args, {
    stdio: 'ignore',
    timeout: 90_000,
    ...options,
  })
}

function jsonFrom(stdout) {
  const start = stdout.indexOf('{')
  const end = stdout.lastIndexOf('}')
  if (start < 0 || end < start) throw new Error(`Expected JSON output, got:\n${stdout}`)
  return JSON.parse(stdout.slice(start, end + 1))
}

function startApp() {
  rmSync(sessionFile, {force: true})
  console.log('starting desktop app under Auditaur')
  auditaurQuiet(['start', '--json'], {timeout: 240_000})
  started = JSON.parse(readFileSync(sessionFile, 'utf8'))
  return started.selectors?.drive ?? ['--latest']
}

function drive(selectorArgs, command, args = []) {
  console.log(`drive ${command} ${args.join(' ')}`)
  const result = jsonFrom(auditaur(['drive', command, ...selectorArgs, ...args, '--json']))
  if (result.ok === false) {
    throw new Error(`${command} failed for ${args.join(' ')}:\n${JSON.stringify(result.payload, null, 2)}`)
  }
  return result
}

function text(selectorArgs, selector) {
  return drive(selectorArgs, 'text', ['--selector', selector]).payload?.text ?? ''
}

function expectText(value, expected, label) {
  if (!value.includes(expected)) {
    throw new Error(`${label} did not include ${JSON.stringify(expected)}.\nObserved:\n${value}`)
  }
}

function expectNoRecords(command, args = []) {
  console.log(`checking ${command} ${args.join(' ')}`)
  const value = JSON.parse(auditaur([command, '--session-file', sessionFile, ...args, '--json']))
  const records = Array.isArray(value) ? value : value.records ?? value.items ?? value.errors ?? value.exceptions ?? value.ipc ?? []
  if (records.length > 0) {
    throw new Error(`${command} returned ${records.length} record(s):\n${JSON.stringify(records, null, 2)}`)
  }
}

function stopStartedApp() {
  const ids = [started?.process?.pid, started?.app?.pid].filter(Number.isInteger)
  if (ids.length === 0) return
  console.log(`stopping desktop app pids ${ids.join(', ')}`)
  if (process.platform === 'win32') {
    execFileSync('powershell.exe', [
      '-NoProfile',
      '-Command',
      `Stop-Process -Id ${ids.join(',')} -Force -ErrorAction SilentlyContinue`,
    ], {stdio: 'ignore'})
    return
  }
  const rootPid = started?.process?.pid
  if (Number.isInteger(rootPid)) {
    try {
      process.kill(-rootPid, 'SIGTERM')
    } catch {
      process.kill(rootPid, 'SIGTERM')
    }
  }
  for (const pid of ids.filter(pid => pid !== rootPid)) {
    try {
      process.kill(pid, 'SIGTERM')
    } catch {
      // Process may already have exited with the root command.
    }
  }
}

try {
  const selectorArgs = startApp()
  drive(selectorArgs, 'wait', ['--selector', '.home-workbench', '--timeout-ms', '30000'])
  expectText(text(selectorArgs, '.home-workbench'), 'Open a pack collection', 'home workbench')

  drive(selectorArgs, 'click', ['--selector', '.source-kind-toggle button:nth-child(2)'])
  drive(selectorArgs, 'fill', ['--selector', '#home-source', '--value', packSource])
  drive(selectorArgs, 'click', ['--selector', 'button[type="submit"]'])

  drive(selectorArgs, 'wait', ['--selector', '.pack-list', '--timeout-ms', '30000'])
  const previewText = text(selectorArgs, '.preview-panel')
  expectText(previewText, 'GitHub writes', 'operation preview')
  expectText(previewText, 'README.md', 'operation preview')
  expectText(previewText, 'Workflow dispatch', 'operation preview')

  drive(selectorArgs, 'click', ['--selector', 'button[aria-label="Run no-write dry-run"]'])
  drive(selectorArgs, 'wait', ['--selector', '.queue-run-status', '--timeout-ms', '30000'])
  expectText(text(selectorArgs, '.queue-run-status'), 'Dry-run complete · 12 writes · 6 warmups', 'dry-run status')

  drive(selectorArgs, 'click', ['--selector', 'button[aria-label="Search packs"]'])
  drive(selectorArgs, 'wait', ['--selector', '.pack-search-dialog', '--timeout-ms', '5000'])
  drive(selectorArgs, 'fill', ['--selector', '.pack-search-field input', '--value', 'generic'])
  const searchText = text(selectorArgs, '.pack-search-results')
  expectText(searchText, 'Generic starter', 'pack search')
  expectText(searchText, 'Ready', 'pack search')

  expectNoRecords('errors')
  expectNoRecords('exceptions')
  expectNoRecords('ipc', ['--failed'])

  console.log('desktop-workflow drill passed')
} finally {
  stopStartedApp()
  rmSync(sessionFile, {force: true})
}

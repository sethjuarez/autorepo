import type {CSSProperties} from 'react'
import type {DropdownRect} from './types'

export function dropdownRectFor(element: HTMLElement | null): DropdownRect | null {
  if (!element) return null
  const rect = element.getBoundingClientRect()
  return {
    top: rect.bottom + 4,
    left: rect.left,
    width: rect.width,
  }
}

export function dropdownStyle(rect: DropdownRect | null): CSSProperties | undefined {
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

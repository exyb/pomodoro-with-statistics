import type { Settings } from './types'

export function applyTheme(theme: Settings['appearance']['theme']) {
  const dark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
  document.documentElement.dataset.theme = dark ? 'dark' : 'light'
}

export function applyScale(scale: number) {
  document.documentElement.style.setProperty('--ui-scale', String(scale))
}

export function applyAccent(accent: string) {
  if (accent && /^#[0-9a-f]{3,8}$/i.test(accent)) {
    document.documentElement.style.setProperty('--accent', accent)
  } else {
    document.documentElement.style.removeProperty('--accent')
  }
}

/** The mini window paints its own background alpha, so no compositor tricks are needed. */
export function applyFloatOpacity(opacity: number) {
  document.documentElement.style.setProperty('--float-opacity', String(opacity))
}

/** 紧凑布局 lowers paddings and the ring size via a root attribute. */
export function applyCompact(compact: boolean) {
  if (compact) document.documentElement.dataset.compact = '1'
  else delete document.documentElement.dataset.compact
}

export function watchSystemTheme(onChange: () => void): () => void {
  const mq = window.matchMedia('(prefers-color-scheme: dark)')
  mq.addEventListener('change', onChange)
  return () => mq.removeEventListener('change', onChange)
}

export function applyAll(settings: Settings) {
  applyTheme(settings.appearance.theme)
  applyScale(settings.appearance.uiScale)
  applyAccent(settings.appearance.accent)
  applyFloatOpacity(settings.floating.opacity)
  applyCompact(settings.appearance.compactLayout)
}

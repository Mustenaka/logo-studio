import { describe, expect, it } from 'vitest'
import { i18n, localeOptions } from './index'

function flattenKeys(value: unknown, prefix = ''): string[] {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return prefix ? [prefix] : []
  }

  return Object.entries(value as Record<string, unknown>).flatMap(([key, nested]) => {
    const nextPrefix = prefix ? `${prefix}.${key}` : key
    return flattenKeys(nested, nextPrefix)
  })
}

describe('i18n registry', () => {
  it('uses Chinese as the default and fallback locale', () => {
    expect(i18n.global.locale.value).toBe('zh')
    expect(i18n.global.fallbackLocale.value).toBe('zh')
  })

  it('registers supported locales through localeOptions', () => {
    expect(localeOptions.map((option) => option.code)).toEqual(['zh', 'en'])
  })

  it('keeps Chinese and English message keys in sync', () => {
    const messages = i18n.global.messages.value
    const zhKeys = flattenKeys(messages.zh).sort()
    const enKeys = flattenKeys(messages.en).sort()

    expect(zhKeys).toEqual(enKeys)
    expect(zhKeys.length).toBeGreaterThan(10)
  })
})

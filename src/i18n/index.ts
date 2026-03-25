import { createI18n } from 'vue-i18n'
import zh from './zh.json'
import en from './en.json'

export const localeOptions = [
  { code: 'zh', label: '简体中文' },
  { code: 'en', label: 'English' },
] as const

export type LocaleCode = (typeof localeOptions)[number]['code']

export const defaultLocale: LocaleCode = 'zh'

export const messages = {
  zh,
  en,
} satisfies Record<LocaleCode, typeof zh>

export const i18n = createI18n({
  legacy: false,
  locale: defaultLocale,
  fallbackLocale: defaultLocale,
  messages,
})

import { describe, expect, it } from 'vitest'
import { getGenerationProgressText } from './progressText'

const t = (key: string, params?: Record<string, unknown>) => {
  if (key === 'aiGen.actions.generateProgress') {
    return `Generating ${params?.step} / ${params?.total} steps`
  }
  return key
}

describe('getGenerationProgressText', () => {
  it('prefers status text before step progress', () => {
    const text = getGenerationProgressText(
      t as never,
      { modelId: 'm', step: 0, totalSteps: 30 },
      { modelId: 'm', status: 'mergingLora' },
      30,
    )

    expect(text).toBe('aiGen.actions.status.mergingLora')
  })

  it('falls back to numeric progress when no status is available', () => {
    const text = getGenerationProgressText(
      t as never,
      { modelId: 'm', step: 7, totalSteps: 30 },
      null,
      30,
    )

    expect(text).toBe('Generating 7 / 30 steps')
  })
})

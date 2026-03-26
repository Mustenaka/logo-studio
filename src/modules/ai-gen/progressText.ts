export interface StepProgress {
  modelId: string
  step: number
  totalSteps: number
}

export interface GenerationStatus {
  modelId: string
  status: string
}

export function getGenerationProgressText(
  t: (key: string, params?: Record<string, unknown>) => string,
  stepProgress: StepProgress | null,
  status: GenerationStatus | null,
  fallbackTotal: number,
) {
  if (status?.status) {
    return t(`aiGen.actions.status.${status.status}`)
  }

  return t('aiGen.actions.generateProgress', {
    step: stepProgress?.step ?? 0,
    total: stepProgress?.totalSteps ?? fallbackTotal,
  })
}

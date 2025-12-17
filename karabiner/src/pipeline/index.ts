import type { PipelineContext, Transformer } from '../types'

/**
 * Compose multiple transformers into a single transformer.
 * Each transformer receives the same context and returns manipulators.
 * Results are concatenated in order (first transformer = highest priority).
 */
export function pipe(...transformers: Transformer[]): Transformer {
  return (ctx: PipelineContext) => {
    return transformers.flatMap((t) => t(ctx))
  }
}

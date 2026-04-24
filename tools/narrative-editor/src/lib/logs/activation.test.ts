import { describe, expect, it } from 'vitest'
import { activationSummary } from './metrics'
import type {
  LogEvent, LogFooter, RunModel, SystemActivationEvent,
} from './types'

function sysAct(
  tick: number,
  positive: Record<string, number>,
  negative: Record<string, number> = {},
  neutral: Record<string, number> = {},
): SystemActivationEvent {
  return { type: 'SystemActivation', tick, positive, negative, neutral }
}

function run(partial: {
  id?: string
  events?: LogEvent[]
  footer?: LogFooter | null
}): RunModel {
  return {
    id: partial.id ?? 'test-run',
    files: [],
    header: null,
    footer: partial.footer ?? null,
    events: partial.events ?? [],
    narrative: [],
    traces: [],
    focalCat: null,
    parseErrors: [],
  }
}

describe('activationSummary', () => {
  it('reads per-feature counts from the last SystemActivation event', () => {
    const r = run({
      events: [
        sysAct(100, { WardPlaced: 1 }, { ShadowFoxAmbush: 0 }, { FateAssigned: 2 }),
        sysAct(200, { WardPlaced: 3, CookedMeal: 1 },
                    { ShadowFoxAmbush: 1 },
                    { FateAssigned: 4 }),
      ],
    })
    const s = activationSummary(r)
    expect(s.source).toBe('event')
    expect(s.positive).toEqual({ WardPlaced: 3, CookedMeal: 1 })
    expect(s.negative).toEqual({ ShadowFoxAmbush: 1 })
    expect(s.neutral).toEqual({ FateAssigned: 4 })
  })

  it('returns empty maps + source=footer when no SystemActivation events but footer has aggregates', () => {
    const r = run({
      events: [
        // ColonyScore and other events — no SystemActivation
        { type: 'ColonyScore', tick: 50, welfare: 0.8 },
      ],
      footer: {
        _footer: true,
        positive_features_active: 12,
        positive_features_total: 40,
        neutral_features_active: 3,
        neutral_features_total: 8,
        negative_events_total: 0,
      },
    })
    const s = activationSummary(r)
    expect(s.source).toBe('footer')
    expect(s.positive).toEqual({})
    expect(s.neutral).toEqual({})
    expect(s.negative).toEqual({})
  })

  it('returns source=none when no events and no footer present', () => {
    const s = activationSummary(run({ events: [], footer: null }))
    expect(s.source).toBe('none')
    expect(s.positive).toEqual({})
    expect(s.neverFiredExpected).toEqual([])
  })

  it('round-trips never_fired_expected_positives as string[]', () => {
    const r = run({
      events: [sysAct(100, { WardPlaced: 2 })],
      footer: {
        _footer: true,
        never_fired_expected_positives: ['ScryCompleted', 'CookedMeal'],
      },
    })
    const s = activationSummary(r)
    expect(s.neverFiredExpected).toEqual(['ScryCompleted', 'CookedMeal'])
  })

  it('defensively handles a non-array never_fired_expected_positives', () => {
    const r = run({
      events: [sysAct(100, { WardPlaced: 2 })],
      footer: {
        _footer: true,
        never_fired_expected_positives: 'not an array' as unknown as string[],
      },
    })
    const s = activationSummary(r)
    expect(s.neverFiredExpected).toEqual([])
  })

  it('filters out non-string items in never_fired_expected_positives', () => {
    const r = run({
      events: [sysAct(100, { WardPlaced: 2 })],
      footer: {
        _footer: true,
        never_fired_expected_positives: ['Valid', 42, null, 'AlsoValid'] as unknown as string[],
      },
    })
    const s = activationSummary(r)
    expect(s.neverFiredExpected).toEqual(['Valid', 'AlsoValid'])
  })

  it('returns empty maps when SystemActivation event has no maps at all', () => {
    // Defensive case — sim should always emit all three, but parser may see
    // partial data from a schema drift.
    const r = run({
      events: [
        {
          type: 'SystemActivation', tick: 100,
        } as unknown as SystemActivationEvent,
      ],
    })
    const s = activationSummary(r)
    expect(s.source).toBe('event')
    expect(s.positive).toEqual({})
    expect(s.neutral).toEqual({})
    expect(s.negative).toEqual({})
  })

  it('prefers SystemActivation events over footer aggregates when both are present', () => {
    const r = run({
      events: [sysAct(200, { WardPlaced: 7 })],
      footer: {
        _footer: true,
        positive_features_active: 99,   // would be picked if we were fallback-only
        never_fired_expected_positives: ['SomeCanary'],
      },
    })
    const s = activationSummary(r)
    expect(s.source).toBe('event')
    expect(s.positive).toEqual({ WardPlaced: 7 })
    // Canary list still comes from footer regardless of source.
    expect(s.neverFiredExpected).toEqual(['SomeCanary'])
  })
})

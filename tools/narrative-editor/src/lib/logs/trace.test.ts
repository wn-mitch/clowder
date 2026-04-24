import { describe, expect, it } from 'vitest'
import {
  buildFrameIndex, decisionTickIndex, nearestDecisionTick, stepDecisionTick,
  type L1Record, type L2Record, type L3Record, type L3CommitmentRecord,
  type TraceRecord,
} from './trace'

function l1(tick: number, cat: string, map: string): L1Record {
  return {
    layer: 'L1', tick, cat, map, faction: 'neutral', channel: 'scent',
    pos: [0, 0], base_sample: 0.5,
    attenuation: { species_sens: 1, role_mod: 1, injury_deficit: 0, env_mul: 1 },
    perceived: 0.5, top_contributors: [],
  }
}

function l2(tick: number, cat: string, dse: string, final_score: number): L2Record {
  return {
    layer: 'L2', tick, cat, dse,
    eligibility: { markers_required: [], passed: true },
    considerations: [],
    composition: { mode: 'WeightedSum', raw: final_score },
    maslow_pregate: 1.0, modifiers: [], final_score,
    intention: { kind: 'Activity' },
    top_losing: [],
  }
}

function l3(tick: number, cat: string, chosen: string): L3Record {
  return {
    layer: 'L3', tick, cat,
    ranked: [['Explore', 1.0], ['Hunt', 0.5]],
    softmax: { temperature: 0.15, probabilities: [0.8, 0.2] },
    momentum: { active_intention: null, commitment_strength: 0, margin_threshold: 0, preempted: false },
    chosen, intention: { kind: 'Activity' }, goap_plan: [],
  }
}

function commit(tick: number, cat: string, branch: string, dropped: boolean): L3CommitmentRecord {
  return {
    layer: 'L3Commitment', tick, cat,
    disposition: 'Exploring', strategy: 'OpenMinded',
    proxies: { achievement_believed: true, achievable_believed: false, still_goal: true },
    plan_state: { trips_done: 6, target_trips: 6, replan_count: 3, max_replans: 3 },
    branch, dropped,
  }
}

describe('buildFrameIndex', () => {
  it('groups records into a single frame by (tick, cat)', () => {
    const records: TraceRecord[] = [
      l1(100, 'Simba', 'fox_scent'),
      l1(100, 'Simba', 'prey_scent'),
      l2(100, 'Simba', 'hunt', 0.52),
      l2(100, 'Simba', 'explore', 1.01),
      l3(100, 'Simba', 'Explore'),
    ]
    const idx = buildFrameIndex(records, 'Simba')

    expect(idx.decisionTicks).toEqual([100])
    expect(idx.allTicks).toEqual([100])
    expect(idx.frames.size).toBe(1)

    const frame = idx.frames.get(100)!
    expect(frame.cat).toBe('Simba')
    expect(frame.l1).toHaveLength(2)
    expect(frame.l2).toHaveLength(2)
    expect(frame.l3?.chosen).toBe('Explore')
    expect(frame.commitment).toHaveLength(0)
  })

  it('filters records whose cat does not match the focal', () => {
    const records: TraceRecord[] = [
      l3(100, 'Simba', 'Hunt'),
      l3(100, 'Nala', 'Sleep'),
      l2(100, 'Nala', 'sleep', 0.9),
    ]
    const idx = buildFrameIndex(records, 'Simba')

    expect(idx.decisionTicks).toEqual([100])
    const frame = idx.frames.get(100)!
    expect(frame.l2).toHaveLength(0)
    expect(frame.l3?.chosen).toBe('Hunt')
  })

  it('sorts decision ticks and deduplicates DSE names', () => {
    const records: TraceRecord[] = [
      l3(300, 'Simba', 'Hunt'),
      l2(300, 'Simba', 'hunt', 0.5),
      l3(100, 'Simba', 'Explore'),
      l2(100, 'Simba', 'explore', 1.0),
      l2(100, 'Simba', 'hunt', 0.4),
      l3(200, 'Simba', 'Explore'),
      l2(200, 'Simba', 'explore', 0.9),
    ]
    const idx = buildFrameIndex(records, 'Simba')

    expect(idx.decisionTicks).toEqual([100, 200, 300])
    expect(idx.dseNames).toEqual(['hunt', 'explore'])
  })

  it('marks ticks where the chosen disposition changed', () => {
    const records: TraceRecord[] = [
      l3(100, 'Simba', 'Explore'),
      l3(200, 'Simba', 'Explore'),
      l3(300, 'Simba', 'Hunt'),
      l3(400, 'Simba', 'Hunt'),
      l3(500, 'Simba', 'Explore'),
    ]
    const idx = buildFrameIndex(records, 'Simba')

    // 100 is the initial pick; 300 and 500 are changes from the prior.
    expect(idx.chosenChangeTicks).toEqual([100, 300, 500])
  })

  it('builds per-DSE score series with null gaps for ineligible ticks', () => {
    const records: TraceRecord[] = [
      l3(100, 'Simba', 'Hunt'),
      l2(100, 'Simba', 'hunt', 0.5),
      l3(200, 'Simba', 'Hunt'),
      // No hunt L2 at tick 200 → null gap.
      l2(200, 'Simba', 'sleep', 0.3),
    ]
    const idx = buildFrameIndex(records, 'Simba')

    const hunt = idx.dseSeries.find(s => s.dse === 'hunt')!
    expect(hunt.ticks).toEqual([100, 200])
    expect(hunt.scores).toEqual([0.5, null])

    const sleep = idx.dseSeries.find(s => s.dse === 'sleep')!
    expect(sleep.scores).toEqual([null, 0.3])
  })

  it('collects commitment and plan-failure tick lists', () => {
    const records: TraceRecord[] = [
      l3(100, 'Simba', 'Explore'),
      commit(100, 'Simba', 'achieved', true),
      l3(200, 'Simba', 'Explore'),
      commit(200, 'Simba', 'retained', false),
    ]
    const idx = buildFrameIndex(records, 'Simba')

    expect(idx.commitmentTicks).toEqual([100, 200])
    expect(idx.frames.get(100)?.commitment).toHaveLength(1)
    expect(idx.frames.get(100)?.commitment[0].dropped).toBe(true)
  })

  it('treats null final_score as ineligible when building series', () => {
    const records: TraceRecord[] = [
      l3(100, 'Simba', 'Sleep'),
      {
        layer: 'L2', tick: 100, cat: 'Simba', dse: 'eat',
        eligibility: { markers_required: ['HasStoredFood'], passed: false },
        considerations: [], composition: { mode: 'Unknown', raw: 0 },
        maslow_pregate: 0, modifiers: [], final_score: 0,
        intention: { kind: 'Activity' }, top_losing: [],
      } as L2Record,
    ]
    const idx = buildFrameIndex(records, 'Simba')
    const eat = idx.dseSeries.find(s => s.dse === 'eat')!
    expect(eat.scores).toEqual([null])
  })
})

describe('tick navigation helpers', () => {
  const records: TraceRecord[] = [
    l3(100, 'Simba', 'Explore'),
    l3(200, 'Simba', 'Explore'),
    l3(300, 'Simba', 'Hunt'),
    l3(400, 'Simba', 'Sleep'),
  ]
  const idx = buildFrameIndex(records, 'Simba')

  it('nearestDecisionTick snaps to ≤ tick', () => {
    expect(nearestDecisionTick(idx, 250)).toBe(200)
    expect(nearestDecisionTick(idx, 200)).toBe(200)
    expect(nearestDecisionTick(idx, 99)).toBe(null)
    expect(nearestDecisionTick(idx, 9999)).toBe(400)
  })

  it('decisionTickIndex returns -1 for non-decision ticks', () => {
    expect(decisionTickIndex(idx, 200)).toBe(1)
    expect(decisionTickIndex(idx, 250)).toBe(-1)
  })

  it('stepDecisionTick clamps at both ends', () => {
    expect(stepDecisionTick(idx, 100, 1)).toBe(200)
    expect(stepDecisionTick(idx, 100, -1)).toBe(100)
    expect(stepDecisionTick(idx, 400, 1)).toBe(400)
    expect(stepDecisionTick(idx, 300, 10)).toBe(400)
  })
})

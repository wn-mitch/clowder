// End-to-end sanity test against the real `logs/tuned-42/trace-Simba.jsonl`
// from a full deep-soak. Skipped unless the file is present and
// `CLOWDER_E2E_TRACE` is set in the env — the 641k-line file takes ~1-2s
// to parse and we don't want every CI run to spend that budget.

import { describe, expect, it } from 'vitest'
import { readFileSync, existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { buildFrameIndex, type TraceRecord } from './trace'

const TRACE_PATH = resolve(
  __dirname, '..', '..', '..', '..', '..', 'logs', 'tuned-42', 'trace-Simba.jsonl',
)

const shouldRun = process.env.CLOWDER_E2E_TRACE === '1' && existsSync(TRACE_PATH)

describe.runIf(shouldRun)('real focal-cat trace parse', () => {
  it('parses all layers and builds a non-empty frame index', () => {
    const raw = readFileSync(TRACE_PATH, 'utf-8')
    const lines = raw.split('\n').filter(Boolean)

    let header: { focal_cat?: string } | null = null
    const records: TraceRecord[] = []
    for (const line of lines) {
      const obj = JSON.parse(line)
      if (obj._header === true) { header = obj; continue }
      if (typeof obj.layer !== 'string') continue
      records.push(obj as TraceRecord)
    }

    expect(header).not.toBeNull()
    expect(header!.focal_cat).toBe('Simba')
    expect(records.length).toBeGreaterThan(100_000)

    const layerCounts = records.reduce<Record<string, number>>((acc, r) => {
      acc[r.layer] = (acc[r.layer] ?? 0) + 1; return acc
    }, {})
    expect(layerCounts.L1).toBeGreaterThan(0)
    expect(layerCounts.L2).toBeGreaterThan(0)
    expect(layerCounts.L3).toBeGreaterThan(0)

    const idx = buildFrameIndex(records, 'Simba')
    expect(idx.decisionTicks.length).toBeGreaterThan(100)
    expect(idx.dseNames.length).toBeGreaterThan(5)

    // The first decision tick's frame should have the same number of
    // L2 records as DSEs in the ranked L3 list plus any marker-filtered
    // rows — the substrate spec §11.3 says L2 fires per-eligible-DSE.
    const first = idx.frames.get(idx.decisionTicks[0])!
    expect(first.l3).not.toBeNull()
    expect(first.l2.length).toBeGreaterThan(0)
  }, 30_000)
})

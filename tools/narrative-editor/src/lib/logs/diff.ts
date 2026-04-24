// Helpers for comparing run headers and flagging guardable mismatches.
// Mirrors the spirit of scripts/score_diff.py's schema/constants warnings:
// comparisons across differing commits or constants are not invalid, but
// they should never happen silently.

import type { RunModel } from './types'

export type MismatchKind =
  | 'commit_hash'
  | 'constants'
  | 'duration_secs'
  | 'dirty_tree'
  | 'missing_header'

export interface Mismatch {
  kind: MismatchKind
  message: string
  /** Run IDs implicated in this mismatch. */
  runIds: string[]
}

/** Collect all guardable mismatches across a set of runs. Returns an empty
 *  array if all runs agree and none are dirty. */
export function collectMismatches(runs: RunModel[]): Mismatch[] {
  if (runs.length < 2) {
    const out: Mismatch[] = []
    for (const r of runs) {
      if (r.header?.commit_dirty) {
        out.push({
          kind: 'dirty_tree',
          message: `${shortId(r)} was built from a dirty tree (${r.header?.commit_hash_short ?? '—'}); this run cannot be reproduced from the commit alone.`,
          runIds: [r.id],
        })
      }
    }
    return out
  }

  const mismatches: Mismatch[] = []

  const missing = runs.filter(r => !r.header).map(r => r.id)
  if (missing.length > 0) {
    mismatches.push({
      kind: 'missing_header',
      message: `${missing.length} run(s) have no header — origin and reproducibility cannot be verified.`,
      runIds: missing,
    })
  }

  const withHeader = runs.filter(r => r.header)
  if (withHeader.length >= 2) {
    const commits = new Set(withHeader.map(r => r.header!.commit_hash))
    if (commits.size > 1) {
      mismatches.push({
        kind: 'commit_hash',
        message: `${commits.size} different commit hashes across ${withHeader.length} runs — behavior differences may reflect code changes, not tuning.`,
        runIds: withHeader.map(r => r.id),
      })
    }

    const dirtyRuns = withHeader.filter(r => r.header!.commit_dirty)
    if (dirtyRuns.length > 0) {
      mismatches.push({
        kind: 'dirty_tree',
        message: `${dirtyRuns.length} run(s) were built from a dirty tree and are not reproducible from their commit hashes alone.`,
        runIds: dirtyRuns.map(r => r.id),
      })
    }

    const durations = new Set(withHeader.map(r => r.header!.duration_secs))
    if (durations.size > 1) {
      mismatches.push({
        kind: 'duration_secs',
        message: `Runs have different durations (${Array.from(durations).sort((a, b) => a - b).join(', ')}s) — aggregate metrics may not be directly comparable.`,
        runIds: withHeader.map(r => r.id),
      })
    }

    // Constants comparison: list keys that differ across runs.
    const diffKeys = diffConstants(withHeader)
    if (diffKeys.length > 0) {
      mismatches.push({
        kind: 'constants',
        message: `SimConstants differ in: ${diffKeys.slice(0, 6).join(', ')}${diffKeys.length > 6 ? `, +${diffKeys.length - 6} more` : ''}.`,
        runIds: withHeader.map(r => r.id),
      })
    }
  }

  return mismatches
}

function shortId(run: RunModel): string {
  return run.header?.commit_hash_short ?? run.id.slice(0, 8)
}

/** Returns the sorted list of top-level keys whose values differ (by JSON
 *  equality) across the given runs' `constants` objects. Runs without a
 *  constants block are skipped entirely. */
function diffConstants(runs: RunModel[]): string[] {
  const withConstants = runs.filter(r => r.header?.constants)
  if (withConstants.length < 2) return []
  const keys = new Set<string>()
  for (const r of withConstants) {
    for (const k of Object.keys(r.header!.constants!)) keys.add(k)
  }
  const differing: string[] = []
  for (const k of keys) {
    const serialized = new Set(
      withConstants.map(r => JSON.stringify(r.header!.constants![k] ?? null)),
    )
    if (serialized.size > 1) differing.push(k)
  }
  return differing.sort()
}

// Serializes NarrativeTemplate[] back to RON text.
// Matches the hand-written style: 4-space indent, omits default fields, preserves comments.

import type { NarrativeTemplate } from './types'

function escapeString(s: string): string {
  return s
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/\n/g, '\\n')
    .replace(/\t/g, '\\t')
}

function serializeTemplate(t: NarrativeTemplate): string {
  const lines: string[] = []

  lines.push(`    text: "${escapeString(t.text)}",`)
  lines.push(`    tier: ${t.tier},`)

  if (t.weight !== 1.0) {
    // Format weight to avoid unnecessary decimals: 1.5 not 1.50, but 1.0 is skipped (default)
    const w = Number.isInteger(t.weight) ? `${t.weight}.0` : `${t.weight}`
    lines.push(`    weight: ${w},`)
  }

  if (t.action !== undefined) {
    lines.push(`    action: Some(${t.action}),`)
  }

  if (t.event !== undefined) {
    lines.push(`    event: Some("${escapeString(t.event)}"),`)
  }

  if (t.day_phase !== undefined) {
    lines.push(`    day_phase: Some(${t.day_phase}),`)
  }

  if (t.season !== undefined) {
    lines.push(`    season: Some(${t.season}),`)
  }

  if (t.weather !== undefined) {
    lines.push(`    weather: Some(${t.weather}),`)
  }

  if (t.mood !== undefined) {
    lines.push(`    mood: Some(${t.mood}),`)
  }

  if (t.life_stage !== undefined) {
    lines.push(`    life_stage: Some(${t.life_stage}),`)
  }

  if (t.has_target !== undefined) {
    lines.push(`    has_target: Some(${t.has_target}),`)
  }

  if (t.terrain !== undefined) {
    lines.push(`    terrain: Some(${t.terrain}),`)
  }

  if (t.personality.length > 0) {
    const reqs = t.personality
      .map(r => `(axis: ${r.axis}, bucket: ${r.bucket})`)
      .join(', ')
    lines.push(`    personality: [${reqs}],`)
  }

  if (t.needs.length > 0) {
    const reqs = t.needs
      .map(r => `(axis: ${r.axis}, level: ${r.level})`)
      .join(', ')
    lines.push(`    needs: [${reqs}],`)
  }

  return `(\n${lines.join('\n')}\n)`
}

export function serializeRon(
  templates: NarrativeTemplate[],
  headerComment?: string,
): string {
  const parts: string[] = []

  if (headerComment) {
    parts.push(headerComment)
  }

  parts.push('[')

  for (let i = 0; i < templates.length; i++) {
    const t = templates[i]

    // Re-insert preserved comment block
    if (t._comment) {
      parts.push(`    ${t._comment.split('\n').join('\n    ')}`)
    }

    parts.push(`    ${serializeTemplate(t).split('\n').join('\n    ')},`)

    // Blank line between templates (not after the last one)
    // But don't add blank line if the next template has a comment (the comment provides spacing)
    if (i < templates.length - 1) {
      const nextHasComment = templates[i + 1]._comment !== undefined
      if (!nextHasComment) {
        // Only add blank line within the same "section" (no comment boundary)
      }
    }
  }

  parts.push(']')

  return parts.join('\n') + '\n'
}

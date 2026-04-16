import { describe, it, expect } from 'vitest'
import { parseRon } from './ron-parser'
import { serializeRon } from './ron-serializer'
import { readFileSync, readdirSync } from 'fs'
import { join } from 'path'

describe('RON parser', () => {
  it('parses a minimal template', () => {
    const src = `[
    (
        text: "{name} sits quietly.",
        tier: Micro,
        action: Some(Idle),
    ),
]`
    const { templates } = parseRon(src)
    expect(templates).toHaveLength(1)
    expect(templates[0].text).toBe('{name} sits quietly.')
    expect(templates[0].tier).toBe('Micro')
    expect(templates[0].action).toBe('Idle')
    expect(templates[0].weight).toBe(1.0)
    expect(templates[0].personality).toEqual([])
    expect(templates[0].needs).toEqual([])
  })

  it('parses personality requirements', () => {
    const src = `[
    (
        text: "test",
        tier: Action,
        personality: [(axis: Boldness, bucket: High), (axis: Playfulness, bucket: Low)],
    ),
]`
    const { templates } = parseRon(src)
    expect(templates[0].personality).toEqual([
      { axis: 'Boldness', bucket: 'High' },
      { axis: 'Playfulness', bucket: 'Low' },
    ])
  })

  it('parses needs requirements', () => {
    const src = `[
    (
        text: "test",
        tier: Micro,
        needs: [(axis: Hunger, level: Critical)],
    ),
]`
    const { templates } = parseRon(src)
    expect(templates[0].needs).toEqual([
      { axis: 'Hunger', level: 'Critical' },
    ])
  })

  it('parses event tags', () => {
    const src = `[
    (
        text: "test",
        tier: Action,
        action: Some(Hunt),
        event: Some("catch"),
    ),
]`
    const { templates } = parseRon(src)
    expect(templates[0].event).toBe('catch')
    expect(templates[0].action).toBe('Hunt')
  })

  it('parses has_target', () => {
    const src = `[
    (
        text: "test",
        tier: Action,
        has_target: Some(true),
    ),
]`
    const { templates } = parseRon(src)
    expect(templates[0].has_target).toBe(true)
  })

  it('parses weight', () => {
    const src = `[
    (
        text: "test",
        tier: Action,
        weight: 1.5,
    ),
]`
    const { templates } = parseRon(src)
    expect(templates[0].weight).toBe(1.5)
  })

  it('preserves comments', () => {
    const src = `[
    // --- Section Header ---
    (
        text: "test",
        tier: Micro,
    ),
]`
    const { templates } = parseRon(src)
    expect(templates[0]._comment).toBe('// --- Section Header ---')
  })

  it('handles multi-condition templates', () => {
    const src = `[
    (
        text: "{name} catches snowflakes.",
        tier: Micro,
        action: Some(Idle),
        personality: [(axis: Playfulness, bucket: High)],
        weather: Some(Snow),
        season: Some(Winter),
        mood: Some(Happy),
        life_stage: Some(Kitten),
        terrain: Some(Grass),
    ),
]`
    const { templates } = parseRon(src)
    const t = templates[0]
    expect(t.action).toBe('Idle')
    expect(t.weather).toBe('Snow')
    expect(t.season).toBe('Winter')
    expect(t.mood).toBe('Happy')
    expect(t.life_stage).toBe('Kitten')
    expect(t.terrain).toBe('Grass')
    expect(t.personality).toEqual([{ axis: 'Playfulness', bucket: 'High' }])
  })
})

describe('RON round-trip on real files', () => {
  const narrativeDir = join(__dirname, '../../../../assets/narrative')
  const ronFiles = readdirSync(narrativeDir)
    .filter(f => f.endsWith('.ron'))
    .sort()

  for (const file of ronFiles) {
    it(`parses ${file}`, () => {
      const src = readFileSync(join(narrativeDir, file), 'utf-8')
      const { templates } = parseRon(src)
      expect(templates.length).toBeGreaterThan(0)

      // Every template must have text and tier
      for (const t of templates) {
        expect(t.text).toBeTruthy()
        expect(t.tier).toBeTruthy()
      }
    })
  }

  it('round-trips idle.ron template count', () => {
    const src = readFileSync(join(narrativeDir, 'idle.ron'), 'utf-8')
    const { templates, headerComment } = parseRon(src)
    const serialized = serializeRon(templates, headerComment)
    const { templates: reparsed } = parseRon(serialized)
    expect(reparsed.length).toBe(templates.length)

    // Verify field values match
    for (let i = 0; i < templates.length; i++) {
      expect(reparsed[i].text).toBe(templates[i].text)
      expect(reparsed[i].tier).toBe(templates[i].tier)
      expect(reparsed[i].action).toBe(templates[i].action)
      expect(reparsed[i].weather).toBe(templates[i].weather)
      expect(reparsed[i].mood).toBe(templates[i].mood)
      expect(reparsed[i].personality).toEqual(templates[i].personality)
      expect(reparsed[i].needs).toEqual(templates[i].needs)
    }
  })

  it('round-trips hunt_catch.ron template count', () => {
    const src = readFileSync(join(narrativeDir, 'hunt_catch.ron'), 'utf-8')
    const { templates, headerComment } = parseRon(src)
    const serialized = serializeRon(templates, headerComment)
    const { templates: reparsed } = parseRon(serialized)
    expect(reparsed.length).toBe(templates.length)

    for (let i = 0; i < templates.length; i++) {
      expect(reparsed[i].text).toBe(templates[i].text)
      expect(reparsed[i].event).toBe(templates[i].event)
    }
  })

  it('round-trips all files without data loss', () => {
    for (const file of ronFiles) {
      const src = readFileSync(join(narrativeDir, file), 'utf-8')
      const { templates, headerComment } = parseRon(src)
      const serialized = serializeRon(templates, headerComment)
      const { templates: reparsed } = parseRon(serialized)

      expect(reparsed.length).toBe(templates.length)

      for (let i = 0; i < templates.length; i++) {
        const orig = templates[i]
        const rt = reparsed[i]
        expect(rt.text, `${file}[${i}].text`).toBe(orig.text)
        expect(rt.tier, `${file}[${i}].tier`).toBe(orig.tier)
        expect(rt.weight, `${file}[${i}].weight`).toBe(orig.weight)
        expect(rt.action, `${file}[${i}].action`).toBe(orig.action)
        expect(rt.day_phase, `${file}[${i}].day_phase`).toBe(orig.day_phase)
        expect(rt.season, `${file}[${i}].season`).toBe(orig.season)
        expect(rt.weather, `${file}[${i}].weather`).toBe(orig.weather)
        expect(rt.mood, `${file}[${i}].mood`).toBe(orig.mood)
        expect(rt.life_stage, `${file}[${i}].life_stage`).toBe(orig.life_stage)
        expect(rt.has_target, `${file}[${i}].has_target`).toBe(orig.has_target)
        expect(rt.terrain, `${file}[${i}].terrain`).toBe(orig.terrain)
        expect(rt.event, `${file}[${i}].event`).toBe(orig.event)
        expect(rt.personality, `${file}[${i}].personality`).toEqual(orig.personality)
        expect(rt.needs, `${file}[${i}].needs`).toEqual(orig.needs)
      }
    }
  })
})

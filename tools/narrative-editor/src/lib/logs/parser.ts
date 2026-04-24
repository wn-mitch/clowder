// Streaming JSONL parser for simulation log files.
// Consumes one File at a time; produces a LoadedFile with header, events,
// narrative entries, footer, and per-line parse errors collected inline.
//
// The parser is kind-agnostic: it reads every line, inspects whether the
// object looks like a narrative entry (has "text" + "tier" + "tick") or a
// structured event (has "type"), and routes it to the right bucket. The
// dashboard can then decide what to render.

import type {
  LoadedFile, LogEvent, LogFileKind, LogFooter, LogHeader,
  NarrativeEntry, ParseError,
} from './types'
import type { TraceRecord, TraceLayer } from './trace'

const TRACE_LAYERS: readonly TraceLayer[] = [
  'L1', 'L2', 'L3', 'L3Commitment', 'L3PlanFailure',
] as const

/** Parse a single dropped log file into a LoadedFile. Never throws — all
 *  parse errors are collected into `parseErrors` with their line numbers. */
export async function parseLogFile(file: File): Promise<LoadedFile> {
  const events: LogEvent[] = []
  const narrative: NarrativeEntry[] = []
  const traces: TraceRecord[] = []
  const parseErrors: ParseError[] = []
  let header: LogHeader | null = null
  let footer: LogFooter | null = null

  const stream = file.stream()
    .pipeThrough(new TextDecoderStream('utf-8', { fatal: false }))
    .pipeThrough(lineSplitter())

  const reader = stream.getReader()
  let lineNumber = 0

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    lineNumber += 1
    const trimmed = value.trim()
    if (!trimmed) continue

    let parsed: unknown
    try {
      parsed = JSON.parse(trimmed)
    } catch (e) {
      parseErrors.push({
        line: lineNumber,
        message: (e as Error).message,
      })
      continue
    }

    if (!isObject(parsed)) {
      parseErrors.push({
        line: lineNumber,
        message: `expected JSON object, got ${typeof parsed}`,
      })
      continue
    }

    if (parsed._header === true) {
      header = parsed as unknown as LogHeader
      continue
    }
    if (parsed._footer === true) {
      footer = parsed as unknown as LogFooter
      continue
    }
    if (typeof parsed.layer === 'string'
        && (TRACE_LAYERS as readonly string[]).includes(parsed.layer)
        && typeof parsed.tick === 'number'
        && typeof parsed.cat === 'string') {
      traces.push(parsed as unknown as TraceRecord)
      continue
    }
    if (typeof parsed.type === 'string') {
      events.push(parsed as unknown as LogEvent)
      continue
    }
    if (typeof parsed.text === 'string' && typeof parsed.tier === 'string') {
      narrative.push(parsed as unknown as NarrativeEntry)
      continue
    }

    parseErrors.push({
      line: lineNumber,
      message: 'line did not match header / footer / event / narrative / trace shape',
    })
  }

  return {
    name: file.name,
    size: file.size,
    kind: detectKind(file.name, events.length, narrative.length, traces.length),
    header,
    events,
    narrative,
    traces,
    footer,
    parseErrors,
    loadedAt: new Date(),
  }
}

function detectKind(
  name: string, eventsCount: number, narrativeCount: number, tracesCount: number,
): LogFileKind {
  const lower = name.toLowerCase()
  if (lower.startsWith('trace') || lower.includes('/trace-') || lower.includes('trace-')) return 'trace'
  if (lower.includes('narrative')) return 'narrative'
  if (lower.includes('event')) return 'events'
  if (tracesCount > 0 && eventsCount === 0 && narrativeCount === 0) return 'trace'
  if (eventsCount > 0 && narrativeCount === 0) return 'events'
  if (narrativeCount > 0 && eventsCount === 0) return 'narrative'
  return 'unknown'
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** Splits a UTF-8 string stream into single lines (without the terminator).
 *  Buffers partial lines until the next chunk arrives; flushes the final
 *  unterminated line on close. */
function lineSplitter(): TransformStream<string, string> {
  let buffer = ''
  return new TransformStream<string, string>({
    transform(chunk, controller) {
      buffer += chunk
      let newlineIdx = buffer.indexOf('\n')
      while (newlineIdx !== -1) {
        const line = buffer.slice(0, newlineIdx)
        buffer = buffer.slice(newlineIdx + 1)
        controller.enqueue(line)
        newlineIdx = buffer.indexOf('\n')
      }
    },
    flush(controller) {
      if (buffer.length > 0) controller.enqueue(buffer)
      buffer = ''
    },
  })
}

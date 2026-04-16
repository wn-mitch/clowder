// Recursive-descent parser for the narrative template RON subset.
// Handles: arrays, tuple structs, Some/None, bare enums, strings, floats, bools, comments.

import type { NarrativeTemplate, PersonalityReq, NeedReq } from './types'

class ParseError extends Error {
  constructor(message: string, public pos: number, public source: string) {
    const line = source.slice(0, pos).split('\n').length
    const col = pos - source.lastIndexOf('\n', pos - 1)
    super(`RON parse error at line ${line}, col ${col}: ${message}`)
  }
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

type TokenType =
  | 'LBracket' | 'RBracket' | 'LParen' | 'RParen'
  | 'Colon' | 'Comma'
  | 'String' | 'Number' | 'Ident' | 'Bool'
  | 'Comment' | 'EOF'

interface Token {
  type: TokenType
  value: string
  pos: number
}

function tokenize(src: string): Token[] {
  const tokens: Token[] = []
  let i = 0

  while (i < src.length) {
    // Whitespace
    if (/\s/.test(src[i])) { i++; continue }

    // Line comment — capture as token so we can preserve them
    if (src[i] === '/' && src[i + 1] === '/') {
      const start = i
      while (i < src.length && src[i] !== '\n') i++
      tokens.push({ type: 'Comment', value: src.slice(start, i), pos: start })
      continue
    }

    // Single chars
    if (src[i] === '[') { tokens.push({ type: 'LBracket', value: '[', pos: i }); i++; continue }
    if (src[i] === ']') { tokens.push({ type: 'RBracket', value: ']', pos: i }); i++; continue }
    if (src[i] === '(') { tokens.push({ type: 'LParen', value: '(', pos: i }); i++; continue }
    if (src[i] === ')') { tokens.push({ type: 'RParen', value: ')', pos: i }); i++; continue }
    if (src[i] === ':') { tokens.push({ type: 'Colon', value: ':', pos: i }); i++; continue }
    if (src[i] === ',') { tokens.push({ type: 'Comma', value: ',', pos: i }); i++; continue }

    // String literal
    if (src[i] === '"') {
      const start = i
      i++ // skip opening quote
      let val = ''
      while (i < src.length && src[i] !== '"') {
        if (src[i] === '\\') {
          i++
          if (src[i] === 'n') val += '\n'
          else if (src[i] === 't') val += '\t'
          else if (src[i] === '\\') val += '\\'
          else if (src[i] === '"') val += '"'
          else if (src[i] === 'u') {
            // Unicode escape: \u{XXXX}
            if (src[i + 1] === '{') {
              i += 2
              let hex = ''
              while (i < src.length && src[i] !== '}') hex += src[i++]
              val += String.fromCodePoint(parseInt(hex, 16))
            } else {
              val += '\\u'
            }
          } else {
            val += src[i]
          }
        } else {
          val += src[i]
        }
        i++
      }
      i++ // skip closing quote
      tokens.push({ type: 'String', value: val, pos: start })
      continue
    }

    // Number (float or int, possibly negative)
    if (/[0-9]/.test(src[i]) || (src[i] === '-' && /[0-9]/.test(src[i + 1] || ''))) {
      const start = i
      if (src[i] === '-') i++
      while (i < src.length && /[0-9]/.test(src[i])) i++
      if (i < src.length && src[i] === '.') {
        i++
        while (i < src.length && /[0-9]/.test(src[i])) i++
      }
      tokens.push({ type: 'Number', value: src.slice(start, i), pos: start })
      continue
    }

    // Identifier (includes enum variants, "true", "false", "Some", "None")
    if (/[a-zA-Z_]/.test(src[i])) {
      const start = i
      while (i < src.length && /[a-zA-Z0-9_]/.test(src[i])) i++
      const word = src.slice(start, i)
      if (word === 'true' || word === 'false') {
        tokens.push({ type: 'Bool', value: word, pos: start })
      } else {
        tokens.push({ type: 'Ident', value: word, pos: start })
      }
      continue
    }

    throw new ParseError(`Unexpected character: ${src[i]}`, i, src)
  }

  tokens.push({ type: 'EOF', value: '', pos: i })
  return tokens
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

class Parser {
  private tokens: Token[]
  private pos = 0
  private src: string
  private pendingComments: string[] = []

  constructor(tokens: Token[], src: string) {
    this.tokens = tokens
    this.src = src
  }

  private peek(): Token {
    return this.tokens[this.pos]
  }

  private advance(): Token {
    return this.tokens[this.pos++]
  }

  private expect(type: TokenType): Token {
    const tok = this.advance()
    if (tok.type !== type) {
      throw new ParseError(`Expected ${type}, got ${tok.type} ("${tok.value}")`, tok.pos, this.src)
    }
    return tok
  }

  private consumeComments() {
    while (this.peek().type === 'Comment') {
      this.pendingComments.push(this.advance().value)
    }
  }

  private takePendingComment(): string | undefined {
    if (this.pendingComments.length === 0) return undefined
    const comment = this.pendingComments.join('\n')
    this.pendingComments = []
    return comment
  }

  parseTemplateList(): { templates: NarrativeTemplate[]; headerComment?: string } {
    this.consumeComments()
    const headerComment = this.takePendingComment()

    this.expect('LBracket')

    const templates: NarrativeTemplate[] = []

    while (true) {
      this.consumeComments()

      if (this.peek().type === 'RBracket') break

      const comment = this.takePendingComment()
      const template = this.parseTemplate()
      if (comment) template._comment = comment
      templates.push(template)

      // Optional trailing comma
      if (this.peek().type === 'Comma') this.advance()
    }

    this.expect('RBracket')
    return { templates, headerComment }
  }

  private parseTemplate(): NarrativeTemplate {
    this.expect('LParen')

    const template: NarrativeTemplate = {
      text: '',
      tier: 'Micro',
      weight: 1.0,
      personality: [],
      needs: [],
    }

    while (this.peek().type !== 'RParen') {
      this.consumeComments()
      if (this.peek().type === 'RParen') break

      const fieldName = this.expect('Ident').value
      this.expect('Colon')

      switch (fieldName) {
        case 'text':
          template.text = this.expect('String').value
          break
        case 'tier':
          template.tier = this.expect('Ident').value as NarrativeTemplate['tier']
          break
        case 'weight':
          template.weight = parseFloat(this.expect('Number').value)
          break
        case 'action':
          template.action = this.parseOption() as NarrativeTemplate['action']
          break
        case 'day_phase':
          template.day_phase = this.parseOption() as NarrativeTemplate['day_phase']
          break
        case 'season':
          template.season = this.parseOption() as NarrativeTemplate['season']
          break
        case 'weather':
          template.weather = this.parseOption() as NarrativeTemplate['weather']
          break
        case 'mood':
          template.mood = this.parseOption() as NarrativeTemplate['mood']
          break
        case 'life_stage':
          template.life_stage = this.parseOption() as NarrativeTemplate['life_stage']
          break
        case 'has_target':
          template.has_target = this.parseOptionBool()
          break
        case 'terrain':
          template.terrain = this.parseOption() as NarrativeTemplate['terrain']
          break
        case 'event':
          template.event = this.parseOptionString()
          break
        case 'personality':
          template.personality = this.parsePersonalityArray()
          break
        case 'needs':
          template.needs = this.parseNeedsArray()
          break
        default:
          // Skip unknown fields — consume the value
          this.skipValue()
      }

      // Optional trailing comma
      if (this.peek().type === 'Comma') this.advance()
    }

    this.expect('RParen')
    return template
  }

  private parseOption(): string | undefined {
    const tok = this.advance()
    if (tok.type === 'Ident' && tok.value === 'None') return undefined
    if (tok.type === 'Ident' && tok.value === 'Some') {
      this.expect('LParen')
      const val = this.expect('Ident').value
      this.expect('RParen')
      return val
    }
    throw new ParseError(`Expected Some(...) or None, got "${tok.value}"`, tok.pos, this.src)
  }

  private parseOptionString(): string | undefined {
    const tok = this.advance()
    if (tok.type === 'Ident' && tok.value === 'None') return undefined
    if (tok.type === 'Ident' && tok.value === 'Some') {
      this.expect('LParen')
      const val = this.expect('String').value
      this.expect('RParen')
      return val
    }
    throw new ParseError(`Expected Some("...") or None, got "${tok.value}"`, tok.pos, this.src)
  }

  private parseOptionBool(): boolean | undefined {
    const tok = this.advance()
    if (tok.type === 'Ident' && tok.value === 'None') return undefined
    if (tok.type === 'Ident' && tok.value === 'Some') {
      this.expect('LParen')
      const val = this.expect('Bool').value === 'true'
      this.expect('RParen')
      return val
    }
    throw new ParseError(`Expected Some(bool) or None, got "${tok.value}"`, tok.pos, this.src)
  }

  private parsePersonalityArray(): PersonalityReq[] {
    this.expect('LBracket')
    const reqs: PersonalityReq[] = []
    while (this.peek().type !== 'RBracket') {
      this.expect('LParen')
      let axis = '' as PersonalityReq['axis']
      let bucket = '' as PersonalityReq['bucket']
      // Parse fields in any order
      for (let f = 0; f < 2; f++) {
        const name = this.expect('Ident').value
        this.expect('Colon')
        const val = this.expect('Ident').value
        if (name === 'axis') axis = val as PersonalityReq['axis']
        else if (name === 'bucket') bucket = val as PersonalityReq['bucket']
        if (this.peek().type === 'Comma') this.advance()
      }
      this.expect('RParen')
      reqs.push({ axis, bucket })
      if (this.peek().type === 'Comma') this.advance()
    }
    this.expect('RBracket')
    return reqs
  }

  private parseNeedsArray(): NeedReq[] {
    this.expect('LBracket')
    const reqs: NeedReq[] = []
    while (this.peek().type !== 'RBracket') {
      this.expect('LParen')
      let axis = '' as NeedReq['axis']
      let level = '' as NeedReq['level']
      for (let f = 0; f < 2; f++) {
        const name = this.expect('Ident').value
        this.expect('Colon')
        const val = this.expect('Ident').value
        if (name === 'axis') axis = val as NeedReq['axis']
        else if (name === 'level') level = val as NeedReq['level']
        if (this.peek().type === 'Comma') this.advance()
      }
      this.expect('RParen')
      reqs.push({ axis, level })
      if (this.peek().type === 'Comma') this.advance()
    }
    this.expect('RBracket')
    return reqs
  }

  private skipValue() {
    // Skip a single value: nested parens/brackets or a simple token
    const tok = this.advance()
    if (tok.type === 'LParen') {
      let depth = 1
      while (depth > 0) {
        const t = this.advance()
        if (t.type === 'LParen') depth++
        else if (t.type === 'RParen') depth--
        else if (t.type === 'EOF') break
      }
    } else if (tok.type === 'LBracket') {
      let depth = 1
      while (depth > 0) {
        const t = this.advance()
        if (t.type === 'LBracket') depth++
        else if (t.type === 'RBracket') depth--
        else if (t.type === 'EOF') break
      }
    } else if (tok.type === 'Ident' && tok.value === 'Some') {
      this.expect('LParen')
      this.skipValue()
      this.expect('RParen')
    }
    // Simple values (String, Number, Bool, Ident like None) are already consumed
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function parseRon(src: string): { templates: NarrativeTemplate[]; headerComment?: string } {
  const tokens = tokenize(src)
  const parser = new Parser(tokens, src)
  return parser.parseTemplateList()
}

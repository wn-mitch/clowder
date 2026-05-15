# Mood

## Purpose
Tracks each cat's emotional state as a single signed scalar (`valence`) plus a deque of time-limited
additive modifiers. Mood feeds personality, social dynamics, and several DSEs; it is read by code
that wants "how does this cat feel right now?" and written by event sites that want to nudge a cat
toward happier or sadder over a bounded window.

## Pattern
Mood is a **continuous-scalar accumulator with time-limited additive modifiers** — *not* a Markov
chain (no finite mood states, no transition probabilities) and *not* a deterministic cascade (the
update is not "state A becomes state B given condition C"). The shape is closer to a discretized
stochastic-drift model: a personality-driven baseline plus a sum of decaying perturbations.

| Aspect | What it is |
|--------|-----------|
| State carrier | `Mood { valence: f32 in [-1, 1], modifiers: VecDeque<MoodModifier> }` (`src/components/mental.rs`) |
| Per-modifier shape | `{ amount: f32, ticks_remaining: u64, source: String, kind: MoodSource }` |
| Update rule | `valence = clamp(baseline + Σ positive + Σ amplified_negative, -1, 1)` (`src/systems/mood.rs::update_mood`) |
| Decay | Per-modifier TTL countdown; expired modifiers drop. Fear-kind decays faster (`fear_decay_rate` ticks per real tick). |
| Injection | Rule-triggered modifier `push_back` at event sites (combat → Fear; bonded death → Grief; phys-needs-high → Contentment-Physical; respect-low → Pride-wounded; …) |

## MoodSource categories
The `MoodSource` enum (`src/components/mental.rs:17`) tags each modifier so the per-tick update can
apply per-kind decay rates and per-kind anxiety amplification. Eight variants:

| Variant | Domain | Decay / amp notes |
|---------|--------|--------------------|
| `Physical` | physiological: contentment, hunger relief, warmth, remedy | standard |
| `Social` | social warmth, contagion, play, kitten proximity | standard |
| `Fear` | acute threat: fled combat, predator encounter | faster decay; anxiety amplifies more |
| `Grief` | loss: bonded death, witnessed death | slower decay; anxiety amplifies less |
| `Triumph` | shared victory: banishment, aspiration completion | slow decay (identity-defining) |
| `Pride` | esteem: wounded pride, built something, combat win | standard |
| `Magic` | spirit communion, ward success, corruption events | standard |
| `Misc` | unclassified push sites — uses standard rates | standard |

## Why this matters

The pattern label has engineering consequences. New mood-affecting features should be expressed as
*modifier injections* (one `push_back` at the event site, with an `amount`, a TTL, and a
`MoodSource`), not as direct `valence` writes. Direct writes lose the audit trail, skip the
anxiety-amplification pipeline, and don't decay — they leave permanent ratchets in a system that
otherwise self-heals over time.

Conversely, this is **not the right substrate for "moods that switch and stay switched"** (e.g. a
cat that flips into a depressive state and remains there until something specific happens). That
would require an explicit state variable with its own transition rule — Markov-flavored or otherwise
— layered on top of the valence accumulator. Don't try to encode that by carefully picking modifier
TTLs; the resulting system would be impossible to reason about. Add a state, name it, and write the
transition.

## Tuning Notes
_Record observations and adjustments here during iteration._

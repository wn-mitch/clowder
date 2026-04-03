# Narrative

## Purpose
Converts mechanical simulation state into readable prose via RON template matching. Three tiers provide different granularities of story: ambient micro-behavior, action narration, and significant events.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Rate limit — micro-behaviors | 1 per cat per 5 ticks | Prevents text flood; maintains readability |
| Template target volume — v1 | 100–150 templates | Enough variety to avoid obvious repetition |

### Tier Definitions
| Tier | Display Style | Description |
|------|--------------|-------------|
| Tier 1 (micro-behavior) | Dimmed | Ambient flavor; low-significance cat actions |
| Tier 2 (action narration) | Normal weight | Standard action descriptions |
| Tier 3 (significant event) | Always shown / highlighted | Deaths, bonds formed, magic events, etc. |

### Template Matching
| Step | Rule |
|------|------|
| Candidate selection | All templates where every non-None condition field matches current state |
| Specificity score | Count of non-None condition fields in template |
| Selection method | Weighted random by (specificity * template.weight) |

### Template Variables
| Variable | Resolves To |
|----------|-------------|
| `{name}` | Cat's name |
| `{other}` | Other cat in interaction |
| `{location_name}` | Named location or terrain description |
| `{prey}` | Type of prey being hunted/caught |
| `{item}` | Item being used or crafted |
| `{weather_desc}` | Human-readable weather description |
| `{time_desc}` | Human-readable time-of-day description |

## Formulas
```
candidates = [t for t in templates if all conditions match current_state]

specificity(t) = count(field for field in t.conditions if field is not None)

selection_weight(t) = specificity(t) * t.weight

chosen = weighted_random(candidates, weights=[selection_weight(t) for t in candidates])
```

## Tuning Notes
_Record observations and adjustments here during iteration._

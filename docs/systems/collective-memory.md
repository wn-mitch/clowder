# Collective Memory

## Purpose
Tracks what individual cats know and how colony-level knowledge emerges from social transmission. Enables emergent avoidance behaviors, resource knowledge, and oral tradition without a global omniscient state.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Individual memory capacity | 20 entries | Limits cognitive overhead; forces forgetting of old/weak memories |
| Initial strength — firsthand | 1.0 | Witnessed directly; maximum confidence |
| Initial strength — secondhand | 0.5 | Heard from another; half as reliable |
| Decay rate — firsthand | 0.01/tick | Vivid memories persist longer |
| Decay rate — secondhand | 0.02/tick | Rumors fade twice as fast |
| Transmission probability — base | 0.1 per social interaction | Low baseline; not every chat spreads knowledge |
| Transmission probability — threat | +0.1 bonus (total 0.2) | Danger is more eagerly shared |
| Transmission probability — gathering | +0.2 bonus (total 0.3) | Gatherings are prime knowledge-sharing events |
| Colony knowledge threshold | 3+ carriers | Minimum to become "colony knowledge" |
| Colony knowledge decay rate | 0.001/tick | Much slower; institutional knowledge is durable |

### Memory Types
| Type | Description |
|------|-------------|
| ThreatSeen | Location and nature of a predator or hostile agent |
| ResourceFound | Location of food, herbs, or materials |
| Death | Record of a colony member's death |
| MagicEvent | Witnessed magic cast or corruption event |
| Injury | Known injury to a colony member |
| SocialEvent | Significant gathering, conflict, or bond formation |

## Formulas
```
strength(t+1) = strength(t) - decay_rate

transmission_probability = base_prob + threat_bonus + gathering_bonus

becomes_colony_knowledge = count(cats with memory.strength > 0) >= 3

colony_knowledge_strength(t+1) = colony_knowledge_strength(t) - 0.001
```

When capacity is exceeded, the weakest (lowest strength) memory is evicted.

## Tuning Notes
_Record observations and adjustments here during iteration._

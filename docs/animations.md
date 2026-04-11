# Sprite Animation Reference

Frame ranges for the two animation sheets used in Clowder. Open
`tools/sprite_catalog.html` (Trees / Characters tabs) to inspect frames visually.

---

## Trees

**Sheet:** `assets/sprites/Sprout Lands - Sprites - premium pack/Objects/Tree animations/tree_sprites.png`  
**Layout:** 12 cols × 4 rows · 48×48 px per sprite

| Row | Name | All frames | Active frames | Notes |
|-----|------|-----------|--------------|-------|
| 0 | Tree | 0–11 | 0 | Static; single frame |
| 1 | TreeShake | 12–23 | 12–15 | Short jostle (4 frames) |
| 2 | TreeWindy1 | 24–35 | 24–29 | Moderate sway (6 frames) |
| 3 | TreeWindy2 | 36–47 | 36–47 | Full windy cycle (12 frames) |

First frame of row N = N × 12.

---

## Characters

**Sheet:** `assets/sprites/Sprout Lands - Sprites - premium pack/Characters/Premium Charakter Spritesheet.png`  
**Layout:** 8 cols × 24 rows · 48×48 px per sprite

| Row | Name | Frames |
|-----|------|--------|
| 0 | South Idle | 0–7 |
| 1 | North Idle | 8–15 |
| 2 | West Idle | 16–23 |
| 3 | East Idle | 24–31 |
| 4 | South Run | 32–39 |
| 5 | North Run | 40–47 |
| 6 | West Run | 48–55 |
| 7 | East Run | 56–63 |
| 8 | South Sprint | 64–71 |
| 9 | North Sprint | 72–79 |
| 10 | West Sprint | 80–87 |
| 11 | East Sprint | 88–95 |
| 12 | South Hoe | 96–103 |
| 13 | North Hoe | 104–111 |
| 14 | West Hoe | 112–119 |
| 15 | East Hoe | 120–127 |
| 16 | South Chop | 128–135 |
| 17 | North Chop | 136–143 |
| 18 | West Chop | 144–151 |
| 19 | East Chop | 152–159 |
| 20 | South Water | 160–167 |
| 21 | North Water | 168–175 |
| 22 | West Water | 176–183 |
| 23 | East Water | 184–191 |

First frame of row N = N × 8.

Rows 6/7, 9–11, 13–15, 17–19, 21–23 are directional variants inferred from the
sheet's pattern — only South-facing rows were explicitly confirmed in the catalog.

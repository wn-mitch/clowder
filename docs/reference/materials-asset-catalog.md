# Materials Asset (16×16) — sprite catalog

Reference for the `materials` atlas declared in
[`assets/sprites/bindings.toml`](../../assets/sprites/bindings.toml).

- **Texture:** `sprites/materials_sheet.png` (a non-hidden copy of the pack's
  `Materials Asset (16x16)/Materials Asset/.Full Sheet(Normal).png`; the editor's
  PNG walker skips dotfiles, so the served sheet must be non-hidden).
- **Grid:** `cols = 10`, `rows = 25`, `tile = 16` (160×400). Row-major:
  `index = row*10 + col`. 244 real frames (0–243); indices **244–249 are blank**.
- **Variants on disk (not wired):** the pack also ships Black-Outline,
  Selected, and Black-Outline+Selected variant folders (244 frames each) plus a
  Mortar-and-Pestle animation. Only the Normal full sheet is bound.

This doc lives outside `bindings.toml` on purpose: the sprite editor
(`just sprite-editor`) regenerates the TOML on save and drops freestanding
comments, so the durable catalog lives here and per-binding rationale lives in
each entry's `note =` field.

## Frame catalog (index → depiction)

Identifications are eyeballed from a 6× labelled montage of the sheet; tiny
16×16 icons are sometimes ambiguous, so treat fine distinctions as approximate.

| Row | Cols 0–9 (indices left→right) |
|-----|-------------------------------|
| r0 (0–9) | bone · curved rib bone · light tusk/horn · skull · brown ore rock · brown ore boulder · red-topped pouch · folded hide · blue crystal shard · amber shard |
| r1 (10–19) | red vial · blue crystal · dark coal · dark coal · gray rock · gray stone · silver ore boulder · iron ore boulder · silver flakes · iron nuggets |
| r2 (20–29) | silver ore chunk · silver nuggets · flint shards · tan pebbles · gravel · smooth stone · stone block · silver ingot · crossed twigs · nuts/seeds |
| r3 (30–39) | gray rocks · tan pebbles · gold nuggets · gold ingot (sm) · gold ingot · gold bar · gray boulder · flat gray stone · brown-flecked stone · stone shards |
| r4 (40–49) | silver boulder · gray rocks · stone chunk · gold ingot bar · gold bar · gold nuggets · gold nuggets (sm) · wooden club/mallet · purple folded cloth · — |
| r5 (50–59) | lit torch · brazier/campfire · wood log · wood log · wood plank · firewood bundle · silver-wrapped log · gray plank · plain log · blue firewood bundle |
| r6 (60–69) | wooden spoon · whisk/crossed tools · pestle · mortar bowl · empty bowl/pot · sinew strand · curved filament · dark cauldron · lit cauldron · ring/horseshoe |
| r7 (70–79) | dark ladle · empty bowl · wooden mug · shallow bowl · dried reed bundle · leafy greens · gray axe/blade head · trowel/spade · sickle/hook · pickaxe/blade |
| r8 (80–89) | white daisy · white diamond · white/cyan gem · lavender sprig · purple flower · amethyst · red tulip · red flower · orange gem · blue forget-me-nots |
| r9 (90–99) | blue gems · sapphire · red rose · rose petal/gem · green sprig · sunflower · feather · teal meat · purple meat · pink/red meat |
| r10 (100–109) | blue flask · wheat sheaf · pretzel · bread loaf · oval bread · round bun · bread roll · pale bun · pale loaf · pale bun |
| r11 (110–119) | dough ball · flatbread/pita · small dough · flour sack · honey jar w/dipper · rolling pin · empty bucket · milk bucket · water bucket · knife |
| r12 (120–129) | thin dagger · gray cleaver · crossed hammer+pick · stone hammer · anvil/metal pot · purple geode · blue flame gem · red fire crystal · blue gem · blue ore |
| r13 (130–139) | navy gem orb · pale fat chunk · brown meat strip · arrow/wand · blue-fletched arrow · staff/arrow · blue arrow · bronze ring · copper ring · gold hook/ring |
| r14 (140–149) | jeweled rings & plain silver rings (140–145 jeweled, 146–149 plain/clasp) |
| r15 (150–159) | rings, beaded necklaces, gem chains |
| r16 (160–169) | gold chain · onion/garlic bulbs · white cloves · red apple/tomato · red apples · tan mushrooms · red berry cluster · green cabbage · green cabbage · ginger root |
| r17 (170–179) | rooted garlic · blueberry · blueberries · blue garlic/mushroom · gray-blue stones · pink raw meat · pink raw meat · ham slice · ham slice · meat on bone |
| r18 (180–189) | raw organ · carrot · tan beans · potatoes/tubers · small seeds · red apple w/leaf · brown nuts · eggplant · tan seeds · peach |
| r19 (190–199) | tan roots · radish/beet · tan beans · meat w/green · tan beans · red chili · brown beans · pink steak · brown steak · cooked drumstick |
| r20 (200–209) | fried drumstick · roast · roast bird · raw steak · red meat cut · ham steak · bacon/pink meat · ham hock on bone · raw ham leg · red meat slice |
| r21 (210–219) | fried chicken · sausage · bone-in drumstick · gray egg · brown egg · teal egg · bacon strips · golden pastry · golden shrimp/seahorse · pink shrimp curl |
| r22 (220–229) | pink meat on bone · golden skewer · blue dart · **whole fish** · green ball · red ball · blue ball · purple-handled mallet · white rune/snowflake · silver S-rune |
| r23 (230–239) | brown horn · purple orb · pearl orb · white egg-face · orange orb/yolk · leafy herb · green sprig+stick · green pea pod · red-brown bacon · tan fat strip |
| r24 (240–249) | dark curved horn · gold amulet (eye) · white bone/tusk · tan bone/horn · *(244–249 blank)* |

## Item → cell rebindings applied

All bound via `atlas = "materials"`, index as listed. Edit any of these visually
in `just sprite-editor`.

**Raw prey:** RawMouse 177 · RawRat 176 · RawRabbit 208 · RawFish 223 · RawBird 212
**Foraged:** Berries 166 · Nuts 29 · Roots 190 · WildOnion 161 · Mushroom 165 · Moss 235 · DriedGrass 74 · Feather 96
**Herb items:** HerbHealingMoss 94 · HerbMoonpetal 80 · HerbCalmroot 169 · HerbThornbriar 92 · HerbDreamroot 83 · HerbCatnip 75 · HerbSlumbershade 89 · HerbOracleOrchid 84
**Curiosities:** ShinyPebble 8 · GlassShard 81 · ColorfulShell 232
**Shadow:** ShadowBone 3 (skull)
**Storage / build:** Barrel 116 · Shelf 54 · Wood 52 · Stone 26
**Remedies:** RemedyHealingPoultice 114 · RemedyEnergyTonic 100 · RemedyMoodTonic 10
**Organ / preserved:** RawOrgan 180 · DriedFish 218 · SmokedMeat 198 · PreservedOrgan 205
**Byproducts:** Bone 0 · Sinew 65 · Whisker 66 · Hide 7 · FishScale 18 · Tallow 131
**Crafting inputs:** Twig 28 · Flower 86 · PolishedStone 25
**Behavioral tool:** CourtshipGift 241
**Warrior's kit** (was all on the shared `solaria_objects:622` placeholder):
BoneTipSpear 133 · BoneStiletto 120 · FlintBlade 121 · HideBracers 230 ·
HidePlatedWrap 239 · Sling 229 · WovenReedCloak 48 · ToothNotchedClub 47
**Flavor-plant rocks** (static, all stages): Pebble 23 · Rock 14 · Stone 15 ·
StoneChunk 42 · StoneFlat 37 · Boulder 36

## Intentionally left on prior bindings

The pack has no clearly-better icon for these, so they keep their existing
sprites:

- **Crate** — no crate icon in the pack.
- **Bristle / Fiber** — Solaria's labelled *Pelt* / *String* cells are more
  literal than anything here.
- **GroomingBrush** — no brush/comb in the pack.
- **PlayBundle** — already bound to a bundle tile.
- **`herbs.*` growth-stage plants** (HealingMoss, Thornbriar, Moonpetal,
  Calmroot, Dreamroot, Catnip, Slumbershade, OracleOrchid) and
  **`flavor_plants.Sunflower` / `Rose`** — these need 4-frame growth
  progressions the single-icon materials pack can't provide; they stay on the
  Sprout Lands herbs atlas.

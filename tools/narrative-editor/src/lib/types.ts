// Types mirroring the Rust narrative template schema.
// Source of truth: src/resources/narrative_templates.rs

export type NarrativeTier = 'Micro' | 'Action' | 'Significant' | 'Danger' | 'Nature'

export type Action =
  | 'Eat' | 'Sleep' | 'Hunt' | 'Forage' | 'Wander' | 'Idle'
  | 'Socialize' | 'Groom' | 'Explore' | 'Flee' | 'Fight' | 'Patrol'
  | 'Build' | 'Farm' | 'Herbcraft' | 'PracticeMagic'
  | 'Coordinate' | 'Mentor' | 'Mate' | 'Caretake'

export type DayPhase = 'Dawn' | 'Day' | 'Dusk' | 'Night'

export type Season = 'Spring' | 'Summer' | 'Autumn' | 'Winter'

export type Weather =
  | 'Clear' | 'Overcast' | 'LightRain' | 'HeavyRain'
  | 'Snow' | 'Fog' | 'Wind' | 'Storm'

export type MoodBucket = 'Miserable' | 'Low' | 'Neutral' | 'Happy' | 'Euphoric'

export type LifeStage = 'Kitten' | 'Young' | 'Adult' | 'Elder'

export type Terrain =
  | 'Grass' | 'LightForest' | 'DenseForest' | 'Water' | 'Rock' | 'Mud' | 'Sand'
  | 'Den' | 'Hearth' | 'Stores' | 'Workshop' | 'Garden'
  | 'Watchtower' | 'WardPost' | 'Wall' | 'Gate'
  | 'FairyRing' | 'StandingStone' | 'DeepPool' | 'AncientRuin'

export type PersonalityAxis =
  | 'Boldness' | 'Sociability' | 'Curiosity' | 'Diligence'
  | 'Warmth' | 'Spirituality' | 'Ambition' | 'Patience'
  | 'Anxiety' | 'Optimism' | 'Temper' | 'Stubbornness' | 'Playfulness'
  | 'Loyalty' | 'Tradition' | 'Compassion' | 'Pride' | 'Independence'

export type PersonalityBucket = 'Low' | 'Mid' | 'High'

export type NeedAxis =
  | 'Hunger' | 'Energy' | 'Warmth' | 'Safety' | 'Social'
  | 'Acceptance' | 'Respect' | 'Mastery' | 'Purpose'

export type NeedLevel = 'Critical' | 'Low' | 'Moderate' | 'Satisfied'

export interface PersonalityReq {
  axis: PersonalityAxis
  bucket: PersonalityBucket
}

export interface NeedReq {
  axis: NeedAxis
  level: NeedLevel
}

export interface NarrativeTemplate {
  text: string
  tier: NarrativeTier
  weight: number
  action?: Action
  day_phase?: DayPhase
  season?: Season
  weather?: Weather
  mood?: MoodBucket
  personality: PersonalityReq[]
  needs: NeedReq[]
  life_stage?: LifeStage
  has_target?: boolean
  terrain?: Terrain
  event?: string
  /** Preserved comment block preceding this template in the RON source. */
  _comment?: string
}

/** A loaded .ron file with its templates and metadata. */
export interface TemplateFile {
  name: string
  templates: NarrativeTemplate[]
  dirty: boolean
  /** Leading comment block before the first template (e.g. file-level section headers). */
  _headerComment?: string
}

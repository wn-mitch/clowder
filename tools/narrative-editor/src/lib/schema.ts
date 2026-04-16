// Enum value arrays with labels and metadata.
// Single source of truth for all dropdowns, heatmap axes, and coverage logic.
// Derived from the Rust types in src/resources/narrative_templates.rs and related modules.

import type {
  Action, DayPhase, Season, Weather, MoodBucket, LifeStage,
  Terrain, PersonalityAxis, PersonalityBucket, NeedAxis, NeedLevel,
  NarrativeTier,
} from './types'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export interface EnumEntry<T> {
  value: T
  label: string
  description?: string
}

// ---------------------------------------------------------------------------
// Narrative Tiers
// ---------------------------------------------------------------------------

export const TIERS: EnumEntry<NarrativeTier>[] = [
  { value: 'Micro',       label: 'Micro',       description: 'Ambient observations — background flavor' },
  { value: 'Action',      label: 'Action',      description: 'Routine actions — the bread and butter' },
  { value: 'Significant', label: 'Significant', description: 'Story-worthy moments — memorable events' },
  { value: 'Danger',      label: 'Danger',      description: 'Threats and combat — high stakes' },
  { value: 'Nature',      label: 'Nature',      description: 'Environmental events — weather, seasons' },
]

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

export const ACTIONS: EnumEntry<Action>[] = [
  { value: 'Eat',           label: 'Eat' },
  { value: 'Sleep',         label: 'Sleep' },
  { value: 'Hunt',          label: 'Hunt' },
  { value: 'Forage',        label: 'Forage' },
  { value: 'Wander',        label: 'Wander' },
  { value: 'Idle',          label: 'Idle' },
  { value: 'Socialize',     label: 'Socialize' },
  { value: 'Groom',         label: 'Groom' },
  { value: 'Explore',       label: 'Explore' },
  { value: 'Flee',          label: 'Flee' },
  { value: 'Fight',         label: 'Fight' },
  { value: 'Patrol',        label: 'Patrol' },
  { value: 'Build',         label: 'Build' },
  { value: 'Farm',          label: 'Farm' },
  { value: 'Herbcraft',     label: 'Herbcraft' },
  { value: 'PracticeMagic', label: 'Practice Magic' },
  { value: 'Coordinate',    label: 'Coordinate' },
  { value: 'Mentor',        label: 'Mentor' },
  { value: 'Mate',          label: 'Mate' },
  { value: 'Caretake',      label: 'Caretake' },
]

// ---------------------------------------------------------------------------
// Day Phases
// ---------------------------------------------------------------------------

export const DAY_PHASES: EnumEntry<DayPhase>[] = [
  { value: 'Dawn',  label: 'Dawn' },
  { value: 'Day',   label: 'Day' },
  { value: 'Dusk',  label: 'Dusk' },
  { value: 'Night', label: 'Night' },
]

// ---------------------------------------------------------------------------
// Seasons
// ---------------------------------------------------------------------------

export const SEASONS: EnumEntry<Season>[] = [
  { value: 'Spring', label: 'Spring' },
  { value: 'Summer', label: 'Summer' },
  { value: 'Autumn', label: 'Autumn' },
  { value: 'Winter', label: 'Winter' },
]

// ---------------------------------------------------------------------------
// Weather
// ---------------------------------------------------------------------------

export const WEATHERS: EnumEntry<Weather>[] = [
  { value: 'Clear',     label: 'Clear' },
  { value: 'Overcast',  label: 'Overcast' },
  { value: 'LightRain', label: 'Light Rain' },
  { value: 'HeavyRain', label: 'Heavy Rain' },
  { value: 'Snow',      label: 'Snow' },
  { value: 'Fog',       label: 'Fog' },
  { value: 'Wind',      label: 'Wind' },
  { value: 'Storm',     label: 'Storm' },
]

// ---------------------------------------------------------------------------
// Mood
// ---------------------------------------------------------------------------

export const MOODS: EnumEntry<MoodBucket>[] = [
  { value: 'Miserable', label: 'Miserable', description: 'Valence < -0.3' },
  { value: 'Low',       label: 'Low',       description: 'Valence -0.3 to 0.0' },
  { value: 'Neutral',   label: 'Neutral',   description: 'Valence 0.0 to 0.3' },
  { value: 'Happy',     label: 'Happy',     description: 'Valence 0.3 to 0.7' },
  { value: 'Euphoric',  label: 'Euphoric',  description: 'Valence >= 0.7' },
]

// ---------------------------------------------------------------------------
// Life Stages
// ---------------------------------------------------------------------------

export const LIFE_STAGES: EnumEntry<LifeStage>[] = [
  { value: 'Kitten', label: 'Kitten', description: '0-3 seasons' },
  { value: 'Young',  label: 'Young',  description: '4-11 seasons' },
  { value: 'Adult',  label: 'Adult',  description: '12-47 seasons' },
  { value: 'Elder',  label: 'Elder',  description: '48+ seasons' },
]

// ---------------------------------------------------------------------------
// Terrain
// ---------------------------------------------------------------------------

export const TERRAINS: EnumEntry<Terrain>[] = [
  // Natural
  { value: 'Grass',        label: 'Grass' },
  { value: 'LightForest',  label: 'Light Forest' },
  { value: 'DenseForest',  label: 'Dense Forest' },
  { value: 'Water',        label: 'Water' },
  { value: 'Rock',         label: 'Rock' },
  { value: 'Mud',          label: 'Mud' },
  { value: 'Sand',         label: 'Sand' },
  // Settlement
  { value: 'Den',          label: 'Den' },
  { value: 'Hearth',       label: 'Hearth' },
  { value: 'Stores',       label: 'Stores' },
  { value: 'Workshop',     label: 'Workshop' },
  { value: 'Garden',       label: 'Garden' },
  // Defensive
  { value: 'Watchtower',   label: 'Watchtower' },
  { value: 'WardPost',     label: 'Ward Post' },
  { value: 'Wall',         label: 'Wall' },
  { value: 'Gate',         label: 'Gate' },
  // Special
  { value: 'FairyRing',    label: 'Fairy Ring' },
  { value: 'StandingStone', label: 'Standing Stone' },
  { value: 'DeepPool',     label: 'Deep Pool' },
  { value: 'AncientRuin',  label: 'Ancient Ruin' },
]

// ---------------------------------------------------------------------------
// Personality Axes
// ---------------------------------------------------------------------------

export interface PersonalityAxisEntry extends EnumEntry<PersonalityAxis> {
  lowLabel: string
  highLabel: string
  group: 'Core Drives' | 'Temperament' | 'Values'
}

export const PERSONALITY_AXES: PersonalityAxisEntry[] = [
  // Core Drives
  { value: 'Boldness',     label: 'Boldness',     lowLabel: 'cautious',      highLabel: 'bold',          group: 'Core Drives' },
  { value: 'Sociability',  label: 'Sociability',   lowLabel: 'solitary',      highLabel: 'gregarious',    group: 'Core Drives' },
  { value: 'Curiosity',    label: 'Curiosity',     lowLabel: 'routine',       highLabel: 'adventurous',   group: 'Core Drives' },
  { value: 'Diligence',    label: 'Diligence',     lowLabel: 'lazy',          highLabel: 'industrious',   group: 'Core Drives' },
  { value: 'Warmth',       label: 'Warmth',        lowLabel: 'aloof',         highLabel: 'affectionate',  group: 'Core Drives' },
  { value: 'Spirituality', label: 'Spirituality',  lowLabel: 'pragmatic',     highLabel: 'mystical',      group: 'Core Drives' },
  { value: 'Ambition',     label: 'Ambition',      lowLabel: 'content',       highLabel: 'ambitious',     group: 'Core Drives' },
  { value: 'Patience',     label: 'Patience',      lowLabel: 'impulsive',     highLabel: 'deliberate',    group: 'Core Drives' },
  // Temperament
  { value: 'Anxiety',      label: 'Anxiety',       lowLabel: 'serene',        highLabel: 'nervous',       group: 'Temperament' },
  { value: 'Optimism',     label: 'Optimism',      lowLabel: 'melancholic',   highLabel: 'cheerful',      group: 'Temperament' },
  { value: 'Temper',       label: 'Temper',        lowLabel: 'even-keeled',   highLabel: 'volatile',      group: 'Temperament' },
  { value: 'Stubbornness', label: 'Stubbornness',  lowLabel: 'flexible',      highLabel: 'headstrong',    group: 'Temperament' },
  { value: 'Playfulness',  label: 'Playfulness',   lowLabel: 'serious',       highLabel: 'mischievous',   group: 'Temperament' },
  // Values
  { value: 'Loyalty',      label: 'Loyalty',       lowLabel: 'self-interested', highLabel: 'devoted',     group: 'Values' },
  { value: 'Tradition',    label: 'Tradition',     lowLabel: 'iconoclast',    highLabel: 'traditionalist', group: 'Values' },
  { value: 'Compassion',   label: 'Compassion',    lowLabel: 'detached',      highLabel: 'empathetic',    group: 'Values' },
  { value: 'Pride',        label: 'Pride',         lowLabel: 'humble',        highLabel: 'proud',         group: 'Values' },
  { value: 'Independence', label: 'Independence',  lowLabel: 'communal',      highLabel: 'self-reliant',  group: 'Values' },
]

export const PERSONALITY_BUCKETS: EnumEntry<PersonalityBucket>[] = [
  { value: 'Low',  label: 'Low',  description: 'Value < 0.33' },
  { value: 'Mid',  label: 'Mid',  description: 'Value 0.33 to 0.67' },
  { value: 'High', label: 'High', description: 'Value >= 0.67' },
]

// ---------------------------------------------------------------------------
// Needs
// ---------------------------------------------------------------------------

export const NEED_AXES: EnumEntry<NeedAxis>[] = [
  { value: 'Hunger',     label: 'Hunger' },
  { value: 'Energy',     label: 'Energy' },
  { value: 'Warmth',     label: 'Warmth' },
  { value: 'Safety',     label: 'Safety' },
  { value: 'Social',     label: 'Social' },
  { value: 'Acceptance', label: 'Acceptance' },
  { value: 'Respect',    label: 'Respect' },
  { value: 'Mastery',    label: 'Mastery' },
  { value: 'Purpose',    label: 'Purpose' },
]

export const NEED_LEVELS: EnumEntry<NeedLevel>[] = [
  { value: 'Critical',  label: 'Critical',  description: 'Value < 0.2' },
  { value: 'Low',       label: 'Low',       description: 'Value 0.2 to 0.4' },
  { value: 'Moderate',  label: 'Moderate',  description: 'Value 0.4 to 0.7' },
  { value: 'Satisfied', label: 'Satisfied', description: 'Value >= 0.7' },
]

// ---------------------------------------------------------------------------
// Template Variables
// ---------------------------------------------------------------------------

export interface TemplateVariable {
  name: string
  description: string
  example: string
  /** Actions/contexts where this variable is available. Empty = always available. */
  availableFor?: string[]
}

export const TEMPLATE_VARIABLES: TemplateVariable[] = [
  { name: 'name',         description: "The cat's name",                        example: 'Bramble' },
  { name: 'Subject',      description: 'Capitalized subject pronoun',           example: 'She' },
  { name: 'subject',      description: 'Lowercase subject pronoun',             example: 'she' },
  { name: 'object',       description: 'Object pronoun',                        example: 'her' },
  { name: 'possessive',   description: 'Possessive pronoun',                    example: 'her' },
  { name: 'other',        description: "Target cat's name (or 'a companion')",  example: 'Reed' },
  { name: 'weather_desc', description: 'Current weather label',                 example: 'Snow' },
  { name: 'time_desc',    description: 'Current day phase',                     example: 'Dusk' },
  { name: 'season',       description: 'Current season',                        example: 'Winter' },
  { name: 'life_stage',   description: 'Cat life stage label',                  example: 'kitten' },
  { name: 'fur_color',    description: "Cat's fur color",                       example: 'tortoiseshell' },
  { name: 'prey',         description: 'Prey species name',                     example: 'vole',    availableFor: ['Hunt'] },
  { name: 'item',         description: 'Foraged item name',                     example: 'berries', availableFor: ['Forage'] },
  { name: 'quality',      description: 'Quality tier label',                    example: 'fine',    availableFor: ['Hunt', 'Forage', 'Herbcraft'] },
]

// ---------------------------------------------------------------------------
// Coverage axis metadata (for heatmap axis selectors)
// ---------------------------------------------------------------------------

export type CoverageAxisId =
  | 'action' | 'day_phase' | 'season' | 'weather'
  | 'mood' | 'life_stage' | 'terrain' | 'event'

export interface CoverageAxis {
  id: CoverageAxisId
  label: string
  values: { value: string; label: string }[]
}

export const COVERAGE_AXES: CoverageAxis[] = [
  { id: 'action',     label: 'Action',     values: ACTIONS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'mood',       label: 'Mood',       values: MOODS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'weather',    label: 'Weather',    values: WEATHERS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'season',     label: 'Season',     values: SEASONS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'day_phase',  label: 'Day Phase',  values: DAY_PHASES.map(e => ({ value: e.value, label: e.label })) },
  { id: 'life_stage', label: 'Life Stage', values: LIFE_STAGES.map(e => ({ value: e.value, label: e.label })) },
  { id: 'terrain',    label: 'Terrain',    values: TERRAINS.map(e => ({ value: e.value, label: e.label })) },
]

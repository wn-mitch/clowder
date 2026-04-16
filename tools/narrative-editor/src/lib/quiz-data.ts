// Cat personality questionnaire data.
// Ported from tools/cat_questionnaire.html — scoring deltas and question text.

import type { PersonalityAxis } from './types'

export type TraitDeltas = Partial<Record<Lowercase<PersonalityAxis>, number>>

export interface QuizQuestion {
  title: string
  flavor: string
  options: string[]
}

// Each question maps answer index (0=a, 1=b, ...) to trait deltas.
export const SCORING: TraitDeltas[][] = [
  // Q1: The String Hunt
  [
    { boldness: -0.10, playfulness: -0.10, patience: 0.10 },
    { boldness: 0.05, playfulness: 0.05, patience: 0.15 },
    { boldness: 0.05, playfulness: 0.15, patience: -0.05 },
    { boldness: 0.10, playfulness: 0.05, patience: -0.10 },
    { boldness: 0.15, playfulness: 0.10, patience: -0.15 },
  ],
  // Q2: A Stranger Arrives
  [
    { sociability: -0.15, anxiety: 0.15, boldness: -0.10, warmth: -0.05 },
    { sociability: -0.05, anxiety: 0.05, boldness: 0.05 },
    {},
    { sociability: 0.15, anxiety: -0.10, boldness: 0.10, warmth: 0.10 },
    { sociability: 0.10, anxiety: -0.05, boldness: 0.15, warmth: 0.15 },
  ],
  // Q3: The Closed Door
  [
    { curiosity: -0.10, stubbornness: -0.10, diligence: -0.05 },
    { curiosity: 0.05, stubbornness: 0.05, patience: -0.10, diligence: -0.05 },
    { curiosity: 0.10, stubbornness: 0.05, patience: 0.05, diligence: 0.05 },
    { curiosity: 0.15, stubbornness: 0.10, patience: 0.10, diligence: 0.15 },
    { curiosity: 0.10, stubbornness: 0.15, patience: -0.10, diligence: 0.10 },
  ],
  // Q4: The Empty Food Bowl
  [
    { temper: -0.10, diligence: -0.05, patience: 0.15, independence: -0.05 },
    { temper: 0.05, diligence: -0.05, patience: 0.05, independence: -0.05 },
    { temper: -0.05, diligence: 0.15, patience: 0.05, independence: 0.05 },
    { temper: 0.10, diligence: 0.05, patience: -0.10, independence: 0.10 },
    { temper: 0.05, diligence: 0.10, patience: -0.15, independence: 0.15 },
  ],
  // Q5: Another Cat's Toy
  [
    { compassion: -0.05, pride: 0.15, warmth: -0.05 },
    { compassion: 0.05, pride: -0.05, warmth: 0.05 },
    { compassion: 0.15, pride: -0.10, warmth: 0.15 },
    { compassion: -0.15, pride: 0.10, warmth: -0.10 },
    { compassion: 0.10, pride: -0.05, warmth: 0.10 },
  ],
  // Q6: The Vacuum Cleaner
  [
    { anxiety: 0.15, boldness: -0.15, optimism: -0.10 },
    { anxiety: 0.05, boldness: -0.05 },
    { anxiety: -0.05, boldness: 0.10, optimism: 0.05 },
    { anxiety: -0.10, boldness: 0.15, optimism: 0.10 },
    { anxiety: -0.15, boldness: 0.10, optimism: 0.15 },
  ],
  // Q7: Favorite Spot Occupied
  [
    { tradition: -0.15, temper: -0.10, stubbornness: -0.10, ambition: -0.05 },
    { tradition: 0.10, temper: -0.05, stubbornness: 0.10, ambition: -0.05 },
    { tradition: 0.15, temper: 0.05, stubbornness: 0.10, ambition: 0.05 },
    { tradition: 0.10, temper: 0.10, stubbornness: 0.15, ambition: 0.15 },
    { tradition: 0.05, temper: 0.15, stubbornness: 0.10, ambition: 0.10 },
  ],
  // Q8: The Bird Outside
  [
    { curiosity: -0.10, boldness: -0.05, spirituality: -0.05 },
    { curiosity: 0.10, patience: -0.05, boldness: 0.05, spirituality: 0.05 },
    { curiosity: 0.10, patience: 0.15, boldness: 0.05, spirituality: 0.05 },
    { curiosity: 0.05, patience: -0.15, boldness: 0.15, spirituality: -0.05 },
    { curiosity: 0.05, patience: 0.05, boldness: 0.10, spirituality: 0.10 },
  ],
  // Q9: Your Laptop
  [
    { ambition: -0.10, loyalty: -0.10, playfulness: -0.05, warmth: -0.05 },
    { ambition: -0.05, loyalty: 0.15, playfulness: -0.05, warmth: 0.10 },
    { ambition: 0.15, loyalty: 0.05, playfulness: 0.05, warmth: 0.05 },
    { ambition: 0.05, loyalty: 0.05, playfulness: 0.15, warmth: 0.05 },
    { ambition: 0.10, loyalty: 0.10, playfulness: 0.10, warmth: -0.05 },
  ],
  // Q10: The 3 AM Phenomenon
  [
    { playfulness: -0.15, independence: -0.05, curiosity: -0.10, optimism: 0.05 },
    { playfulness: 0.05, independence: 0.05, curiosity: 0.05, optimism: 0.05 },
    { playfulness: 0.15, independence: 0.10, curiosity: 0.05, optimism: 0.10 },
    { playfulness: 0.10, independence: -0.05, curiosity: 0.05, optimism: 0.15 },
    { playfulness: 0.05, independence: 0.15, curiosity: 0.15, optimism: -0.05 },
  ],
  // Q11: The Vet
  [
    { anxiety: 0.05, pride: -0.15, stubbornness: -0.15 },
    { anxiety: 0.15, pride: 0.05, stubbornness: 0.05 },
    { anxiety: 0.10, pride: 0.10, stubbornness: 0.15 },
    { anxiety: -0.05, pride: 0.15, stubbornness: 0.10 },
    { anxiety: -0.10, pride: 0.10, stubbornness: 0.10 },
  ],
  // Q12: A New Cat Appears
  [
    { sociability: -0.15, loyalty: 0.10, compassion: -0.10, independence: -0.05 },
    { sociability: -0.05, compassion: -0.05, independence: 0.15 },
    { independence: 0.05 },
    { sociability: 0.15, loyalty: -0.05, compassion: 0.15, independence: -0.10 },
    { sociability: 0.10, loyalty: 0.10, compassion: 0.10, independence: -0.05 },
  ],
  // Q13: The Sacred Routine
  [
    { tradition: -0.15, curiosity: 0.05, independence: 0.05 },
    { tradition: -0.05 },
    { tradition: 0.10, patience: 0.05 },
    { tradition: 0.15, patience: 0.10, stubbornness: 0.05 },
    { tradition: 0.10, patience: 0.15, diligence: 0.05 },
  ],
  // Q14: The Love Language
  [
    { warmth: -0.10, loyalty: -0.10, sociability: -0.10, independence: 0.05 },
    { warmth: 0.05, loyalty: 0.05 },
    { warmth: 0.15, sociability: 0.10 },
    { loyalty: 0.15, warmth: 0.05, independence: -0.10 },
    { warmth: 0.15, loyalty: 0.10, sociability: 0.10 },
  ],
  // Q15: Staring at Nothing
  [
    { spirituality: -0.15, curiosity: -0.05 },
    { spirituality: -0.05, curiosity: 0.05, patience: -0.05 },
    { spirituality: 0.10, curiosity: 0.10, patience: 0.10 },
    { spirituality: 0.15, curiosity: 0.15, patience: 0.05 },
    { spirituality: 0.10, curiosity: 0.05, patience: 0.15 },
  ],
]

export const QUESTIONS: QuizQuestion[] = [
  {
    title: 'The String Hunt',
    flavor: 'When you drag a string across the floor, your cat:',
    options: [
      "Watches from across the room, pretending not to care. It's beneath them. (They're tracking it.)",
      'Sneaks up with military precision and delivers one surgical pounce. Mission complete.',
      'Wiggles their butt dramatically for 30 seconds, then overshoots by a mile.',
      'Grabs it immediately and sprints to a hiding spot. Possession is law.',
      'The string has been shredded. The couch has been shredded. Nothing with fibers is safe in your home.',
    ],
  },
  {
    title: 'A Stranger Arrives',
    flavor: 'A new person enters the house. Your cat:',
    options: [
      'Gone. Vanished. Will reappear in 3\u20136 business hours.',
      'Observes from a high perch, gathering intelligence.',
      'Cautious approach, one sniff, returns to their spot. You passed muster. For now.',
      "Already in the visitor's lap, demanding tribute. This is their new person now.",
      "Brings them a dead thing. (Or a hairband. Or a sock.) You're family now.",
    ],
  },
  {
    title: 'The Closed Door',
    flavor: "There's a closed door in the house. Your cat:",
    options: [
      "Ignores it. If the door is closed, it wasn't meant to be.",
      'Sits outside and cries. Sits and cries. Sits and cries.',
      "Reaches a paw under the gap and fishes around like they're checking for survivors.",
      'Methodically tests the handle. Has learned how handles work.',
      "This is an open door household. They won't tolerate anything else.",
    ],
  },
  {
    title: 'The Empty Food Bowl',
    flavor: 'The food bowl is empty. Your cat:',
    options: [
      'Sits politely next to it. Makes meaningful eye contact. Waits.',
      'Meows once. If nothing happens, meows at the wall about it.',
      'Leads you to the bowl. Leads you back. Leads you to the bowl again. You are being project-managed.',
      'Knocks something off the counter. Maintains eye contact the entire time.',
      'Has located the food bag. Has opened the food bag. Welcome to the self-service economy.',
    ],
  },
  {
    title: "Another Cat's Toy",
    flavor: "There's another cat's toy on the floor. Your cat:",
    options: [
      "Wouldn't touch it. Has their own, thank you very much.",
      'Watches the other cat play, waits for them to leave, then casually inspects it.',
      'Brings their own toy over and drops it nearby. Playdate initiated.',
      'Takes it. This is theirs now. All toys have always been theirs.',
      'Ignores the toy, walks over, and starts grooming the other cat instead.',
    ],
  },
  {
    title: 'The Vacuum Cleaner',
    flavor: 'You turn on the vacuum. Your cat:',
    options: [
      "Under the bed before you've even opened the closet.",
      'Relocates to another room with dignified haste. Not scared. Strategically repositioning.',
      'Watches from the arm of the couch. Suspicious. Holding ground.',
      'Has swatted the vacuum. The vacuum started it.',
      'Rides the Roomba. Has claimed it as a vehicle.',
    ],
  },
  {
    title: 'Favorite Spot Occupied',
    flavor: "Someone is in your cat's spot. Your cat:",
    options: [
      'Finds somewhere else. Spots are temporary. Naps are forever.',
      'Sits nearby and radiates disappointment until the spot opens up.',
      "Squeezes in anyway. There is room. There is always room.",
      "Sits ON the occupant. The spot includes whatever's currently in it.",
      'Initiates a campaign of total warfare until the occupant relocates.',
    ],
  },
  {
    title: 'The Bird Outside',
    flavor: "There's a bird outside the window. Your cat:",
    options: [
      "Couldn't care less. Birds are a concept that doesn't apply to them.",
      'Chatters their teeth at it. The ancient, involuntary battle cry.',
      'Watches in complete stillness for 45 minutes. No blinking. Just bird.',
      'Full-body slams the window. The bird is unfazed. Your cat is undeterred.',
      'Has caught one before. Brought it inside. Presented it with ceremony.',
    ],
  },
  {
    title: 'Your Laptop',
    flavor: "You're trying to work. Your cat:",
    options: [
      'You and your glowing rectangle are beneath notice.',
      'Curls up next to you while you work. Quiet solidarity.',
      'Sits on the keyboard. Your work is done now. Accept this.',
      "Bats at the cursor. Every cursor movement is a personal invitation.",
      'Has closed your laptop lid. Has opened 14 tabs on your phone.',
    ],
  },
  {
    title: 'The 3 AM Phenomenon',
    flavor: 'At 3 AM, your cat:',
    options: [
      'Sleeps through the night like a normal, reasonable creature.',
      'Emits one single loud meow. No context provided. Returns to sleep.',
      'Parkour. Full-contact parkour off every surface in the house.',
      "Brings you a toy and drops it on your face. It's play time.",
      'You hear sounds from rooms that should be empty.',
    ],
  },
  {
    title: 'The Vet',
    flavor: "It's time for the vet. Your cat:",
    options: [
      'Goes limp. Accepts fate. Has transcended this mortal coil.',
      "Vanishes at the first sight of the carrier. You are now 30 minutes late.",
      'Requires two people and a towel. There were casualties (your forearms).',
      'Hisses at the vet. Just the vet. Everyone else gets a pass.',
      'The vet has a note in the file. The note says "Beware."',
    ],
  },
  {
    title: 'A New Cat Appears',
    flavor: 'You bring a new cat into the household. Your cat:',
    options: [
      'Hisses and retreats to a high shelf. This is a betrayal.',
      "Pretends the new cat doesn't exist. For weeks.",
      'Cautious parallel existence. Borders have been drawn.',
      'Grooming the new cat within 48 hours. Best friends acquired.',
      'Has adopted the new cat as their own kitten, regardless of size or age.',
    ],
  },
  {
    title: 'The Sacred Routine',
    flavor: "Your cat's daily habits:",
    options: [
      'What routine? Every day is improvised. The only constant is chaos.',
      'Has a loose pattern \u2014 breakfast spot, afternoon sun patch \u2014 but adapts without complaint.',
      'Same sequence every day. Window. Nap spot. Dinner. Deviation is noted.',
      'Has a schedule. You have learned the schedule. It does not accommodate yours.',
      'Patrols the same route at the same time each day. You could set a clock by it.',
    ],
  },
  {
    title: 'The Love Language',
    flavor: 'How does your cat show affection?',
    options: [
      "They don't. You are the help. Compensation is room and board.",
      "The slow blink from across the room. Once a day, if you're lucky.",
      'Headbutts. Relentless, precision-targeted headbutts.',
      "Follows you from room to room. Not clingy. Supervisory.",
      'Falls asleep on you and purrs until your ribs vibrate.',
    ],
  },
  {
    title: 'Staring at Nothing',
    flavor: 'Your cat and empty walls:',
    options: [
      "Doesn't do this. A grounded, normal cat who lives in the material world.",
      "Occasionally stares at a corner. It's probably a bug. (There's never a bug.)",
      'Regular wall-staring sessions. Ten-minute minimum. No blinking.',
      'Tracks things you cannot see across the room. Gets up to follow them.',
      'Has a spot. Sits in the spot and stares. The spot has no distinguishing features.',
    ],
  },
]

export const TRAIT_KEYS = [
  'boldness', 'sociability', 'curiosity', 'diligence',
  'warmth', 'spirituality', 'ambition', 'patience',
  'anxiety', 'optimism', 'temper', 'stubbornness', 'playfulness',
  'loyalty', 'tradition', 'compassion', 'pride', 'independence',
] as const

export type TraitKey = typeof TRAIT_KEYS[number]

export const TRAIT_SECTIONS = [
  { label: 'Core Drives', start: 0, end: 8 },
  { label: 'Temperament', start: 8, end: 13 },
  { label: 'Values', start: 13, end: 18 },
] as const

export const TRAIT_COLORS: Record<TraitKey, string> = {
  boldness: '#e07040', sociability: '#e0a040', curiosity: '#d4d040',
  diligence: '#80c040', warmth: '#e08080', spirituality: '#a080d0',
  ambition: '#d06060', patience: '#60a0c0',
  anxiety: '#c06080', optimism: '#e0c060', temper: '#d04040',
  stubbornness: '#a08060', playfulness: '#60c0a0',
  loyalty: '#6080c0', tradition: '#a09070', compassion: '#80b080',
  pride: '#c0a060', independence: '#70a0a0',
}

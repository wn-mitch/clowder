# Behavioral Mathematics for Game AI — Chapter 14: Modeling Individual Decisions

> Extracted from `docs/reference/BehavioralMathematicalforGameAI.pdf` (Dave Mark, 2009)  
> PDF pages 370–413 · Book pages 351–394

---

## 14. Modeling Individual Decisions
To this point, we have pondered a lot of theory, laid out plenty of tools, and even examined ways of measuring our workspace. While all of that preparatory work was necessary, we have arrived at the point where we can put all of what we have learned to use. Before we proceed with the glorious and rewarding process of crafting our decision models, however, we really need to determine what we are doing. Before we act, we must choose. Before we choose, we must decide what we are choosing. After all, the decision to choose (or choice to decide?) isn’t one to take lightly. I’ve also heard it said that “if you choose not to decide, you still have made a choice.” (Wow… just saying that gives me a Rush.) With all of this choosing and deciding and acting ahead of us, perhaps a definition of terms is in order.

### DEFINING DECISION

The most atomic structure in behavioral game artificial intelligence (AI) is the individual decision. I use the word atomic, not in the literal sense that it was first used— that is, “the smallest possible object”—but rather in the sense of “what bigger things are built out of.” The true definition of the word atom is “something so small as to prohibit further division.” Scientists of the past originally named atoms “atoms” because the belief was that there was nothing smaller from which an atom was made. They believed it was impossible to divide them further. Since that point, of course, we have discovered otherwise. The etymology of the word has drifted as well. When we talk about the chemical nature of a substance, we don’t make a count of the electrons, protons, and neutrons that are involved. We refer to the atoms. We may refer to a molecule as well, but usually that molecule is made up of atoms. We even name molecules after the atoms that are in them.

Only through changing the name slightly do we imply that a subatomic particle is missing or that there is an extra one along for the ride. So, despite being divisible (and putting the lie to the original meaning of their name), atoms are still the core building blocks from which everything we see, touch, and feel is made. We can say the same for individual choices in game AI. When we look at a game character on a screen, we see many actions. Some are individual choices (e.g., “use the gun instead of my fists”), and some are actual physical events (such as “fire the gun one time”). We also witness conglomerations of multiple actions (such as “draw the gun, raise the gun, aim the gun, fire the gun”). We sometimes refer to these collections of actions as behaviors. Behaviors are roughly analogous to molecules (Figure 14.1). They are often composed of multiple actions (atoms). Some behaviors have many actions; some only have a few. Some actions combine well together to make a stable, understandable behavior; other actions don’t bind quite as readily to each other (e.g., “draw the gun, throw the gun up in the air, pick the flower, smell the flower, aim the flower, eat the flower”). Choices and actions are the atoms we use to make up those behavior molecules. The subatomic particles—the electrons, protons, and neutrons—in my obscure metaphor are the bits and pieces that we use to construct the individual decision.

*FIGURE 14.1 Choices and actions are the atoms of game AI. We think of them as the smallest building blocks of character behavior.*

These include the tools we have covered so far in this book: value, utility, formulas, response curves, scales, granularity, and weighted sums, just to name a few. While, like the atom, we can’t construct the decision without them, we don’t think about the pieces and parts outside the context of the decision itself. For example, a response curve doesn’t have much meaning outside the context of a decision that utilizes it. Naturally, we need to understand how these tools work and how they combine. We need to understand their dynamics and how they affect the bigger picture. The entire existence of those parts, however, is given meaning by their role in forming those atoms—those decisions that our AI agents need. Along the way through this book, we have illustrated many individual decisionmaking processes through our examples. Most of those were specifically constructed examples so that we could use the tool we were learning about in a context that was more familiar to us than simply an abstract theory or dry description. We will revisit some of these decisions and craft new ones throughout this chapter. Our goal is to begin to put everything we have covered into one decision-making process. Because of this, much of this chapter (and the next) falls into our familiar “In the Game” category. Remember, while a particular example may be of a specific behavior or endemic to a stereotypical genre, it is the decision process that is important to learn. Strange as it may sound, deciding what weapon to use in a role-playing game (RPG) is not all that different from deciding what attraction to visit in a theme-park-style game. Deciding whom to shoot or where to hide in a first-person shooter (FPS) is similar in many respects to deciding to whom to pass the ball in a sports simulation. The genres are different, the situations are different, and the behaviors are different. However, the choices our agents are making in those examples are similar, just as the atoms that make up wildly dissimilar substances are made of the same components. And the tools that we use to arrive at those decisions are definitely the same. A response curve is a response curve just as an electron is an electron. The bottom line is that, while a particular example may not seem similar to a challenge we face in our own game, the process may certainly be what the proverbial doctor ordered. (And I give you 80% odds that the process is sugarless.)

### DECIDING WHAT TO DECIDE

We need to go through a number of steps to construct a decision-making algorithm. Because our atom is a single decision, it is naturally the place to focus on. Once we have made some decisions, we can assemble them into behaviors. In fact, making a single decision about an action often makes a decision about a behavior as well.

For example, a decision to “attack Bad Dude with our gun” means we have made a decision about which enemy to attack, which weapon to use, that we need to draw it, aim it, fire it, and so on. All of those other actions are included in the key decision of “attack Bad Dude with our gun.” The reason we view “attack Bad Dude with our gun” as a single decision is that we are processing all of the components as a whole. We could have put it up against “attack Bad Dude with our fists,” “attack Evil Dude with our gun,” or even “attack Evil Dude with our fists.” We were not breaking down the decision into “attack Bad Dude or Evil Dude?” or “attack with gun or fists?” While we certainly could have divided the quandary into two separate parts (i.e., who to attack and how), we may want to score the decision based on the combination of the criteria. For example, if we compare the threats posed by Bad Dude and Evil Dude, we may find that Evil Dude is more of a threat (Figure 14.2). If we compare the relative strengths of our gun and our fists, we will likely find that our gun is a more potent weapon. Those two observations may lead us to attack Evil Dude with our gun (in the library?).

*FIGURE 14.2 If Bad Dude has a particular weakness to fist attacks, separating the two decisions would not have brought us to that choice. Only by combining the factors into a single decision algorithm would we have discovered the correct action.*

If we were to combine the target and weapon decisions together into one (mysterious) utility equation, however, we might find that Bad Dude has a weakness for melee attacks and that our best option (of the four combinations) would be to attack him with our fists. By making two separate decisions and gluing them together, we arrived at a suboptimal behavior. By combining them, we determined the best choice for the situation.

More or Less? The way around this is for us to decide what our decision is going to entail. One action? Two? A whole cluster of them? The more actions we lump into one decision, the more complex the decision becomes. On the other hand, the fewer actions we group into one decision, the more actual decisions we need to make. We also need to be careful not to run into logical pitfalls such as the one illustrated above. As always, the rationale for any given combination is very context-dependent. Most of the time, we want to group actions together if they are strongly related. For example, walking to an object is strongly related to the decision to pick the object up. We wouldn’t separate those two actions. The statements “should I pick up the box?” and “should I walk to box?” sound odd together. If we connect the answer to the first question to the second with “therefore,” however, it makes a lot more sense to us. “Should I pick up the box? Yes. Therefore, I should walk to the box.” The decision (such as it is) to walk to the box is a necessary component of the decision to pick up the box. We can’t pick it up if we don’t walk to it. (You also can’t pick up the box if you can’t walk to it.) Of course, we could not have even considered picking the box up if the assumption wasn’t there that we were going to walk to it. The two actions are almost inextricably linked—which means they should be considered in one decision.

### ANALYZING A SINGLE OPTION

Once we settle on what a decision entails, we need to analyze it. By sifting all of the relevant information through the tools we’ve discussed, we can begin to home in on the right decision.

### I NTHE GAME

Which Dude to Kill?

As our example throughout this next section, we define Evil Dude and Bad Dude as types of antagonistic dudes. We will also add a boss type, Arch Dude, to the mix. (If it helps complete the picture, we can imagine all the Dudes in dark glasses and stupid hats.) There are four types of weapons that we and the Dudes can arm ourselves with: a pistol, a shotgun, a machine gun, and a rocket launcher. The decision we need to make is, when confronted by dudes of various types, armed with any of the available weapons, and at various distances from us, which of the three should we attack first (Figure 14.3)?

*FIGURE 14.3 When our agent is confronted by Dudes of varying types, armed with any of the four weapons, and at a variety of distances, he must select which of the Dudes to attack and with which weapon.*

### IDENTIFYING FACTORS

Beginning as early as Chapter 2, we talked about the necessity of identifying and analyzing all the factors relevant to a decision. Throughout the examples in the book, we limited ourselves to the ones that helped illustrate the point we were trying to make at that time. We are now going to revisit this idea. To determine which factors are relevant to our pending Dude-icide, we need to make a list of things that could help or hinder our ability to successfully attack a Dude in general. After long consideration, here is the list we will use.

Distance to enemy: How far away from us is our target? This relates to the weapon range (below) as well as the threat factor to our own safety (also below). Our weapon range: How far away can we shoot with our current weapon? Our weapon damage: How much damage per second does our current weapon deal? Is the amount of damage related to the distance? Our weapon accuracy: How accurate is our current weapon? Is the accuracy related to distance? Our health: How much damage can we take? Opponent’s weapon range: From how far away can they shoot us?

Opponent’s weapon damage: How much damage does the opponent’s weapon do per second? Is it related to the distance of the shot? Opponent’s weapon accuracy: How accurate is the opponent with his weapon? Is the accuracy related to distance? Opponent’s health: How much damage can the target take?

While there could be other considerations, we are going to stop there for the moment. We can always add more fun stuff later on. As we discussed in the previous chapter, we need to determine if each criterion is concrete or abstract, what range they will fall into, and at what granularity we are measuring. By doing this, we get a better idea of what we are working with. We need to know the shape of each piece before we can start fitting them together. A quick glance through the list tells us that all of the criteria are concrete values. They are nonsubjective, measurable values. In fact, they are all values that are either listed as a property of an object (such as weapon damage) or that the game engine can calculate for us (such as distance). This simplifies our process somewhat for now.

Here a Dude, There a Dude, Everywhere a Dude, Dude… To give us a better idea of the concrete data we are working with, we need to list the specifics for each of the three types of Dudes. Each of the three has an amount of health, with the Arch Dude being able to absorb the most damage. Additionally, they have accuracy modifiers that adjust their ability to shoot their weapons. Being the least trained, the Bad Dudes are… well… bad. The Arch Dudes, on the other hand, have a bonus to their weapon accuracy.

| Dude Type | Health | Acc. Mod. |
| --- | --- | --- |
| Bad | 100 | –20% |
| Evil | 120 | 0% |
| Arch | 150 | +10% |

Choose Your Weapon Our agent and the Dudes can use any one of four weapons. These are a pistol, a shotgun, a machine gun, and a rocket launcher. Each of the four types has its own accuracy and damage-dealing characteristics. The weapons start with a base damage and accuracy rate. Rather than a simple concrete number, however, these values are distance-dependent.

To construct these formulas, we use some of the suggestions in Chapter 10. To exhibit the characteristic of decreasing damage, we need to have a formula that was at its maximum result at a distance of 0 (with one exception, as we shall see). From there, we want it to fall away at an increasing rate. The natural starting point was a parabolic curve that we subtract from our maximum point. All four weapons use the same base formula. This ensures that the general, distance dependency characteristic is present. Each weapon has specific values for each variable, however, which is what separates one weapon from another. The formulas are

Notice that the structure of the two formulas is the same. The magic of each happens with the numbers that we plug in. There are a few things to note, however. First, it is possible that the formula can generate a number less than 0. As we shall see, this is by design. Because we can’t have negative damage or negative accuracy, we need to clamp the result to a minimum of 0. As cryptic as the formulas look in this form, they begin to make more sense as we plug in the weapon-specific values. The figures for each weapon are:

| Value | Pistol | Shotgun | Mach. Gun | Rocket L. |
| --- | --- | --- | --- | --- |
| Range | 137 | 57 | 300 | 300 |
| Base Damage (/sec) | 10 | 50 | 30 | 100 |
| Dmg. Decay Exp. | 2.2 | 2.1 | 2.0 | 2.0 |
| Dmg. Decay Divisor | 5,000 | 100 | 5,000 | 1,500 |
| Dmg. Decay Shift | 0 | 0 | 0 | 50 |
| Base Accuracy | 0.70 | 0.95 | 0.80 | 0.50 |
| Acc. Decay Exp. | 1.5 | 2.2 | 1.8 | 2.0 |
| Acc. Decay Divisor | 3,000 | 15,000 | 50,000 | 30,000 |
| Acc. Decay Shift | 0 | 0 | 0 | 50 |

Unfortunately, looking at the figures in the table doesn’t make them any less mysterious. To shed a little more light on how they operate together, we could use one of the weapons an example. We will leave the Distance and Modifier values blank for now, as those would be specific to the situation.

A word of caution for those of us who seek to infer meaning from things: The numbers used in this example are neither specifically related to anything nor drawn from anything. Often, in the search for an effect for a mathematical model, we select these numbers through a trial-and-error approach. There is no “correct way” to approach this process. We end up using “whatever works.”

Using the formula and numbers above, the damage calculation for a machine gun is:

Assuming that the machine gun is in the hands of an Evil Dude (no modifiers) at a range of 100 feet, we can now calculate how much damage the machine gun will do per second.

As we can see, the travel distance of 100 reduces the damage done by the machine gun by 2 points down to 28.0. If we extend the shot further, to 200 feet, we would find that the damage is reduced further, to 22.0.

The formula for accuracy works the same way. One aspect not shown above is the Modifier for the skill of user. The Modifier parameter directly affects the starting point of the curve. We can see this by comparing the accuracy rates for a Bad Dude and an Evil Dude. The Bad Dude has a –20% modifier to accuracy. Therefore, his accuracy with a pistol at 75 feet would be

On the other hand, the accuracy for an Evil Dude with a pistol at 75 feet would be

The only difference between the two is the inclusion of the Modifier value. The other aspect that we haven’t seen in action yet is the Shift parameter. If we think back to Chapter 10, we will remember that it is possible to shift the vertex of a parabola left or right. We do this by adjusting the value of x under the exponent. If we realize that Distance in our formula is the equivalent of x, then the placement of the Shift parameter along with Distance explains the horizontal movement of the curve. For example, the peak accuracy of a rocket launcher is not at 0 feet. By specifying that Shift = 50, we ensure that the peak accuracy of the rocket launcher (i.e., the vertex of the parabola) is at 50 feet. At 20 feet, for example, the accuracy of the rocket launcher is only 47%—down from its peak of 50%.

All of this data is better visualized (and more easily constructed) by looking at graphs. Figure 14.4 shows the accuracy of the four weapons based on the data for each inserted into the accuracy formula.

*FIGURE 14.4 The accuracy curves of the four weapons are a result of the data for the respective weapons entered into the accuracy equation.*

By looking at the accuracy curves on the same graph, we can see not only how each weapon performs over the given range, but also how they perform compared to each other. For example, while the shotgun is the most accurate close-range weapon (due to the scatter effect), its accuracy falls off dramatically as the distance increases. On the other hand, the accuracy of the machine gun remains relatively good over the range of the graph. Of particular note is the graph of the rocket launcher. As we mentioned above, the Shift parameter moved the vertex of the parabola to the right. Rather than having a peak accuracy at a range of 0, we can see that it is at its best at a range of 50 (the Shift value for a rocket launcher). Both nearer and farther than 50 feet, its accuracy decreases. We can view the results of the damage formula on a graph as well (Figure 14.5). The range of the graph is the same as in the accuracy graph (Figure 14.4). Again, we can see the telltale parabolas of the quadratic equation.

*FIGURE 14.5 The damage curves of the four weapons are a result of the data for the respective weapons entered into the damage equation.*

For what should be obvious reasons, the rocket launcher is the most potent of the four weapons. The lowly pistol is on the low end of the range. It is more important to note the effects of the different shapes of the damage curves as the range increases. As we would expect, a shotgun blast is fairly potent at close range. As the range increases, however, a shotgun blast loses much of its kick. In fact, at about 50 feet, it would be less powerful (per second) than being struck by a bullet from a pistol. At 60 feet, the shotgun blast does no damage whatsoever.

Whereas each of the two formulas gives us valid and important information about the four weapons, we learn a lot more about the effectiveness of the weapons when we combine the graphs. By simply multiplying the damage per second by the percent chance of scoring a hit, we arrive at a new figure: expected damage per second. We can then graph this combination of data in the same manner as either accuracy or damage alone (Figure 14.6).

*FIGURE 14.6 By multiplying the damage by the accuracy rate, we can calculate the overall effectiveness of a weapon at different ranges.*

Once again, analyzing the four curves provides us with some interesting information. First, as we would expect, the high accuracy and high damage rates make the shotgun the weapon of choice at close range. While the rocket launcher certainly packs a punch close in, its accuracy rate causes it to be slightly unreliable. On the other hand, because the accuracy and damage rates of the shotgun drop so quickly, its effectiveness drops swiftly as well. That leaves the rocket launcher as a prime weapon for mid-range strikes. However, because of the poor long-range accuracy of the rockets, its formidable damage rate becomes less important as range increases. Eventually, the machine gun’s reliable accuracy and moderate damage-dealing capability wins out. For longer-range strikes, it becomes the weapon of choice. The pistol may not look impressive in the company of the other, more powerful weapons. However, we have to consider that we may not always have access to (or ammo for) the other weapons. If we only had a shotgun and a pistol, for instance, we would elect to use the pistol at ranges of over 50 feet. If we had a machine gun (and no rocket launcher), we would elect to use it instead of the shotgun for ranges of over 35 feet.

It’s important for us to remember that we are not the only one with a gun. The Dudes are armed as well. Other than the accuracy modifiers that we identified above, the Dudes’ weapons perform identically to ours.

Did I Mention the Detonator? The above information helps us determine what the optimum weapon is for each range. That goes a long way toward helping us make our decision. If we know the range to each Dude, his health, and what weapon he is carrying, we can determine which Dude to attack first and what the optimum weapon is for us to attack him with. The solution is to determine which Dude is some combination of the biggest threat and the easiest kill. However, before we go further, we are going to add one last wrinkle to our example. We will now assume that there is an important point in the area. In true epic James Bond style, we will say that it is a detonator for a large explosive device. (On second thought, this could be in Austin Powers style, too. Or Jack Bauer style. Or Jedi Knight sty–… never mind.) We now have two goals to address. First, as before, we need to avoid allowing ourselves to be killed by a Dude. That is what we were addressing above when we were going to dispatch the biggest threat–easiest kill combination. However, our second goal is to prevent someone from triggering the detonator. By adding a parameter to each Dude “range to goal,” we can determine who is the most dangerous target in that respect. The two priority systems may not yield the same answer. For example, a Dude who we may have judged as the lowest-priority target before may suddenly become extremely important to attack if he moves close to the detonator (Figure 14.7).

*FIGURE 14.7 The poor fighter (Bad Dude) with a poor weapon (shotgun) at long range would normally be the lowest-priority target. If he is standing next to the dreaded detonator, however, his priority as a target increases significantly.*

We need to include a way of adding that priority into our target selection algorithm. Before we do that, however, we need to define what “close to” and “increased priority” mean. To do that, we construct another formula. Once again, we tap into a type of formula from Chapter 10. We will define Urgency as the result of a formula with an exponent that is less than 1 (a root). By subtracting from the high value, 1.0, we arrange it so that as distance from the detonator increases, the urgency of the target drops away from 1.0. The formula we will use is

Once again, the effect of this formula is easier to visualize as a graph (Figure 14.8). In this case, we are using a parabola to simulate the rise in Urgency as the range decreases. The nature of an exponent-based curve is such that the range of change is very significant near the vertex (Range = 0, Urgency = 1.0). While there is an increase in Urgency as the distance diminishes throughout the entire range of the graph, the rate of change increases markedly as the distance approaches 0.

*FIGURE 14.8 As the range to the detonator increases, the urgency level for the target drops. As the target’s range to the detonator decreases, especially as it closes within 25 feet, the urgency rises rapidly.*

A Jumble of Blocks We have now defined all of the pieces and parts that we will use in our decision. We have not only set values for the various Dudes that we will encounter, but we have established formulas for calculating more complex (yet still concrete) values such as the range-based damage and accuracy figures. However, none of these parts work together. We have lots of facts and formulas, but no cohesion.

Before we start putting these blocks together, we need to ensure that we have them built correctly. To establish the compartmentalized confidence we talked about in Chapter 13, we need to ensure that we are comfortable with each component. Only then can we trust that what we build with the blocks will be valid.

### PUTTING I TIN CODE

There are three major components in this example: the agent (us), Dudes, and weapons. Accordingly, we create classes for the three types of entities (CAgent, CDude, and CWeapon). Additionally, for Dudes and weapons, we create collection classes to hold the individual objects (Figure 14.9). We will look at the basics of each of the classes for clarity.

*FIGURE 14.9 The class structure for the “Shoot Dudes” example. CDudeCollection contains a vector of CDude objects. CWeaponCollection contains an array of four CWeapon objects.*

This logical arrangement is optional, of course. This book is not meant to be an educational tome on design patterns or memory management. I have stripped the example down to a simple, easy-to-understand model. Feel free to insert the AI logic into a design of your own choosing.

Weapons Because we utilize CWeapon in both CDude and CAgent, we will begin by defining it. First, we should note that we have enumerated a type to make referencing the weapons easier throughout the entire program.

typedef enum {

WEAPON_PISTOL,

WEAPON_SHOTGUN,

WEAPON_MACHINEGUN,

WEAPON_ROCKETS

} WEAPON_TYPE;

The header of CWeapon is self-explanatory. (For space purposes, I have cut out the constructor, destructor, and the usual “get and set” accessor functions.) class CWeapon

{

public:

//[Ctor/Dtor snipped for space…]

//[Accessors snipped for space…]

///////////////////

// Accuracy and Damage Calculations

///////////////////

double GetAccuracy( USHORT Dist = 0, double Modifier = 0.0 );

double GetDamage( USHORT Dist = 0, double Modifier = 0.0 );

private:

///////////////////

// Member Variables

///////////////////

char* mName; // Name

USHORT mMaxRange; // Max range

USHORT mBaseDamage; // Base (max) damage

double mDmgDecayExp; // Decay formula exponent

USHORT mDmgDecayDiv; // Decay formula divisor

USHORT mDmgDecayShift; // Decay formula shift (horiz.)

double mBaseAccuracy; // Base (max) accuracy

double mAccDecayExp; // Decay formula exponent

USHORT mAccDecayDiv; // Decay formula divisor

USHORT mAccDecayShift; // Decay formula shift (horiz.)

};

The member variables for the weapons hold the numbers that we used for the damage and accuracy calculations. For both damage and accuracy, we have a base, a decay exponent, a decay divisor, and a decay shift. Based on the formulas we laid out above, those are the only figures that we need to define the appropriate curves. By calling GetDamage() and GetAccuracy() with the distance and any modifiers (e.g., the accuracy modifiers for the different types of Dudes), we receive the appropriate damage value or accuracy percentage. These two functions operate very simply. double CWeapon::GetAccuracy( USHORT Dist /*= 0*/,

double Modifier /*= 0.0 */ )

{

double Accuracy = mBaseAccuracy + Modifier -

(pow(( Dist - mAccDecayShift ), mAccDecayExp )) / mAccDecayDiv;

if ( Accuracy < 0 ) { Accuracy = 0; }

return Accuracy;

}

double CWeapon::GetDamage( USHORT Dist /*= 0*/,

double Modifier /*= 0.0 */ )

{

double Damage = mBaseDamage + Modifier -

(pow(( Dist - mDmgDecayShift ), mDmgDecayExp )) / mDmgDecayDiv;

if ( Damage < 0 ) { Damage = 0; }

return Damage;

}

Notice that, because our formulas can generate values that are less than 0, we must clamp the minimum on both values to 0. We can’t do negative damage, and we can’t have less than a 0% chance of hitting our target. Other than that, the formulas for calculating the values are familiar as code-ese translations of their mathematical versions we expressed above. We define an array of CWeapon objects in the CWeaponCollection as follows: #define MAX_WEAPONS 4

CWeapon mWeapons[MAX_WEAPONS]; // Array of all weapons

CWeaponCollection also has accessors for looking up information about any of the members of its array by index. These accessors simply pass the request on to the matching accessor of the referenced object. The important function that CWeaponCollection performs, however, is to initialize the array with the four weapons we have defined in our game. We do this through the function InitOneWeapon(). void CWeaponCollection::InitOneWeapon( WEAPON_TYPE Type,

char* Name,

USHORT MaxRange,

USHORT BaseDamage,

double DmgDecayExp,

USHORT DmgDecayDiv,

USHORT DmgDecayShift,

double BaseAccuracy,

double AccDecayExp,

USHORT AccDecayDiv,

USHORT AccDecayShift )

{

mWeapons[Type].SetMaxRange( MaxRange );

mWeapons[Type].SetBaseDamage( BaseDamage );

mWeapons[Type].SetDmgDecayExp( DmgDecayExp );

mWeapons[Type].SetDamageDecayDiv( DmgDecayDiv );

mWeapons[Type].SetDamageDecayShift( DmgDecayShift );

mWeapons[Type].SetBaseAccuracy( BaseAccuracy );

mWeapons[Type].SetAccDecayExp( AccDecayExp );

mWeapons[Type].SetAccDecayDiv( AccDecayDiv );

mWeapons[Type].SetAccDecayShift( AccDecayShift );

mWeapons[Type].SetName( Name );

}

In the function InitWeapons(), we call InitOneWeapon() once for each of our four weapons. void CWeaponCollection::InitWeapons()

{

InitOneWeapon( WEAPON_PISTOL, “Pistol”, 137,

10, 2.2, 5000, 0, // Damage

0.7, 1.5, 3000, 0 ); // Accuracy

InitOneWeapon( WEAPON_SHOTGUN, “Shotgun”, 57,

50, 2.1, 100, 0, // Damage

0.95, 2.2, 15000, 0 ); // Accuracy

InitOneWeapon( WEAPON_MACHINEGUN, “M/G”, 300,

30, 2.0, 5000, 0, // Damage

0.8, 1.8, 50000, 0 ); // Accuracy

InitOneWeapon( WEAPON_ROCKETS, “R/L”, 300,

100, 2.0, 1500, 50, // Damage

0.5, 2.0, 30000, 50 ); // Accuracy

}

In each call to InitOneWeapon(), we are passing in the numbers from the characteristics table that we laid out earlier. Once we have defined the weapon parameters, the damage and accuracy functions have all the information that they need to do their magic. If we want to change the characteristics of one weapon, we simply change the initialization parameters that we pass in when we create the CWeapon object. The formula does the rest. (In a production environment, we would store these values in a configuration file so that we can tweak them without touching the code.)

What Makes a Dude a Dude? We now turn our attention to CDude and CDudeCollection. There really isn’t a lot to CDude in our implementation. As we did with the weapons, we define an enumerated type for the three different types of Dudes. typedef enum {

BAD_DUDE,

EVIL_DUDE,

ARCH_DUDE

} DUDE_TYPE;

The header file contains the usual suspects. The constructor takes the arguments that set all the member variables. (Again, I have snipped the accessor functions.) class CDude

{

public:

///////////////////

// Ctor/Dtor

///////////////////

CDude( DUDE_TYPE Type,

char* Name,

USHORT Health,

USHORT Location,

USHORT DistToGoal,

CWeapon* pWeapon );

virtual ~CDude();

//[Accessors snipped for space…]

void SetAccAdjust();

private:

///////////////////

// Member Variables

///////////////////

char* mName; // Name of Dude

DUDE_TYPE mType; // Dude Type

USHORT mHealth; // Health of this Dude

USHORT mLocation; // Location (1D) of this Dude

USHORT mDistToGoal; // Distance to the goal

double mAccAdjust; // Accuracy adjustment

CWeapon* mpWeapon; // Pointer to weapon armed

};

Please note that, for purposes of our example, we are tracking the positions of the Dudes and our agent in one dimension only. Therefore, the distance between them is simply a linear measurement on a single axis. The only function in CDude that is not precisely an accessor is SetAccAdjust(). When a Dude is created, the constructer calls SetAccAdjust() to set the accuracy adjustment based on the type of Dude. void CDude::SetAccAdjust()

{

switch( mType ) {

case BAD_DUDE:

mAccAdjust = -0.2;

break;

case EVIL_DUDE:

mAccAdjust = 0.0;

break;

case ARCH_DUDE:

mAccAdjust = 0.1;

break;

default:

mAccAdjust = 0.0;

break;

} // end switch

}

The CDudeCollection is just as bland. It contains, as its member variable, a vector containing CDude objects. typedef std::vector < CDude > DUDE_LIST;

DUDE_LIST mlDudes; // List of Dudes in the game

We fill this vector with our Dudes when the constructor for CDudeCollection calls InitDudes() which, in turn, calls InitOneDude().

void CDudeCollection::InitDudes()

{

InitOneDude( BAD_DUDE, “Baddie”,

100, 180, 50, WEAPON_SHOTGUN );

InitOneDude( EVIL_DUDE, “Evilmeister”,

120, 200, 150, WEAPON_MACHINEGUN );

InitOneDude( ARCH_DUDE, “Boss Man”,

150, 150, 110, WEAPON_ROCKETS );

}

void CDudeCollection::InitOneDude( DUDE_TYPE Type,

char* Name,

USHORT Health,

USHORT Location,

USHORT DistToGoal,

WEAPON_TYPE Weapon )

{

CWeapon* pWeapon = mpWeaponCollection->GetWeaponPointer( Weapon );

CDude NewDude ( Type, Name, Health, Location, DistToGoal, pWeapon );

mlDudes.push_back( NewDude );

}

The Mind of an Agent The last class we need to establish is CAgent. This is where our AI decision-making process will be taking place, of course. Before that, however, we need to establish what our class looks like. The CAgent class uses a pair of structs to help track its information. The first one is for keeping track of the weapons that we have on our person. struct sWEAPON_INFO

{

CWeapon* pWeapon;

USHORT Ammo;

};

typedef std::vector < sWEAPON_INFO > WEAPON_VECTOR;

Each item in a vector of type WEAPON_LIST contains a pointer to a weapon and a record of the amount of ammunition that we carry for this weapon. We also have a similar structure for our list of targets. struct sTARGET_INFO

{

CDude* pDude;

CWeapon* pWeapon;

double Score;

bool operator<( const sTARGET_INFO& j ) {return Score > j.Score;}

};

typedef std::vector < sTARGET_INFO > TARGET_VECTOR;

The information contained in sTARGET_INFO may be confusing at first until we discuss what our decision process will entail. We will be making a choice that is actually two in one. We must decide the best combination of which enemy we are going to kill and with what weapon. As we touched on earlier in this chapter, deciding on our target separately from the weapon we want to use may lead us to a false “best solution.” Therefore, we have to address them as part of the same decision process. Each entry that we place into a TARGET_LIST will be a combination of an enemy and a weapon. We will then score that combination individually and select the best entry.

We overload the < operator for the std::sort algorithm because we have to explain to the compiler that we are sorting by a particular member of the sTARGET_INFO (specifically, Score). Rather than list the entire header file here and spoil the surprise, we will look only at a few key items of CAgent. First, there are no surprises in the member variables. USHORT mHealth; // Current Health

USHORT mLocation; // Current Location in 1D

TARGET_LIST mvTargets; // List of available targets

WEAPON_LIST mvWeapons; // List of available weapons

CDude* mpCurrentTarget; // Current target to attack

WEAPON_TYPE mCurrentWeapon; // Current weapon to use

We have defined two member list variables—one for our current targets and one for our current weapons. When the agent needs the distance to a selected enemy, we call the function

GetDistToTarget( CDude* pTarget )

The implementation is simply USHORT CAgent::GetDistToTarget(CDude *pTarget)

{

USHORT Dist = abs( mLocation - pTarget->GetLocation() );

return Dist;

}

The constructor for CAgent sets mLocation = 0 and mHealth = 100. Its other function is to initialize our weapons. We do this through the functions InitWeapons() and AddWeapon().

void CAgent::InitWeapons()

{

AddWeapon( WEAPON_PISTOL, 20 );

AddWeapon( WEAPON_MACHINEGUN, 100 );

AddWeapon( WEAPON_SHOTGUN, 10 );

AddWeapon( WEAPON_ROCKETS, 6 );

}

void CAgent::AddWeapon( WEAPON_TYPE WeaponType, USHORT Ammo )

{

sWEAPON_INFO ThisWeapon;

ThisWeapon.Ammo = Ammo;

ThisWeapon.pWeapon =

mpWeaponCollection->GetWeaponPointer( WeaponType );

mvWeapons.push_back( ThisWeapon );

}

All Dressed Up and No One to Kill By this point, we have created our objects and filled our lists. Our weapons have stats and functions describing their abilities, our enemies have stats and weapons, and our agent has stats, weapons, and enemies. None of that veritable cornucopia of information, however, yields a decision on its own. All we have is information. Even the nifty formulas that calculate the accuracy and damage that the weapons cause are only different forms of information. We have assembled the factors that we believe will be important to our decision just as a glance at the ingredient list for a cake tells us what we need to have handy. It does not tell us how we should combine those ingredients or what we need to do with them once they are in the same bowl. For that, we need to begin to identify how these components will work together.

### IDENTIFYING RELATIONSHIPS

The first step we must take in combining our data into something meaningful is to identify the items that are directly related. We already touched on a few simple relationships. For example, our formulas for weapon damage and accuracy use distance as one of their components. Therefore, if we know the distance to a target, we can enter that into the formula for a particular weapon and generate a result. We have established a relationship between our location, that of our target, and the weapon.

Another relationship we identified earlier also involved the weapons. The strength of a weapon is neither based only on the damage that it can do nor only on the accuracy with which it can be used. We combine these two factors into a single measurement. We measure it as the amount of damage that a weapon can do per second when we factor in the probability of a hit. In the graph in Figure 14.6, we labeled this “effectiveness.” By combining these two pieces of data into one concept, we establish a relationship. We can add another component to this relationship. This one makes it far more useful and relevant to the decision we are trying to make. By taking into account the health of the target that we are considering, we can determine how long it would take for us to kill that target with a given weapon at a given distance.

Of course, what is missing in the above equation is that Accuracy and Damage are the results of the distance-based formulas for the weapons. Figure 14.10 shows the entire cascade of relationships that leads us to the TimeToKill value.

*FIGURE 14.10 As we add more relationships between our individual components of data, we create data that is more useful to our overall decision process.*

### PUTTING I TIN CODE

The code for this relationship is as simple as it seems. We create a function that takes the accuracy rate, the damage rate, and the target’s health and combines them into one figure based on the formula above.

double CAgent::TimeToKill( USHORT Health,

USHORT Damage,

double Accuracy )

{

USHORT TimeToKill;

double DamagePerSec = ( Accuracy * Damage );

// avoid divide by zero

if ( DamagePerSec != 0 ) {

TimeToKill = Health / DamagePerSec;

} else {

// if damage/sec is 0, clamp time to 1000

TimeToKill = 1000;

} // end if

return TimeToKill;

}

Notice that the process in this function is broken into two steps so that we can avoid “division by zero” errors. We are serving another purpose here. If the amount of damage per second that we can inflict is 0, there is no way that we can injure, much less kill the target with the selected weapon. We want to exclude this targetweapon combination from reasonable consideration. By setting TimeToKill to an absurd value such as 1,000, we are ensuring that it will significantly reduce any further calculations that we perform with it. However, we are not removing it from consideration entirely. If we find ourselves in a situation where none of the weapons are effective against any of the targets, we would still want to use the best combination to at least do some damage. Processing this relationship is as simple as sending the appropriate data into the function. The data comes from a few simple functions as well. First, we have to retrieve the accuracy and damage rates based on the weapon and distance. The following functions provide that information for us. USHORT CAgent::GetDamage( CWeapon* pWeapon, USHORT Dist )

{

USHORT Damage = pWeapon->GetDamage( Dist );

return Damage;

}

double CAgent::GetAccuracy( CWeapon* pWeapon, USHORT Dist )

{

double Accuracy = pWeapon->GetAccuracy( Dist );

return Accuracy;

}

Therefore, the entire process for determining the time it takes to dispatch a particular enemy is: USHORT Dist = GetDistToTarget( pTarget );

USHORT TargetHealth = pTarget->GetHealth();

USHORT DamageIDeal = GetDamage( pWeapon, Dist );

double MyAccuracy = GetAccuracy( pWeapon, Dist );

double TimeToKillEnemy = TimeToKill( TargetHealth,

DamageIDeal,

MyAccuracy );

What Goes ’Round Comes ’Round Because we know what weapon our enemy is carrying, his accuracy rate (adjusted for the type of Dude if necessary), and our own health, we can reverse the above process and determine how long it would take for a particular enemy to dispatch us. The only difference in the functions is that GetAccuracy() must account for the Dude adjustment. By passing a pointer to the selected enemy into an overloaded version of GetAccuracy(), we can add that bit of additional information. double CAgent::GetAccuracy( CWeapon* pWeapon,

USHORT Dist,

CDude* pDude )

{

// Get Dude’s accuracy adjustment

double AccAdjust = pDude->GetAccAdjust();

// Get weapon accuracy adjusted for Dude’s skill

double Accuracy = pWeapon->GetAccuracy( Dist, AccAdjust );

return Accuracy;

}

Assembling the data requires a similar set of function calls.

double DamageITake = GetDamage( pTarget->GetWeapon(), Dist );

double EnemyAccuracy =

GetAccuracy( pTarget->GetWeapon(),

Dist,

pTarget->GetPointer() );

double TimeToKillMe = TimeToKill( mHealth,

DamageITake,

EnemyAccuracy );

Through the processes above, we know how long it will take for us to kill a selected enemy and how long it will take for him to kill us… but now what? What do we do with this information? As we discussed in the first half of the book, the utility of an action is not always as obvious as we would like it to be. Sometimes we have to add a little bit of subjectivity as well.

### BUILDING CONNECTIONS

Certainly, each of the two values, TimeToKillEnemy and TimeToKillMe, is important. It is helpful to know how long it would take us to mow down a Dude with a particular weapon. It is also helpful to know how long we can expect to survive an onslaught from a particular Dude. However, there is not an inherent “cause and effect” relationship between TimeToKillEnemy and TimeToKillMe. We have to define a meaningful connection between the two. As with our simple example earlier, we cannot base our decision solely on one or the other of these two factors. Anecdotally, if we select the Dude we can kill the quickest, we may be exposing ourselves to a Dude who can kill us the quickest. Alternatively, if we elect to assault the one who is the greatest threat, we may be overlooking the fact that we could have quickly dismissed his weaker pals. What we must do is build a connection between these two independent values that expresses a combined utility to us.

In Chapter 9, we explored the idea of relative utility. The decision that we often had to make at each step of the process was to decide if two items or actions were equally important or if one was more desirable than the other. We also had to decide how much more important one selection was than the other. This leads to a decision on how to weigh the utility of each action (Chapter 13). This is what we must do now to build a connection between the independent values of TimeToKillEnemy and TimeToKillMe. Certainly, we would prefer a situation where it takes us less time to vanquish our foe than it does for him to annihilate us. On the other hand, we also want to pick off the targets that are quick for us to kill. If we get them out of our hair, we have less to worry about. We will call the figure we are calculating ThreatRatio. After experimenting with various combinations, we can arrive at something that “feels” reasonable. We will define ThreatRatio with the following formula:

By multiplying TimeToKillEnemy by 3, we are making it three times as important as TimeToKillMe. By dividing the weighted sum by 4, we are converting it to a weighted average. It’s worth noting that we are, once again, relying on the principle of compartmentalized confidence. We trust that the processes that led us to the two “time to kill” values are valid. With that trust, we can turn all of our exploration and experimentation to the components we are adding—the numbers involved in the weighted sum. By selecting a few examples, we can visualize the effect of our formula. For now, we will ignore how we arrived at these numbers (because we are confident, remember?) and instead focus only on the effect of the weighted average.

| TimeToKillMe | TimeToKillEnemy | ThreatRatio |
| --- | --- | --- |
| 5.1 | 3.3 | 3.76 |
| 5.1 | 427.1 | 321.56 |
| 12.6 | 4.7 | 6.69 |
| 12.6 | 1,000.0 | 753.15 |
| 39.9 | 1.0 | 10.72 |
| 39.9 | 10.7 | 18.02 |

In the examples above, we can see a few extremes. The “best” score is the lowest, so we see that the first line is the best option for us to select. In that example, we are facing an enemy who can kill us more quickly than the other examples (5.1 seconds).

On the other hand, by using whatever weapon is represented in the first line, we will dispatch him in a fairly rapid 3.3 seconds. Further examination of the data shows how it is working. For example, the second line is the same dangerous enemy. However, our selection of weapon is less than stellar. It would take us a tragic 427 seconds to finish him off. The threat ratio score reflects this with a very poor result. On the other hand, if we had based our decision solely on how quickly we can kill an enemy, we would have elected to go with the fifth line. In that combination, we can kill the enemy in a single second. However, looking at the first column shows that dispatching that enemy is a pressing problem. We have almost a full 40 seconds in which to deal with him. One other item to note is the appearance of the 1,000-second time in the fourth line. This is due to a weapon choice where the damage per second would have been zero and we manually overrode the value. We still calculate a threat ratio for that selection in case we find ourselves in a hopeless situation where we must choose the “lesser of two evils.” The threat ratio of 753 effectively puts that selection out of the running, however.

Remember that Detonator? While the formula above successfully connects the two expected kill times into a single threat value, we still have the messy problem of the detonator to worry about. The connection between the time to kill values was relatively obvious. We could sum it up with the trite phrase “kill him before he kills you!” However, the Dude’s proximity to the detonator is less directly related. We could calculate the amount of time that it would take a Dude to reach the detonator based on the Dude’s speed and distance, but that assumes that the Dude is moving directly for it without anything else on his mind. Instead, because we constructed a utility curve that yielded the value, Urgency, we will use that as our sole measurement. As with the two “kill times” above, we need to construct an abstract connection between ThreatRatio and Urgency that meaningfully describes their relative merit in our overall decision. We begin by making an assertion that we believe is important: “If a Dude is close to the detonator, he becomes the most dangerous enemy.” While this assertion is indisputably relevant, it still doesn’t tell us how important the factor is. For example, when we say “if a Dude is close to the detonator,” what do we mean by “close?” The Urgency function already accounts for the phenomenon that “closer means more urgent,” but until we relate it to the threat ratio, the notion of “more urgent” is rather vague. As a result, we have no idea when the enemy would become the “most dangerous” one out there. That depends on a lot of factors.

If the Dudes arrayed before us all have the same threat ratio, a small tweak in the distance to the detonator could make the difference. On the other hand, if there is a large difference between the threats that the various Dudes present, it may not matter much how close any one of them is to the detonator. As with many occurrences of subjective ratings, we are left to the obscure and very nonscientific method known as “pulling numbers out of the air to see what works.” Thankfully, by performing a simple division, we can significantly affect our result as both of our two factors change. Consider the formula

Because we had already established that a lower ThreatRatio is more important, it helps for us to keep that arrangement. We also constructed Urgency so that the maximum value is 1. We want Score to be at its extreme (lowest) when Urgency = 1. By placing it in the denominator, we achieve this effect. As Urgency goes down, the value of the fraction would go up—that is, the target would become less of a priority. Likewise, if Urgency stayed the same and ThreatRatio increased (became less important), Score would increase as well—again, making the target less of a priority. Therefore, the arrangement of dividing ThreatRatio by Urgency achieves the desired effect.

### SCORING THE OPTION

We can run some numbers through the formula to ensure that this is working properly. Consider the following values:

| ThreatRatio | Urgency | Score |
| --- | --- | --- |
| 3.8 | 0.26 | 14.64 |
| 3.8 | 0.32 | 11.85 |
| 4.1 | 0.26 | 15.77 |
| 4.1 | 0.32 | 12.81 |
| 10.9 | 0.51 | 21.37 |
| 10.9 | 0.80 | 13.62 |

Again, as we peruse these examples, some things jump out at us. First, consider the first two lines. The threat ratio stays the same, but the urgency increases from the first to the second line. As we would expect, the value for the score decreases (or becomes more important). Using the same two urgency values but increasing the

threat ratio slightly shows that the resulting scores increase as well. However, we can see that a ThreatRatio of 4.1 and an Urgency of 0.32 yields a lower Score than the lower threat ratio (3.8) and lower Urgency (0.26). This exhibits the subtlety in the combination of the two values. In the last two lines, the threat of 10.9 isn’t terribly dangerous in and of itself. When combined with a significantly higher Urgency value (the last line), however, the final score falls into the same range as the more threatening situations. This would be comparable to a nonthreatening foe approaching the detonator. Whether he can kill us or not, he becomes a priority target. In fact, if we increase the Urgency of that last line to 0.95, the final score for that enemy becomes 11.47. That would make it the lowest score (i.e., highest priority) of the ones shown.

### PUTTING I TIN CODE

Assuming we are now comfortable with the relationships and connections we have established, we can now go about coding the scoring process. We looked at a portion of this process already when we laid out the function calls that generated the timeto-kill values. We will now put it into the whole function. void CAgent::ScoreTarget( CDude* pTarget, sWEAPON_INFO Weapon )

{

CWeapon* pWeapon = Weapon.pWeapon;

USHORT Dist = GetDistToTarget( pTarget );

// Calculate time to kill enemy

USHORT TargetHealth = pTarget->GetHealth();

USHORT DamageIDeal = GetDamage( pWeapon, Dist );

double MyAccuracy = GetAccuracy( pWeapon, Dist );

double TimeToKillEnemy = TimeToKill( TargetHealth,

DamageIDeal,

MyAccuracy );

// Calculate time for enemy to kill me

double DamageITake = GetDamage( pTarget->GetWeapon(), Dist );

double EnemyAccuracy =

GetAccuracy( pTarget->GetWeapon(),

Dist,

pTarget->GetPointer() );

double TimeToKillMe = TimeToKill( mHealth,

DamageITake,

EnemyAccuracy );

// Calculate threat ratio

double ThreatRatio = ( TimeToKillMe + ( 3 * TimeToKillEnemy ) ) / 4;

// Calculate target urgency based on proximity to the goal

double Urgency = CalcTargetUrgency( pTarget->GetDistToGoal() );

// Create and store the target information

sTARGET_INFO ThisTarget;

ThisTarget.pDude = pTarget;

ThisTarget.pWeapon = Weapon.pWeapon;

ThisTarget.Score = ThreatRatio / Urgency;

mvTargets.push_back( ThisTarget );

}

The function ScoreTarget() takes a pointer to a CDude and a weapon type as its parameters. We will see where these come from in a moment. The beginning of the function is familiar. First, we calculate the time it takes us to kill the selected enemy. Second, we calculate the same information in the other direction—the time it would take for the selected enemy to kill us. This is the portion we addressed above. Immediately after that, however, we calculate ThreatRatio according to the weighted average formula we defined earlier. We then calculate the urgency of the target based on his distance from the detonator. The function we call to do this is simple. double CAgent::CalcTargetUrgency( USHORT Dist )

{

double Urgency = 1 - ( pow( Dist, 0.4) / 10);

// Clamp negative urgency to 0

if ( Urgency < 0.0 ) {

Urgency = 0.0;

} // end if

return Urgency;

}

Once we have ThreatRatio and Urgency, we proceed with building the information about this target. Remember that sTARGET_INFO represents a combination of a Dude and a weapon that we are scoring for comparison purposes. We create a new instance of sTARGET_INFO, set the appropriate Dude and weapon information, and then save the score using the simple equation we decided on above.

ThisTarget.Score = ThreatRatio / Urgency;

We then push it onto the list of targets and exit the function. That is the entire process for scoring the utility of attacking one of our enemies with one of our weapons.

### COMPARING OPTIONS

Of course, we need to repeat ScoreTarget() for each of the enemies we face and for each of the weapons we carry. If we are facing three Dudes and have four weapons available to us, we have 12 options to score. By looping through the two lists, we can call ScoreTarget() for each of the 12 combinations. void CAgent::ScoreAllTargets()

{

sWEAPON_INFO ThisWeapon;

CDude* pThisEnemy;

USHORT ei, wi; // loop indexes

mvTargets.empty(); // start with a fresh list of targets

for ( ei = 0; ei < mpDudeCollection->GetSize(); ei++ ) {

pThisEnemy = mpDudeCollection->GetPointer( DUDE_TYPE(ei) );

for ( wi = 0; wi < mvWeapons.size(); wi++ ) {

ThisWeapon = mvWeapons[wi];

// Only consider loaded weapons

if ( ThisWeapon.Ammo > 0 ) {

ScoreTarget( pThisEnemy, ThisWeapon );

} // end if

} // end for

} // end for

}

We have also placed our call to ScoreTarget() inside an if statement that checks to see if we have ammo for the selected weapon. If not, there is no need to check it and add it to the list.

### SELECTING AN OPTION

At the end of ScoreAllTargets(), we have a list of all the possible attack combinations. Selecting an option is a simple exercise at this point. Because we built our scoring algorithm so that the lower the value, the better the option, we can logically deduce that the target with the lowest score is the best option for us to select. We can sort mvTargets of targets by the Score value so that the lowest value is at the beginning of the vector. (Naturally, we could simply walk the list and make a note of the lowest-scored item. I sort it here because doing so will come in handy in Chapter 16.) We then set our target and weapon to the values held in that location. The entire function for selecting a target is: CDude* CAgent::SelectTarget()

{

ScoreAllTargets();

std::sort( mvTargets.begin(), mvTargets.end() );

mpCurrentTarget = mvTargets[0].pDude;

mpCurrentWeapon = mvTargets[0].pWeapon;

}

Reading it in order, we score all the targets, sort the vector by the score, set our target, and change our weapon. That’s it. Easy enough. We did all the heavy lifting in the layers of formulas that we built piece by piece (Figure 14.11).

*FIGURE 14.11 The scoring algorithm for selecting a target and a weapon combination cascades through many levels. We combine Time to Kill Enemy with Time to Kill Me, to arrive at Threat Ratio, which we then combine with Urgency to yield our final score for the target.*

In Chapter 16, we will show some alternatives for selecting options that will allow us to create less predictable, deeper-looking behaviors in our agents.

### TESTING THE ALGORITHM

We will now take our new algorithm for a complete test drive to test some combinations of factors. When conducting this process, it is best to start with something simple and predictable. For example, if we place our three Dudes the same distance away from us and the same distance away from the detonator, and arm them all with the same weapon, we eliminate many of the possible variables.

Parameters:

| Dude | Distance | Dist. to Det. | Weapon |
| --- | --- | --- | --- |
| Baddie | 150 | 100 | Machine Gun |
| Evilmeister | 150 | 100 | Machine Gun |
| Boss Man | 150 | 100 | Machine Gun |

Results:

| Name | Weapon | Threat | Urgency | Score |
| --- | --- | --- | --- | --- |
| Baddie | Pistol | 754.6 | 0.369 | 2,044.7 |
| Baddie | M/G | 6.9 | 0.369 | 18.8 |
| Baddie | Shotgun | 754.6 | 0.369 | 2,044.7 |
| Baddie | R/L | 7.0 | 0.369 | 19.0 |
| Evilmeister | Pistol | 753.1 | 0.369 | 2,040.8 |
| Evilmeister | M/G | 6.6 | 0.369 | 18.1 |
| Evilmeister | Shotgun | 753.1 | 0.369 | 2,040.8 |
| Evilmeister | R/L | 6.7 | 0.369 | 18.3 |
| Boss Man | Pistol | 752.7 | 0.369 | 2,039.6 |
| Boss Man | M/G | 7.4 | 0.369 | 20.1 |
| Boss Man | Shotgun | 752.7 | 0.369 | 2,039.6 |
| Boss Man | R/L | 7.5 | 0.369 | 20.4 |

As we can see, because all the Dudes are at the same distance and using the same weapon, their threat ratios are very similar. The difference comes from two factors. First, the differences in their accuracy rates adjusts how much damage they can do to us. Boss Man will be a little more dangerous to us, and Baddie will be slightly less so. Second, the different types of Dudes have different amounts of health (50, 75, and 100). This makes a significant difference in how long it takes us to kill them. This yields an interesting result. Notice that killing the lowly Baddie is the secondmost-preferable option. Despite the fact that Baddie is a less dangerous attacker, because of his lower health, we can kill him quickly. The difference is small, but in such a similar situation as this, we might as well get him out of the picture. In this situation, though, our algorithm suggests that killing an Evil Dude (in this case, Mr. Evilmeister) is our best bet. Also, given the distance, our trusty algorithm informs us that our best choice is the machine gun.

Because the distance to the detonator is the same for all three Dudes, the urgency ratings are all the same. Therefore, there is no change in the best choice as reflected by the final score. We should whip out our trusty machine gun and let Evilmeister have it!

Here… Use This Instead If we make one change to the above scenario and give Evilmeister a shotgun, his threat ratio drops significantly. At 150 feet away, he can’t hurt us. Replacing only his lines from the above table, we can see how his threat ratio (and his final score) changes.

| Name | Weapon | Threat | Urgency | Score |
| --- | --- | --- | --- | --- |
| Evilmeister | Pistol | 1,000.0 | 0.369 | 2,709.7 |
| Evilmeister | M/G | 253.5 | 0.369 | 687.0 |
| Evilmeister | Shotgun | 1,000.0 | 0.369 | 2,709.7 |
| Evilmeister | R/L | 253.6 | 0.369 | 687.3 |

Evilmeister’s threat scores are high enough that he is taken completely out of the running as our prime target. (Lucky him!) Notice that the threat ratio for using a pistol or a shotgun is pinned at 1,000. This is as we designed. Because a pistol and shotgun can’t reach him at 150 feet, the damage we could due with those two weapons is 0. They are not even considered. Because Evilmeister is no longer a threat, we turn our attention to Baddie the Bad Dude, who still comes in with a score slightly under (more important than) Boss Man.

Back Off, Son! We will grudgingly give Evilmeister back his machine gun for the moment. There is something else we need to test. What would happen, for example, if Baddie were to begin to approach us? If we change the distance to Baddie to 100 feet rather than 150, his accuracy and damage rate go up. That makes him more dangerous. Of course, our rates go up as well, making it easier for us to kill him. As we would expect, Baddie becomes a higher priority as he gets closer to us. Again, showing only Baddie’s new data:

| Name | Weapon | Threat | Urgency | Score |
| --- | --- | --- | --- | --- |
| Baddie | Pistol | 29.0 | 0.369 | 78.6 |
| Baddie | M/G | 5.3 | 0.369 | 14.3 |
| Baddie | Shotgun | 753.4 | 0.369 | 2,041.6 |
| Baddie | R/L | 4.3 | 0.369 | 11.8 |

When we recall that the prior best score before Baddie made his move was Evilmeister’s 18.1, we can see how much those 50 feet meant. Not only should we now attack Baddie, but we should do it with our rocket launcher. The reason for the weapon switch is because of the accuracy and damage curves that we designed very early on. A quick glance at Figure 14.6 reminds us that the rocket launcher is our most effective weapon at a range of 100 feet. (We should notice as well that while the shotgun is still not a good choice, the pistol is now something to at least consider.)

Don’t Touch That! So far, all we have changed is the Dudes’ distance to us. Because we have not changed their respective distances to the detonator, the urgency scores have not changed. To test the effect this has on our scenario, let’s assume that Boss Man has made a break for the dastardly device. His path takes him no nearer to us, however. The only distance that is changing is the range to the detonator—to which he is coming alarmingly close. To complete the picture, we will leave Baddie at his closer range of 100 feet. The updated parameters, therefore, are:

| Dude | Distance | Dist. to Det. | Weapon |
| --- | --- | --- | --- |
| Baddie | 100 | 100 | Machine Gun |
| Evilmeister | 150 | 100 | Machine Gun |
| Boss Man | 150 | 20 | Machine Gun |

As we would expect, as Boss Man gets closer to his goal, our urgency to drop him increases. To help us make the various comparisons, we will list all 12 options again.

| Name | Weapon | Threat | Urgency | Score |
| --- | --- | --- | --- | --- |
| Baddie | Pistol | 29.0 | 0.369 | 78.6 |
| Baddie | M/G | 5.3 | 0.369 | 14.3 |
| Baddie | Shotgun | 753.4 | 0.369 | 2,041.6 |
| Baddie | R/L | 4.3 | 0.369 | 11.8 |
| Evilmeister | Pistol | 753.1 | 0.369 | 2,040.8 |
| Evilmeister | M/G | 6.6 | 0.369 | 18.1 |
| Evilmeister | Shotgun | 753.1 | 0.369 | 2,040.8 |
| Evilmeister | R/L | 6.7 | 0.369 | 18.3 |
| Boss Man | Pistol | 752.7 | 0.669 | 1,125.9 |
| Boss Man | M/G | 7.4 | 0.669 | 11.1 |
| Boss Man | Shotgun | 752.7 | 0.669 | 1,125.9 |
| Boss Man | R/L | 7.5 | 0.669 | 11.3 |

Checking back against our original results shows us that the threat scores for Boss Man have not changed. This makes sense because his weapon, our weapon, and the distance between us have not changed. On the other hand, because he is now only 20 feet away from the detonator, our urgency level for him has climbed to 0.669. Despite the fact that Baddie and Evilmeister have a more important (lower) threat ratio than does Boss Man, the Arch Dude’s proximity to the awful annihilatory apparatus drops his final score under that of the other options. Our decision would now be to attack Boss Man with our machine gun.

Dude, There Were Dudes Everywhere! In one last test, our original three Dudes are joined by reinforcements. There are now eight Dudes total: four Bad Dudes, three Evil Dudes, and their infamous Arch Dude leader, Boss Man. They are armed with different weapons, and arrayed at different distances away from us and from the detonator.

| Dude | Distance | Dist. to Det. | Weapon |
| --- | --- | --- | --- |
| Baddie 1 | 40 | 50 | Pistol |
| Baddie 2 | 80 | 125 | Machine Gun |
| Baddie 3 | 30 | 130 | Shotgun |
| Baddie 4 | 60 | 90 | Shotgun |
| Evil Genius | 120 | 80 | Rocket Launcher |
| Evil Knievel | 180 | 40 | Machine Gun |
| Evilmeister | 60 | 110 | Machine Gun |
| Boss Man | 90 | 125 | Rocket Launcher |

At first glance, it is difficult to sort through the list and determine who might be the highest-priority target. Consider the following observations:

Baddie 3 is closest to us and has a powerful shotgun. Evil Knievel is closest to the detonator. Baddie 1 is close to us and to the detonator. Baddies 1 through 4 have only 50 points of health and are, therefore, the easiest to kill. Boss Man is the most accurate shot and has a rocket launcher.

Unfortunately, none of those observations bring us to the correct conclusion. Only when we run the information through our algorithm do we take all of that information into account, rate it according to the formulas we have devised, combine

it in the ways we have defined, and arrive at a single score do we come to the most logical answer:

Use our rocket launcher to kill Evil Genius.

Because there are eight enemies and four weapon choices for each, there are 32 entries in our target list. Rather than clutter things up, we will only list the best weapon for each of the eight targets. (It’s not much of a surprise that it’s usually the rocket launcher, is it?)

| Name | Weapon | Threat | Urgency | Score |
| --- | --- | --- | --- | --- |
| Baddie 1 | R/L | 14.1 | 0.5218 | 27.0 |
| Baddie 2 | R/L | 4.0 | 0.3101 | 13.1 |
| Baddie 3 | R/L | 2.9 | 0.2992 | 9.7 |
| Baddie 4 | R/L | 250.7 | 0.3950 | 634.7 |
| Evil Genius | R/L | 3.2 | 0.4229 | 7.7 |
| Evil Knievel | M/G | 8.0 | 0.5626 | 14.3 |
| Evilmeister | R/L | 3.3 | 0.3445 | 9.8 |
| Boss Man | R/L | 2.6 | 0.3101 | 8.5 |

As we ponder the stats, we begin to see why Evil Genius didn’t immediately attract our attention. He ranks third on threat ratio (remember, lowest is most threatening) and third on urgency (higher is more urgent). If we had looked only at the disconnected data such as distance to us, distance to the detonator, weapon strength, and so on, we would not have detected the combination of information that makes Evil Genius the highest-priority target. (Rather ingenious of him, in an evil sort of way… don’t you think?)

### SUMMARIZING THE DECISION PROCESS

While it seems like it was a long road to accomplish the decision of which Dude to assault with which weapon, by breaking it down into individual steps, we were able to simplify the entire process. One of the important decisions we made about constructing our agent’s decision is what the decision was going to entail. As we theorized early in the chapter and subsequently confirmed in our last example, we could not have separated the two individual choices of who to attack and what weapon to use. Doing so may have led us to less-than-optimum results. This is reminiscent of the Prisoner’s

Dilemma, where thinking only about our own choice rather than taking into consideration our partner’s mindset led us to an acceptable yet not optimal solution. Only when we considered both of the inputs and results did we arrive at the best possible outcome. Once we decided what it was we were going to decide, we identified the individual components of the whole decision and dealt with each portion individually. Establishing compartmentalized confidence in each of those steps as we went along freed us up to concentrate only on the next step. For example, we were confident that the formulas for the accuracy of the weapons were accurate. We were also confident that the formulas for the damage from the weapons were accurate. Feeling good about both of those as individual functions, we felt comfortable combining the two into damage per second. Feeling that damage per second was an accurate measurement of strength, we felt quite secure in the validity of comparing our damagedealing power with that of the enemy. We continued the process of adding more layers—each layer only concerned with the one immediately before it. In the end, we arrived at our final decision of who to attack and with what. One of the payoffs of the time we spent in developing this model is that our agent is now highly dynamic. It responds well to changes in its environment. As Dudes move, it adapts. Adaptation and change in AI agents is one of the major steps in making AI seem more “alive” than mechanical and scripted.

There’s Always Something Bigger That doesn’t have to be the end, however. We could have continued to combine this result with something else. For example, we could introduce the idea of other actions that are not related to attacking: fleeing, hiding, surrendering, grabbing a health pack or a new weapon, running to a detonator of our own, or even pausing to take a photo to memorialize the occasion. To incorporate these other possibilities, we would build a process similar to the one for attacking and, as we have done a few times already, define a connection algorithm between them. Our process above then becomes part of a bigger picture. Rather than simply asking, “Who do we kill and with what?” a higher-level component would be asking, “If we decide to attack, who would we kill and with what?” There is a subtle difference between those two statements. The former is a final decision; the latter is a suggestion. The difference becomes clearer if we imagine processing the decision from the top down, instead. Imagine that our first decision was between the nebulous concepts of attack, flee, hide, get health, get weapon, and take memorial photo. How can we decide between them without knowing more about their relative merits? Sure, we can decide that get health is a high priority if we are low on health, but if there is

no health pack nearby, the arbitrary decision to “go get some health” could send us off on a wild goose chase (because everyone knows that geese provide health). We can’t decide to hide unless we know there are decent hiding places nearby. What constitutes “nearby” anyway? And what are we hiding from? What if we could easily win the battle because the attack decision tells us the Dudes that are arrayed against us have no hope at all? We would have no reason to hide even if there was “the very bestest hiding place in the history of ever” right next to us! In fact, because we completely overpower them, we can take a moment to fulfill that burning desire to pause and take a photo of the poor Dudes for our MySpace page prior to blowing them up. The point is, all of those decisions are related through information that is specifically tied to them. Based on criteria that each would process on its own— hide needing a convenient hiding spot, for example—the possible decisions would generate their own utility values. We can then compare and contrast these utility values to decide which action best suits our needs at the moment. The moral of the story of the Dudes is:

We can’t make decisions without information. Information is often ambiguous until we relate it to something. We can combine lots of little decisions into bigger decisions. We can roll bigger decisions into huge decisions. Beware the Evil Genius with the rocket launcher!

There is one problem with all of the above, however: Information rarely stays the same—and especially not for very long. So then what do we do?

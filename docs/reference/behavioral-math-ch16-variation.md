# Behavioral Mathematics for Game AI — Chapter 16: Variation in Choice

> Extracted from `docs/reference/BehavioralMathematicalforGameAI.pdf` (Dave Mark, 2009)  
> PDF pages 436–464 · Book pages 417–445

---

## 16. Variation in Choice
In the past few chapters, all of our examples have been solved by finding “the best” answer to the problem. We built all of our utility functions to rate and score the aspects of the decision, sorted the options by the score, and selected the one with “the best” score (be that highest or lowest). Naturally, this sounds strikingly like normative (or prescriptive) decision theory. In fact, if we recast it in the vernacular we used in Chapter 4, we were determining what we should do. Remember that normative decision theory assumes that our agent:

Has all of the relevant information available Is able to perceive the information with the accuracy needed Is able to perfectly perform all the calculations necessary to apply those facts Is perfectly rational

We spent a good deal of time in Part II of this book showing examples of how humans just don’t do what is perfectly rational. On the other side of the spectrum, descriptive decision theory tells us what people elect to do. More specifically, it reports what they have already done. Because our data for how demons from the underworld will counter the assault of a single space marine is somewhat lacking, we have to construct our own models—which defies the survey-based definition of descriptive decision theory. Our goal, however, is to create behaviors that only look like they are born out of the wide palette of descriptive decision theory’s data. The first step in this direction is away from the single “best answer” that normative decision theory gives. That is, we are looking for variation in our decisions. How do we create this variation without data from which to work? To find the answer, let’s first examine why we want variation in the first place.

### REASONS FOR VARIATION

There are a number of reasons why we would want to create variation in the decisions our agents make. We can illustrate some of them best by using examples of what it looks like when variation is not present. Imagine the following: We enter a bank containing 20 customers, and 5 employees. We approach the middle of the lobby and begin yelling loudly that we are robbing the bank. As we look around to assess the impact that our display is having on the people, we are stunned to realize that all 25 people are reacting in exactly the same way. They are all standing there in stunned silence, too afraid to move. Not a single one has screamed or spoken. Not a single one has taken a step away from us—or toward us, for that matter. All 25 people have reacted in exactly the same way! Curious about this odd display of solidarity, we reach into our jacket for our gun. The moment our hand disappears into our pocket, every single customer in the bank shrinks into a crouch, ready to either duck or run. All of them. A glance at the tellers behind the counter shows that all five of them have raised their hands. As we pull out our gun and wave it wildly in the air, every player in this peculiar show drops to the floor and screams. None of them only duck. None of them try to run for the door. None of them are still paralyzed with fear. None of them even fall to the floor faster or slower than the others. The tellers, in the mean time, have all raised their right hands toward us in a placating gesture. Exactly two seconds later, their left hands reach into their bank tills. Simultaneously, all five say, “Here, take the money!” Dying to find out what this impromptu synchronized exhibit will do next, we turn the gun to point it at our own head. The customers all stop screaming, even the small girl in the corner. The employees all put down their hands in such rigid precision it seems like they are about to bust into a group version of the Macarena. We pull the trigger on the gun and the water inside squirts against the side of our head. All 25 people gasp in surprise in what sounds like a single, magnified intake of breath. As the water trickles to the floor, we yell “Hey folks! I was just kidding! It was a joke!” As if cued by the producer of a bad TV sit-com, all 25 people begin laughing. We put our now empty squirt gun back into our pocket, whereupon all 25 people turn back to what they were doing prior to our entrance. To a man, they all seem completely unfazed by the trauma we just inflicted on them.

On the other hand, we are a little disturbed by what we just witnessed. We turn to the door, muttering under our breath, “Wow… it was like a bad video game or something.”

### VARIATION BETWEEN ACTORS

As ludicrous as the above scenario seems, many of us have probably experienced something like this in games. The underlying technical problem with the above scenario (if you didn’t get it already) is that all the agents were using the same algorithm to determine their reaction to a stimulus. Every time we changed the world state through our actions, we fed the 25 people the same information with which to calculate their decisions. As a result, they all arrived at the same conclusion about what the “best” reaction should be. The only difference in the actions of the various people was that the employees and the customers had different reactions at one point. The reason for that is that they had different programmed reactions to the stimulus of us waving our gun around. The customers dropped to the floor and began screaming; the employees tried to placate us. The only reason for this occurring is that, at some point, either a designer or a programmer differentiated between the two: “When the player does [insert action], I want the customers to [insert reaction] and the employees to [insert reaction].” While the separation of customer vs. employee is admirable and was certainly a logical division, there is a subtle implication in that statement. Imagine the above statement rephrased this way: “When the player does [insert action], I want every single customer to [insert reaction] and every single employee to [insert reaction].” By inserting the specifier “every single…” we draw attention to the inherent weakness in this line of thinking. The differentiation between customer and employee was based on the reasonable premise that customers and employees would likely react to the situation in different ways. This is a logical assumption given the fact that their roles in a bank robbery would be different. They enter the scenario of a bank robbery with different goals. The customers’ primary goal is “don’t get killed.” The employees have two goals: “don’t get killed” and “defuse the situation by giving the really scary man what he wants.” The problem with the artificial intelligence (AI) as theoretically designed in our example is that every person’s solution for how to reach those goals was exactly the same as every other person’s. If we were to imagine how a scenario such as this one would go in real life (or even in a movie, for those of you who can’t envision yourself in the position of the bank robber), we likely would have a different picture than the one painted above. Simply making a list of possible customer reactions is enlightening.

Stand paralyzed with fear Scream Scream and run Scream and duck Scream and grab your child/parent/husband/wife/random stranger Just run (no screaming) Just duck (no screaming) Just grab your child/parent/husband/wife/random stranger (no screaming) Move slowly away from the robber Move toward the robber Try to get behind a desk Try to jump over a desk Reach for the robber’s gun Reach for our own concealed weapon Try to call 911 on our phone Laugh because the robber is wearing dark glasses and a stupid hat

As we can see, there are a lot of options that the customers and the employees could have chosen from. There are probably many more. Add to this list that most of these actions could be done with various delays and speeds. For example, not everyone is going to scream at exactly the same time. If we can anecdotally list so many actions that our agents could select, the fact that they all selected the same action becomes even more startling. Variation in behavior is what makes humans look… well… human. Because, as we are fond of saying, “no two people are exactly alike,” it is unlikely that their reactions to a given set of stimuli will be the same.

### VARIATION BY A SINGLE ACTOR

While it is important to consider the behaviors of a group of actors, we also need to consider the actions of a single actor. Imagine, if you will, the following scenario:

We enter a bank containing 20 customers and 5 employees. We approach the middle of the lobby and begin yelling loudly that we are robbing the bank. The woman to our left shrieks in terror. The man to our right drops to the ground and assumes the fetal position. The teenager in back looks up from her phone where she is text-messaging, shrugs, and looks back down.

The security guard by the wall turns toward us and stares slack-jawed in disbelief and, after two seconds, reaches for the gun at his belt. The girl in the pink dress says, “Look at the man with the dark glasses, Mommy! His hat is stupid!”

After displaying our nifty squirt gun trick to the bank patrons and employees (and shooting an annoyed look at the little girl with the big mouth), we leave the bank.

Later in the day, we return to the bank, which (surprisingly) still contains 20 customers and 5 employees. We approach the middle of the lobby and begin yelling loudly that we are robbing the bank. The woman to our left shrieks in terror. The man to our right drops to the ground and assumes the fetal position. The teenager in back looks up from her phone where she is text-messaging, shrugs, and looks back down. The security guard by the wall turns toward us and stares slack-jawed in disbelief and, after two seconds, reaches for the gun at his belt. The girl in the pink dress says, “Look at the man with the dark glasses, Mommy! His hat is stupid!”

Wow. That’s odd. Did I just step into my own version of the movie Groundhog Day? What’s with these people? That’s exactly what they did earlier today! Wait a minute… let’s try this again.

We step outside onto the sidewalk and count to ten before we reenter the bank lobby. We approach the middle of the lobby and begin yelling loudly that we are robbing the bank. The woman to our left shrieks in terror. The man to our right drops to the ground and assumes the fetal position. The teenager in back looks up from her phone where she is text-messaging, shrugs, and looks back down. The security guard by the wall turns toward us and stares slack-jawed in disbelief and, after two seconds, reaches for the gun at his belt. The girl in the pink dress says, “Look at the man with the dark glasses, Mommy! His hat is stupid!”

My guess is that you, my ever-gentle reader, skipped the last paragraph of our little story above. Why? Because it was stale the third time around. You knew what was going to happen. You had already seen how each person was going to react to our declaration of impending robbery. If I asked you what the woman on the left would do if we reentered the bank and once again made our threat, you would be able to tell me with unflagging certainty that she was going to shriek in terror. After all, we have been presented with the impression that she always shrieks in terror. Every time. Without fail. Why would we think anything else?

Not only is rigid predictability boring to us, the observer, but it is not even remotely human. If we think of a situation in our own lives that occurs regularly, it is likely that we can also make a list of our actions or reactions to that situation at different times. For the sake of continuity, let’s look again at the list of bank robbery reactions above. How many of them could we see ourselves possibly exhibiting? Some of them might be more likely than others, of course, but we would have at least a small chance of doing a great number of them. There also may be a few that we may never consider. (I can no longer see myself jumping over a desk, for example.) More importantly, if we found ourselves in a bank robbery situation again later (hopefully not the same day!), would we pick the exact same reaction? We still would have the same list of things we might do, but could we guarantee that any one of them is the one we would do every time? It is far more likely that we would react different ways each time we were faced with a bank robbery situation. If we experienced ten different robberies that were all similar in nature, how many different reactions would we exhibit? Two or three? Five? Would we have ten different reactions? (At this point in our saga, I am thinking that banking exclusively online would also start to look more attractive.) The quandary becomes apparent when we juxtapose having all those choices that we might select from against a formula or algorithm that tells us that this is “the best” reaction to have every time. Just as it was a false assumption to think that every person in the bank would react the same as every other person one time, it is likewise a tenuous assertion to say that one person would react the same way every time.

### EMBRACING RANDOMNESS

Individual people make decisions based on a massive collection of information— only some of which is the current environment. The adage goes that “we are the sum of our life experiences.” Those experiences were, themselves, information at one time. Now, however, they have taken on a different role in our lives. What were events at the time have become memories. Experiences have shaped beliefs. Traumas have begotten fears. Pleasures have nurtured cravings and desires. Put into mathematical terms, those life experiences of the past are now the formulas, equations, and algorithms through which we pass the inputs of the current environment. The problem is, even if we were to analyze our own life in great detail, we would be hard-pressed to codify our own psychology accurately enough to construct the algorithms that would yield the “right way” for us to respond to those inputs. Even if we could do so, the construction of such a system would be prohibitively labor-intensive. (Yes, this is a massive understatement.) Additionally, the computations involved would be processor-intensive. Oh yeah… don’t forget that

we would have to model the game world in such excruciatingly fine detail that we may as well be creating the holodeck from Star Trek. To sum up, it ain’t gonna happen. Of course, constructing one algorithm to tell us what we “would do” is a lot like the normative decision theory approach of one algorithm to tell us what we “should do.” That’s what got us into this problem in the first place. To return to our bank example, we would have to construct algorithms to replicate the life experiences of 20 typical bank customers and 5 typical bank employees. And when the cops show up at the bank to arrest us, we would have to create three or four individual “life experience models” of typical police officers. If constructing and using one such model was prohibitive, we probably don’t have a word for what the prospect of creating dozens of them would be like. (I nominate “insane.”)

Never Mind All That… Remember, however, that we really don’t care about how a bank customer got to the point in his life that would cause him to react in a particular way; we care about the result of all of that information and experience-building. This is the same rationale that we used with the dentists dispensing their helpful advice. We don’t know or care why four of the five dentists recommended sugarless gum or why the fifth did not. We just know that they did. If we were going to create our Dental Advice Simulator, we would not have to worry about the fact that one of the dentists’ spouses recently died of complications due to diabetes and another is taking kickbacks from the Sugar Growers of the World Association. To simulate the dentists we would simply codify that 80% of dentists we encounter recommend sugarless gum and the remaining 20% do not. We can leave it up to the players to overlay such creative interpretations if they really feel the driving need to do so. As we dealt with in Chapters 11 and 12, probability distribution and random selection is something we can model rather well. How hard is it, after all, to recommend sugarless gum 80% of the time? Using that simple procedure, our Dental Advice Simulator could simulate a whole convention hall full of dentists! We have substituted simple probability distributions for the complex (and largely unnecessary) process of generating the minutia of what goes into individualized human decisions. The results are strikingly compelling, however. When we query our faux dentists about their thoughts on sugarless gum, rather than acting in rigid unison, 80% of them (give or take a few, I’m sure) would raise their hands. A player of our Dental Advice Simulator would see that, nod to himself, and say, “Wow… looks like about four out of five to me. That’s a reasonably accurate simulation of dentists!”

A Framework for Randomness It’s important to note that we did not make the decision completely random. We did not flip a coin to decide. Doing so would have generated a 50/50 split on the

Great Sugarless Debate. We used a random number process in conjunction with a carefully constructed probability distribution (i.e., four out of five). Yes, the dentist example was simple, but we can do far more if necessary. In fact, we explored some of this in Chapter 11 when we randomly created a population of guesses in the Guess Two-Thirds the Average Game that was similar to the results of the Danish study. Our approach in that exercise was to use random generation of guesses based on a combination of specifically tailored probability curves. Thinking further back to Chapter 6, recall the real lesson of the Guess Two- Thirds the Average Game. We were trying to find the sweet spot between the purely rational (yet very unlikely) answer of 0 and a completely random (and also unlikely) guess. That is, we were trying to get away from the rigid rationality of normative decision theory and move toward the more human-like descriptive decision theory. Our solution at the time was the carefully crafted and controlled application of structured randomness. The question that we still must answer, however, is how do we build that structure around our randomness? In the Guess Two-Thirds the Average Game, we had an example from which to work. It was much like how I was able to paint my somewhat realistic-looking pig because I was looking at a photo of a pig. From an artistic standpoint, I couldn’t draw a pig from scratch. However, by looking at one, I could copy the bits and pieces and replicate what was already there. We did this in Chapter 11 by reconstructing the guesser distribution one part at a time. We weren’t modeling why they were guessing the way they did, only that they did so because a survey told us, “This is how 19,000 Danes guessed.” Unfortunately, we don’t always have a handy survey to tell us how people divided themselves among the choices that range between “the best” and “the most ridiculous ever.” We need to have a more procedural way of outlining the framework that will support our otherwise stochastic process. Surprisingly, we are already closer to the solution than we might think.

### SELECTING FROM MULTIPLE CHOICES

Let’s break our problem down into the features we need in our decision model and the features we don’t want.

We want more variety than “the best” answer. We don’t want to include “the worst” answer. We do want to include “reasonable” answers. We don’t want to include “unreasonable” answers.

In the above criteria for what we do and don’t want, we were coming at the problem from both ends. We want more than just the best choice. We acknowledged that there are other choices that might be acceptable. On the other hand, we certainly don’t want the worst choice, and there are likely a lot of choices that aren’t the worst but are pretty darn bad as well. We want to avoid those. But how do we separate the good from the bad?

### RANDOM FROM TOP

n CHOICES If we list our choices ranked from optimum down to downright silly, someplace in the middle is a meeting point between the good ones and the bad ones. The problem is that we don’t know where that point happens to be. This causes us to get a little arbitrary at first. In our Dudes example, we scored each combination of Dude and weapon. By selecting the best score (in this case, the lowest number), we chose which weapon to use and which Dude to target. However, that means that every time we encounter that exact arrangement of Dudes, we will do exactly the same thing. While this may not be startling in and of itself, what if there are two of us… or five… or a whole bank full of us all running the same algorithm. We would all respond exactly the same way. We would all pull out the same weapon and fire at the same Dude. Never mind that this collective response is a tactical nightmare; it just looks silly. It looks even more ludicrous when we translate it back out of our Dude scenario and into something like our bank example. Even if everyone was processing the same inputs, would everyone react exactly the same way? Not likely. Therefore, despite the fact that there may be a single best Dude to attack with a single best weapon, it probably does not behoove us to insist that we follow that model rigidly.

Opening the Playbook Thankfully, one of the advantages of ranking the scores was that we learned more than simply which one was best. We also are aware that there were others that were decent choices as well… just not the best. To confirm this, let’s examine the results of our final Dude example from Chapter 14. (Note that in Chapter 14, we had limited our list to only the best weapon for each Dude. Here, we include them all. We’ll see why in a moment.)

| Name | Weapon | Score |
| --- | --- | --- |
| Evil Genius | R/L | 7.7 |
| Boss Man | R/L | 8.5 |
| Baddie 3 | R/L | 9.7 |
| Evilmeister | R/L | 9.8 |

| Evil Genius | M/G | 10.8 |
| --- | --- | --- |
| Baddie 3 | Shotgun | 11.2 |
| Baddie 3 | M/G | 12.6 |
| Baddie 2 | R/L | 13.1 |
| Evilmeister | M/G | 13.8 |
| Evil Knievel | M/G | 14.3 |
| Boss Man | M/G | 14.8 |
| Baddie 2 | M/G | 16.3 |
| Baddie 1 | R/L | 27.0 |
| Baddie 3 | Pistol | 28.7 |
| Baddie 1 | M/G | 28.8 |
| Baddie 1 | Shotgun | 29.4 |
| Baddie 1 | Pistol | 38.6 |
| Evilmeister | Pistol | 44.0 |
| Baddie 2 | Pistol | 54.2 |
| Boss Man | Pistol | 100.0 |
| Evil Genius | Pistol | 257.7 |
| Baddie 4 | R/L | 634.7 |
| Baddie 4 | M/G | 637.0 |
| Baddie 4 | Pistol | 654.5 |
| Evil Knievel | Pistol | 1,339.7 |
| Evil Knievel | Shotgun | 1,339.7 |
| Evil Knievel | R/L | 1,339.7 |
| Evil Genius | Shotgun | 1,777.0 |
| Evilmeister | Shotgun | 2,183.4 |
| Boss Man | Shotgun | 2,421.3 |
| Baddie 2 | Shotgun | 2,428.8 |
| Baddie 4 | Shotgun | 2,531.1 |

Remember that we had 8 enemies and 4 weapon choices for each, giving us 32 possible combinations. We have ranked them here in order from the best choice to the worst. Recall that our eventual selection in this scenario was to kill Evil Genius with our rocket launcher. After all, our delightfully thorough algorithm told us that was the best answer.

Wheat and Chaff Now that we have looked at all the possibilities, however, we can see that, while “shoot Evil Genius with the rocket launcher” is, indeed, the lowest score, there were plenty of other options that were very close. How much worse would it have been for us to attack Boss Man with the rocket launcher instead? In the heat of battle, would it have looked completely illogical for us to attack Boss Man instead of Evil Genius? (After all, he’s Boss Man! And he’s got a rocket launcher too!) What about Baddie 3 (who was 30 feet away with his potent shotgun) or Evilmeister (only 60 feet away with his machine gun)? Those would certainly have been understandable and legitimate targets for us—and we see this reflected in the scores. On the other end of the spectrum, we can see that there were plenty of options that would have looked just plain odd. With all these Dudes running around near and far, why would we elect to attack the ones that are far away with a shotgun that is all but useless at those distances? That explains the bottom five options in the list. They are simply out of the question.

Drawing a Line As we theorized, someplace in the middle of this list, the good meets the bad (by blending into the mediocre). While we would have been comfortable selecting one of the “reasonable” answers, we want to avoid the “unreasonable” ones. The trick is deciding where that line is. Everything under 10? Under 50? Under 500? Pragmatically, we realize that we can’t even use the scores in this list as a guide. In a different situation, we may not have any options that score under 10… or even 50. The scores will change all the time relative to whatever arbitrary line in the sand we draw. However, one thing is certain: No matter what the scores are, we can always arrange them relative to each other. After all, that’s how we determined the best option before. “Attack Evil Genius with our rocket launcher” was the best relative to the other scores. The solution, therefore, lies not in basing the threshold on an absolute score but on the score relative to other scores. Instead of limiting ourselves to only the single best item, we can select from a handful of the best scores. How many options we elect to choose from will be very problem-specific. (Certainly, you are getting tired of hearing that by now.) We could, for example, choose to select from the top 10 options. However, what if there are fewer Dudes and fewer weapons available to us? We may not have 10 options from which to select. Usually, it is a better idea to use the top n% of the choices rather than the top n choices. In the case of the Dudes, for example, we could decide that our cut-off is 25% of the total possible choices. In this case, we would consider the top eight items. If we had more options, that number would increase. As we removed Dudes

from the battlefield and perhaps ran out of ammo for some of our weapons, the total number of options available to us would decrease, and the number that we would consider would contract with it. Now that we have decided that we will consider the top eight choices, we have truncated our list to:

| Name | Weapon | Score |
| --- | --- | --- |
| Evil Genius | R/L | 7.7 |
| Boss Man | R/L | 8.5 |
| Baddie 3 | R/L | 9.7 |
| Evilmeister | R/L | 9.8 |
| Evil Genius | M/G | 10.8 |
| Baddie 3 | Shotgun | 11.2 |
| Baddie 3 | M/G | 12.6 |
| Baddie 2 | R/L | 13.1 |

Those all look like fairly reasonable actions for us to take. Their scores are so similar, it almost doesn’t matter which one we choose. Therefore, a simple solution would be to pick one of them randomly.

### PUTTING I TIN CODE

We can do this very simply by changing the selection code in CAgent. The original SelectTarget() function sorted the vector and selected the target in the 0 index— the lowest-scored combination. void CAgent::SelectTarget()

{

ScoreAllTargets();

std::sort( mvTargets.begin(), mvTargets.end() );

mpCurrentTarget = mvTargets[0].pDude;

mpCurrentWeapon = mvTargets[0].pWeapon;

}

By adding two lines, we can determine how many records we should consider (25% of the total number) and generate a random vector index in that range. Rather than returning the record at index 0, we are returning a random one between index 0 and (NumberToConsider – 1). void CAgent::SelectTarget()

{

ScoreAllTargets();

std::sort( mvTargets.begin(), mvTargets.end() );

USHORT NumberToConsider = mvTargets.size() * 0.25;

USHORT i = rand() % ( NumberToConsider – 1 );

mpCurrentTarget = mvTargets[i].pDude;

mpCurrentWeapon = mvTargets[i].pWeapon;

}

Selecting randomly may seem counterintuitive and even mildly alarming. Why, after all the work we went through in Chapter 14 to arrive at these scores, would we seemingly cast it all aside and go back to random selection? The answer to this is in the lessons we learned all the way back in Chapter 5. In the Ultimatum Game, we determined through similarly mathematical methods that the best option for the Giver to bestow upon the Receiver was $1. That was the most logical answer. Likewise, it was in the best interests of the Receiver to accept that amount. (After all $1 > $0, right?) And yet, people placed in those two roles rarely do either of those “best” actions. We explored this further in Chapter 6 with examples such as the Pirate Game, the Guess Two-Thirds the Average Game, and even the Monty Hall Problem. We humans like to talk about how rational we are, but, for a variety of reasons, we usually aren’t completely so. We may occasionally select the “best” answer but usually only get close by selecting a reasonable answer.

The Payoff That is what we accomplish by using a hybrid of our scoring system (pure rationality) and random selection (inexplicable irrationality). By allowing our agent to select from a number of reasonable-looking choices, we are effectively modeling all

the little vagaries of humanness that we don’t care to model. Perhaps when our agent asked for his first rocket launcher at the age of eight, someone told him “You’ll put your eye out, kid,” and he’s been a little wary of them ever since. Perhaps he is particularly offended by the dark glasses and stupid hat that Baddie 3 has donned for this encounter. It doesn’t really matter why they act differently; humans simply do. Therefore, we don’t need to model the whys of the different decisions. We simply provide a mechanism that simulates those different (but still somewhat reasonable) decisions. Think back to Chapter 1 and the definition of a game provided by Sid Meier: “A game is a series of interesting choices.” We examined that notion by using examples such as Rock-Paper-Scissors, Tic-Tac-Toe, Blackjack, and Poker. Rock- Paper-Scissors is predominantly random. The choices in Tic-Tac-Toe are almost entirely predictable. Blackjack has random components, but the dealer’s choices are entirely rule-bound; we don’t know what either of us will get next, but we know exactly what the dealer will do with it when he gets it. Only when we get to Poker do we see a challenge provided by the other player (i.e., the agent). The reason this is so compelling is that while we have an idea of what he might do, we can’t predict exactly what he will do. Responding to his interesting choices is what makes our choices that much more interesting. We will find that the result of changing our agent’s model to include more options is that he will display broader behavior than when he was restricted to only the one choice. In fact, if the agent were to encounter the same circumstances again, he may or may not select the same approach. If we are playing as one of the Dudes, this is actually a boon. Just as with the Poker player, the agent’s interesting (yet reasonable) variation of choices makes our choices that much more interesting as well. From a player’s standpoint, it also helps avoid situations like that uncomfortable moment in the bank when everyone reacted in exactly the same way. In this case, if we were a Dude facing multiple AI agents, they would look just as peculiar all doing exactly the same thing. It seems that not only is variety the spice of life, but it keeps AI from looking like the Stepford Wives—completely identical in thought and deed… and horribly boring.

### WEIGHTED RANDOM FROM TOP

n CHOICES We still have one problem with the approach we are using. We arbitrarily chose to randomly select from the top 25% of options. In the case of our primary example, that meant there were eight possibilities in play. Once we had split those eight options off, for all intents and purposes, we treated them equally. We didn’t bias the random selection whatsoever. Because the top eight options were similar, there was little difference between them anyway. We can see this in the black bars of Figure 16.1.

While that approach worked for our sample data, imagine that our scores had turned out more like what we see with the gray bars in Figure 16.1. In that example, there is a large jump between the score of the first two and the rest of the options. We can no longer consider these eight options as being equal. That hamstrings our idea of selecting randomly from that list of eight.

*FIGURE 16.1 If there is a significant difference between the scores of our top n selections, we should not treat them equally.*

One possible solution to this problem is to reduce our cutoff to two possibilities instead of eight. However, that almost negates the purpose of expanding it in the first place. Also, we have no way to predict where that cusp will be on any iteration of this process. Looking at our original data (black bars), we could have included the first 12 records before we saw a significant jump in the scores. To continue to provide some variety in our choices, we would like to continue drawing from the top eight selections (or, more generally, the top 25% of the possible selections). What we do not want to do, however, is to treat all eight possibilities as equal. Thankfully, our scoring system provided us with something other than an ordinal ranking of the possibilities. The very fact that the scores vary from one option to the next reminds us that these differences are proportional. In layman’s speak, some options are “more better” than the options that come after them. We need a way to leverage these proportional differences.

We already know that we would like for the “better options” to occur more often than the “not quite as good” options. We discussed in Chapter 12 that response curves are excellent at helping us tailor customized distributions. We can then select randomly from the weighted options, knowing that the more weight we give to an option, the more often that option will occur. The first thing we need to address is how to weight our options. We want to convert our somewhat abstract “score” into a proportional weight. As a general rule, the larger the weight we want to set an option at, the larger the number we will need to use to set the size of the bucket for that option. When we look at our score values for the options, we might have a moment of concern—our better options have smaller scores. Our scores are an abstract rather than concrete value, however; they do not represent something we are measuring. All we are concerned with is their relationship to each other. We don’t have to preserve any sanctity of the actual numbers.

Assigning Weights In Chapter 13, we discussed a method of weighing scores relative to other scores. If we put all the other scores in terms of the best score, we preserve the relationship between them. To set our benchmark, we set the best option’s score to 1.0. For each of our options, i, we score them as

Naturally, Weight1 = 1.0 because we are dividing it by itself. The next item in our original list, Boss Man with the rocket launcher, scored an 8.5, compared to 7.7 for the best option (Evil Genius with the rocket launcher).

Therefore, the second option would have a smaller weight and be slightly less likely than the first. Continuing through our eight options, we find:

| Name | Weapon | Score | Weight | Edge | % |
| --- | --- | --- | --- | --- | --- |
| Evil Genius | R/L | 7.7 | 1.00 | 1.00 | 16.5 |
| Boss Man | R/L | 8.5 | 0.90 | 1.90 | 14.8 |
| Baddie 3 | R/L | 9.7 | 0.79 | 2.69 | 13.0 |
| Evilmeister | R/L | 9.8 | 0.78 | 3.47 | 12.9 |

| Evil Genius | M/G | 10.8 | 0.71 | 4.18 | 11.7 |
| --- | --- | --- | --- | --- | --- |
| Baddie 3 | Shotgun | 11.2 | 0.69 | 4.87 | 11.4 |
| Baddie 3 | M/G | 12.6 | 0.61 | 5.48 | 10.0 |
| Baddie 2 | R/L | 13.1 | 0.59 | 6.07 | 9.7 |

Examining the weights of the options, we can see how the weights are proportional to how often we would like them to occur. For example, the last option (Baddie 2 with the rocket launcher) would occur about 60% as often as our top choice. The last two columns give us another look at our results. The fifth column, Edge, shows us what the edges of the buckets would be if we put all our weighted options end to end. The final column shows the percentage chance of each option occurring based on the weights. If we had picked randomly, each option would have had a 12.5% chance of being selected. Because of the application of our weights, those percentages now range from 16.5% for our most preferable selection to 9.7% for our eighth-place option.

The Haves and Have Nots While it is noticeable, there is not much of a spread between the percentages of those eight options. We can see a more distinct difference when we use data that is more disparate—such as the modified data (gray bars) in Figure 16.1.

| Name | Weapon | Score | Weight | Edge | % |
| --- | --- | --- | --- | --- | --- |
| Evil Genius | R/L | 7.7 | 1.00 | 1.00 | 25.8 |
| Boss Man | R/L | 8.5 | 0.91 | 1.91 | 23.5 |
| Baddie 3 | R/L | 18.1 | 0.43 | 2.34 | 11.1 |
| Evilmeister | R/L | 19.3 | 0.40 | 2.74 | 10.3 |
| Evil Genius | M/G | 22.6 | 0.34 | 3.08 | 8.8 |
| Baddie 3 | Shotgun | 26.9 | 0.29 | 3.37 | 7.5 |
| Baddie 3 | M/G | 27.4 | 0.28 | 3.65 | 7.2 |
| Baddie 2 | R/L | 33.1 | 0.23 | 3.88 | 5.9 |

Because the scores for this batch of data are more widely spread, the resulting occurrence percentages are significantly different as well. We can see that the two options with the relatively low scores of 7.7 and 8.5 will occur 25.8% and 23.5% of the time, respectively. That is almost 50% of the time between the two of them. We are not precluding the other six options from selection, however. They will occur, but at a reduced frequency that is on par with their proportional score.

A Different Edge We will notice one problem with the numbers that we have generated above when we try to use a response curve to select an option, however. Remember that to use a response curve, we need to generate a number between 0 and whatever the outermost edge is—that is, the edge of the last bucket. In the case of the example above, that edge is 3.88. Because some random number generators tend to yield integers, however, we would have to do a conversion from the integer result to generate a number between 0 and 3.88. While we certainly could do that conversion, there is another approach we could take. Remember, the actual values of the weights themselves do not matter—only the proportion between them. Therefore, to avoid this problem, we could multiply the weight formula by a coefficient. The result is the same but has the benefit of converting our bucket edges to integers instead of decimal values. The actual coefficient that we should select is a little difficult to ascertain, however. Again (say it along with me, folks), it is a very problem-specific issue. The main consideration is that we want to achieve enough granularity to express subtleties in the sizes of the buckets. Because we are going to have to round to the nearest whole number, if the coefficient is too small, we risk rounding similar numbers off to the same size when we would rather they remain distinctly different. For example, if we were to multiply the weight scores above by a coefficient of 2, we would get the following numbers:

| Name | Weapon | Score | Weight | Edge | % |
| --- | --- | --- | --- | --- | --- |
| Evil Genius | R/L | 7.7 | 2.00 | 2.00 | 25.8 |
| Boss Man | R/L | 8.5 | 1.82 | 3.82 | 23.5 |
| Baddie 3 | R/L | 18.1 | 0.86 | 4.68 | 11.1 |
| Evilmeister | R/L | 19.3 | 0.80 | 5.48 | 10.3 |
| Evil Genius | M/G | 22.6 | 0.68 | 6.16 | 8.8 |
| Baddie 3 | Shotgun | 26.9 | 0.58 | 6.74 | 7.5 |
| Baddie 3 | M/G | 27.4 | 0.56 | 7.30 | 7.2 |
| Baddie 2 | R/L | 33.1 | 0.46 | 7.76 | 5.9 |

Note that only the weights and the edges changed. The percentage scores and the percentages are still the same. This process is not changing the distribution of choices; we are only attempting to make selecting a choice a more accurate process. The problem we would still have with the above data is that the granularity is still too coarse. If we rounded the data off, we would have two 2s, five 1s, and even

a 0. Ignoring the problem of having a bucket size of 0, we still have to deal with the fact that five of our buckets are the same size again. We have lost all the subtlety of the scores between 18.1 and 27.4 by compressing them into a bucket width of 1 each. If we increase the coefficient to 10, our numbers now are a little more workable:

| Name | Weapon | Score | Weight | Edge | % |
| --- | --- | --- | --- | --- | --- |
| Evil Genius | R/L | 7.7 | 10.00 | 10.00 | 25.8 |
| Boss Man | R/L | 8.5 | 9.06 | 19.06 | 23.4 |
| Baddie 3 | R/L | 18.1 | 4.25 | 23.31 | 11.0 |
| Evilmeister | R/L | 19.3 | 3.99 | 27.30 | 10.3 |
| Evil Genius | M/G | 22.6 | 3.41 | 30.71 | 8.8 |
| Baddie 3 | Shotgun | 26.9 | 2.86 | 33.57 | 7.4 |
| Baddie 3 | M/G | 27.4 | 2.81 | 36.38 | 7.3 |
| Baddie 2 | R/L | 33.1 | 2.32 | 38.70 | 6.0 |

As we look at the weights for each option, we see that our buckets are a variety of different sizes. The third and fourth end up with a rounded size of 4, and the fifth, sixth, and seventh round to a size of 3, however; we still are losing some of our fine shades of difference. In the original example where the eight scores were closer together, the loss of detail would be even more pronounced.

Automatic Scaling Thankfully, we don’t have to guess what size would be an appropriate coefficient. We can rewrite our weight equation to help us automatically scale our weights to a reasonable granularity. It does involve one extra step, however. To generate the weights above, we scaled everything relative to the lowest score. That is, the lowest score was the anchor point of 1.0 (before the application of a coefficient). The other, higher scores generated smaller weights as a result. We had no way of accounting for what the other scores were because we were using the single score as our only frame of reference. Because we are generating proportions of the whole that each option represents, we need to begin with what the whole looks like. If we sum the scores in our list of options, we can then further calculate the proportion of the whole that each individual score represents using the following formula:

Looking back at our data table, the sum of the eight scores is 163.6. Therefore, the weight of our first option would be

Rounded off to an edge-friendly integer, the Weight of our first bucket is 21. Checking our accuracy, we find that 21 is 25.9% of 163.6. This is reasonably close to the 25.8% that this option represented before. Applying this method to the other options, we arrive at the following new data:

| Name | Weapon | Score | Weight | Edge | % |
| --- | --- | --- | --- | --- | --- |
| Evil Genius | R/L | 7.7 | 21 | 21 | 25.9 |
| Boss Man | R/L | 8.5 | 19 | 40 | 23.5 |
| Baddie 3 | R/L | 18.1 | 9 | 49 | 11.1 |
| Evilmeister | R/L | 19.3 | 8 | 57 | 9.9 |
| Evil Genius | M/G | 22.6 | 7 | 64 | 8.6 |
| Baddie 3 | Shotgun | 26.9 | 6 | 70 | 7.4 |
| Baddie 3 | M/G | 27.4 | 6 | 76 | 7.4 |
| Baddie 2 | R/L | 33.1 | 5 | 81 | 6.2 |

For those of us who are curious, if we had still been using the previous method of generating weights to arrive at this set, we would have needed to use a coefficient of 21. Of course, we had no way of knowing that ahead of time, nor did we have any reasonable method of guessing it. (Any guess we had used may not have worked for some arrangements of data, anyway.) A glance down the list shows that our granularity is such that we still show the subtlety of variation between the options. The only two items that have the same weight are the sixth and seventh options. Looking at their scores, however, we have to concede that they were almost identical anyway. For our purposes, the 0.1% difference is loose change. More importantly, our weights are now integers; therefore, our edges are now integers as well. By generating a random number between 0 and 81, we can select a random option whose rate of occurrence reflects the percentages shown. Our agent is now still reasonable, but neither rigidly predictable nor boring. Out of the 32 original possibilities, we will observe our agent selecting from among the eight best options proportionally distributed by their relative merit.

### PUTTING I TIN CODE

To accommodate the above discussion, the changes that we make to our code are relatively minor. First, we must change the struct that we use for the target information to hold an edge value. struct sTARGET_INFO

{

CDude* pDude;

CWeapon* pWeapon;

double Score;

USHORT Edge;

bool operator<( const sTARGET_INFO& j ) {return Score < j.Score;}

};

Next, we need to add a function that calculates the edges. Because we need to have the scores completely filled out to determine weights and edges, the process of calculating this information cannot begin until we have finished scoring all the targets. BuildEdges() also uses the functions SumScores() and ScoreToWeight(). void CAgent::BuildEdges(USHORT NumBuckets)

{

double TotalScore = SumScores( NumBuckets );

mvTargets[0].Edge = ScoreToWeight( mvTargets[0].Score, TotalScore );

for ( USHORT i = 1; i < NumBuckets; i++ ) {

mvTargets[i].Edge = mvTargets[i-1].Edge +

ScoreToWeight( mvTargets[i].Score, TotalScore ); } // end for

}

double CAgent::SumScores(USHORT NumBuckets)

{

double TotalScore = 0;

for ( USHORT i = 1; i < NumBuckets; i++ ) {

TotalScore += mvTargets[i].Score;

} // end for

return TotalScore;

}

USHORT CAgent::ScoreToWeight(double ThisScore, double TotalScore)

{

// note the 0.5 addition to round rather than simply truncate

USHORT Weight = USHORT( ( TotalScore / ThisScore ) + 0.5 );

return Weight;

}

As we can see, SumScores() simply loops through a number of items in mvTargets set by the value NumBuckets and totals their scores. We can then pass TotalScore into ScoreToWeight with each of the records’ scores to calculate the new weight value. Despite its simplicity, we have split ScoreToWeight() out as a separate function. We never know when we may want to modify (and likely complicate) the algorithm later.

In this example, notice that we don’t store the actual weight of each bucket but rather use it immediately to calculate the new edge of that bucket. If we ever needed to, we could calculate the weight of any bucket by comparing its edge to that of the bucket preceding it. On the other hand, depending on how often our data changed and how much of the data we expected to change at any one time, there are times when storing the weights would be advantageous. This is the approach we used in Chapter 12.

Because we need to sort our target vector to determine the top n targets (in this case, eight), we put the call to BuildEdges() into SelectTarget() after we have already scored and sorted the target information. void CAgent::SelectTarget()

{

ScoreAllTargets();

std::sort( mvTargets.begin(), mvTargets.end() );

USHORT NumberToConsider = mvTargets.size() * 0.25;

BuildEdges( NumberToConsider );

// USHORT i = rand() % ( NumberToConsider - 1 ); // <— The old way

USHORT Guess = rand() % ( mvTargets[NumberToConsider - 1].Edge );

USHORT i = GetResult( Guess, NumberToConsider );

mpCurrentTarget = mvTargets[i].pDude;

mpCurrentWeapon = mvTargets[i].pWeapon;

}

Notice that, for comparison purposes, we have left in the original method for determining the index of the target to select (commented out). In that method, we selected a random index between 0 and the last record we were considering (NumberToConsider – 1). Instead, rather than generating a random index, we generate a random guess between 0 and the edge of the last bucket (in our example, 81). We then need to find out into which bucket the random guess falls. We do this by passing it into GetResult(). Our implementation of GetResult() here is similar to the one that we originally used in Chapter 12. The implementation is the same “divide and conquer” binary search. The only difference is that we have included a parameter, NumBuckets, to state how many of the buckets we are searching. USHORT CAgent::GetResult( USHORT Target, USHORT NumBuckets )

{

// Bucket indexes

USHORT iHigh = NumBuckets;

USHORT iLow = 0;

USHORT iGuess;

bool found = false;

while ( !found ) {

// Guess is halfway between the low and high indexes

iGuess = iLow + ( ( iHigh - iLow ) / 2 );

// Check for correct guess

if ( InBucket( iGuess, Target ) ) {

return iGuess;

} // end if

// If not correct...

if ( Target > mvTargets[iGuess].Edge ) {

// guess is too low, change the bottom boundary

iLow = iGuess;

} else {

// guess is too high, change the top boundary

iHigh = iGuess;

} // end if

} // end while

// Code should never get here!

assert( 0 && “Code fell through while loop!”);

return 0;

}

GetResult() returns the index of the target record that we are randomly selecting. We have now replaced the evenly distributed random target selection with a selection process that proportionally weighs the relative scores of those targets.

We are now relieved of the problem of not knowing how those top eight selections are distributed ahead of time. They can be similar or widely disparate, and our algorithm would account for it. We could also change the number of buckets we are searching. If the number of possible target selections was larger or smaller, the 25% of them that we are allowing for consideration would grow or shrink, respectively. Even if we changed our minds and decided that we wanted to consider more or less than 25%, our algorithm would score, weigh, and select from among the choices.

### WEIGHTED RANDOM FROM ALL CHOICES

Let’s recap a few points. Our algorithm adapts to changing numbers of options. It automatically puts those options into the proper distribution to account for how well (or poorly) the option scored when we analyzed our situation. The question is, if our algorithm takes all of that into account, why are we limiting our choices anyway? We have already noted not only the difficulty in determining an abstract cutoff point, but we have noted that by having a cutoff point at all, we risk disqualifying potentially viable options. Looking back at our original data for the Dude example, while we cut off the options at eight, the difference in score between the first 12 options is minimal. Why wouldn’t we consider those extra four? Thankfully, the weighted random approach biases our results toward the options we want and biases away from the options we don’t. We have reduced the options that are very bad to having almost no chance whatsoever of being picked. To see this effect, we can run the weighting algorithm on all 32 of our possible options.

| Name | Weapon | Score | Weight | Edge | % |
| --- | --- | --- | --- | --- | --- |
| Evil Genius | R/L | 7.7 | 2,343 | 2,343 | 10.07 |
| Boss Man | R/L | 8.5 | 2,114 | 4,457 | 9.08 |
| Baddie 3 | R/L | 9.7 | 1,860 | 6,317 | 7.99 |
| Evilmeister | R/L | 9.8 | 1,834 | 8,151 | 7.88 |
| Evil Genius | M/G | 10.8 | 1,670 | 9,821 | 7.18 |
| Baddie 3 | Shotgun | 11.2 | 1,607 | 11,428 | 6.90 |
| Baddie 3 | M/G | 12.6 | 1,430 | 12,858 | 6.14 |
| Baddie 2 | R/L | 13.1 | 1,374 | 14,232 | 5.90 |
| Evilmeister | M/G | 13.8 | 1,303 | 15,535 | 5.60 |
| Evil Knievel | M/G | 14.4 | 1,254 | 16,789 | 5.39 |
| Boss Man | M/G | 14.8 | 1,221 | 18,010 | 5.25 |
| Baddie 2 | M/G | 16.3 | 1,106 | 19,116 | 4.75 |
| Baddie 1 | R/L | 27.1 | 666 | 19,782 | 2.86 |
| Baddie 3 | Pistol | 28.7 | 628 | 20,410 | 2.70 |
| Baddie 1 | M/G | 28.8 | 627 | 21,037 | 2.69 |
| Baddie 1 | Shotgun | 29.4 | 613 | 21,650 | 2.63 |
| Baddie 1 | Pistol | 38.6 | 468 | 22,118 | 2.01 |
| Evilmeister | Pistol | 44.0 | 410 | 22,528 | 1.76 |

| Baddie 2 | Pistol | 54.2 | 333 | 22,861 | 1.43 |
| --- | --- | --- | --- | --- | --- |
| Boss Man | Pistol | 100.0 | 180 | 23,041 | 0.77 |
| Evil Genius | Pistol | 257.7 | 70 | 23,111 | 0.30 |
| Baddie 4 | R/L | 634.7 | 28 | 23,139 | 0.12 |
| Baddie 4 | M/G | 637.0 | 28 | 23,167 | 0.12 |
| Baddie 4 | Pistol | 654.5 | 28 | 23,195 | 0.12 |
| Evil Knievel | Pistol | 1,339.7 | 13 | 23,208 | 0.06 |
| Evil Knievel | Shotgun | 1,339.7 | 13 | 23,221 | 0.06 |
| Evil Knievel | R/L | 1,339.7 | 13 | 23,234 | 0.06 |
| Evil Genius | Shotgun | 1,777.0 | 10 | 23,244 | 0.04 |
| Evilmeister | Shotgun | 2,183.4 | 8 | 23,252 | 0.03 |
| Boss Man | Shotgun | 2,421.3 | 7 | 23,259 | 0.03 |
| Baddie 2 | Shotgun | 2,428.8 | 7 | 23,266 | 0.03 |
| Baddie 4 | Shotgun | 2,531.1 | 7 | 23,273 | 0.03 |

There are a few things to notice about this data. First, a quick glance shows that our selections at the bottom are all but disqualified from consideration. We only have a 1% chance of selecting one of the 12 worst options combined. But the percentages for the good choices are not that much different from what they were when we were only considering eight options. Our best option, Evil Genius with the rocket launcher, represented 16.5% of the choices originally. In this variation, it will occur slightly more than 10% of the time. However, we have now included more potential options from which our agent can select. In particular, the 9th through 12th options are in play with reasonable frequency. Our original arbitrary choice of selecting from only eight options had excluded these choices despite having similar scores to the top eight. Only after the 12th item (Baddie 2 with the machine gun), when the score jumps from 16.3 to 27.1, does the percentage chance of occurrence drop from around 5% to below 3%. We only have to make two changes to our code. One is to pass the total number of buckets into GetResult() rather than the 25% (or whatever) that we had calculated and passed before. The other, more beneficial change is that we can now remove the call to std::sort() that we had made prior to calling BuildEdges() and GetResult(). Because we're now selecting from all of our options, we no longer have to be concerned with the order in which they appear in our vector. The sizes of the buckets determine the probability of selection, not the order in which they appear. If we're processing a decision many times—especially if it is for many agents—sorting the

list to provide either the single best option or the top n candidates can be a major performance hit. Now, by considering all options, we only need to pass through the vector once to weight them and build the edges, and then select.

Remind Me Why We Did This? Spreading the behavior possibilities around over more options may not seem important when considering one agent acting one time. When we think back to our bank example, however, we remember that when we use one algorithm to simultaneously drive many agents, we run the risk of having those agents exhibit identical behaviors. If the Dudes faced 100 agents whose weaponry and distances from the Dudes were equal, they would see a large variety of reactions from those agents. Most of them (approximately 82) would select from those top 12 “reasonable” behaviors—between 5 and 10 agents selecting each of the 12. About 17 of them would select from the 8 “not quite as good” behaviors—making about 2 agents for each behavior. And one of our 100 agents is going to look…ahem… like he’s only burning about 15 watts, if you know what I mean. This may be alarming to some people. “Why would we want any of our agents to do something that looks dumb?!” The simple answer is… because real people sometimes do things that look dumb. The more involved answer is based more on the idea that we are facing many, many agents. In the previous example, having 1 of the 100 agents doing something odd provides a function of variety rather than one of accuracy. He is the person who guessed the impossible solution of greater than 66 in the Guess Two-Thirds the Average Game. Our focus is on the people who guessed reasonably despite the fact that there are unreasonable guessers in the mix. By making some of the 100 agents more dangerous, it provides us with an “interesting choice.” As a player, we would have to determine which of the enemies is more of a threat to us rather than simply take the “kill ’em all!” approach. This aspect can make the encounter more engaging, interesting, and fun. Similarly, one agent running the algorithm with the same inputs many times in succession can produce repetitive behaviors. By stepping away from the narrow view that we should only consider the best-scoring action, we open up far more varied behaviors. More importantly, because we’ve already done all the work to score each of the potential actions anyway, the extra few steps above are only a small addition.

Expanding Our Horizons One last aspect of this approach is worth mentioning again. In the previous example, we had 32 possible choices. By approaching this problem in such an open-ended fashion and by utilizing the very scalable binary search method, we do not have to be daunted by including scores—or even hundreds—of possible choices.

Imagine for a moment a character in a role-playing game (RPG). The character may have 4 different weapons to select from just as our agent above did. In addition to that, however, our character may have 3 offensive magic items, 2 defensive magic items, 8 different types of potions, 20 spells (some offensive, some defensive), and a number of non-combat options. If he was also facing multiple opponents whom he could attack (in any of those methods) and had multiple allies whom he could assist, the number of possible actions would increase geometrically. Using the methods we have discussed, we can score each of the possibilities in the same manner, weight them appropriately, and easily select a weighted random result from among them—even if there are hundreds of possible actions. We do not need to be horrified by the prospect of our agents having that flexibility. We can give them the freedom to choose and feel comfortable that they will exhibit that precious balance of rationality and variety.

### SCORES AND WEIGHTS

We made one very significant assumption in the examples in this chapter: Scores can be easily converted to weights. In both methods of converting the Dude assault score to a weight, we proceeded on the premise that the distribution of scores would provide proper weights for us with a minimum of manipulation. In this case, the outcome seemed reasonable. However, that may not always be the case. (For that matter, we don’t even know that this is the case in what we did above.) If we think back to the gymnastics and skating examples, the scoring systems that they use have all of the contestants compressed into a very small range. There is little difference between them. Furthermore, the 0.05 difference between 9.85 and 9.90 may not mean the same as the 0.05 difference between 9.45 and 9.50. We just don’t know what those scores mean. Because of the lack of differentiation between the scores, it might be difficult to convert this scoring system into a weighted selection system that represents the range from excellent to horrible. We can say the same for our abstract score for the dudes. As we processed that score, we went from very concrete numbers like “damage per second” and “distance to detonator” to more abstract “threat ratios.” Unlike the concrete values, our final score is not something that we can look at and mentally decipher. Thankfully, it seems that this system worked properly. Testing and observation may lead us to adjust the formula that we use to convert the scores to weights, but for the most part we are in the right neighborhood.

Measure Twice, Cut Once It is generally a good idea, however, to build our process with this eventuality in mind. If we are planning on using weighted randoms to select from among our scored behaviors, we need to ensure that our scoring system will enable the process of weighting. That is not to say that the scores themselves need to be weights, but we need to design them in such a fashion that they are convertible. To an extent, we helped ourselves in this fashion when we decided to set the undesirable “we can’t kill him with this weapon” selections to 1,000. This effectively took those options off the table. If we had elected to use a smaller number such as 100, those options would have been scored much more favorably relative to the other scores and would, therefore, have been weighted more highly. By changing that value to 2,000, we would have achieved the opposite effect—minimizing the probability of those options occurring even more than they already are.

Use the Right Tool for the Job Additionally, remember that we discussed a number of techniques in Chapter 10 that helped us convert one set of numbers to another. These tools are very powerful when they are wielded by the hands of creativity. If we wanted to bias the Dude’s scores more toward the best possible selection, we could have used a coefficient— or even an exponent—to decrease the weights of the less-preferable selections and increase the weights of our preferred options. Using the more advanced functions such as the logistic curve can provide interesting results as well. In a situation where the “right answer” is in the middle of the group, for example, we can use the logistic curve to weigh the other answers above and below that midpoint in a more expressive fashion than the data may otherwise exhibit. In other cases, which we didn’t touch on here, we could have applied a random value generated by a normal distribution to account for errors in observation, measurement, or just plain differences in personality. Think of what the effect would be if we had applied a small plus-or-minus x% value to any or all of the steps along the way. This has the effect of “fuzzying up” variables from one iteration to the next so that we don’t arrive at the same outcome every time. Most importantly, remember the principle of compartmentalized confidence. If we know that we are eventually going to use the scores as weights, we can apply these functions along the way to minimize the manipulation in that final step of converting a score to a weight. The possibilities are dizzying. And because the depth and breadth of human behavior is just as immense, there are no final answers. To say it one final time in this chapter, every solution is problem-specific. The power is in your hands.

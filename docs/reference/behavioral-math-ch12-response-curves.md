# Behavioral Mathematics for Game AI — Chapter 12: Response Curves

> Extracted from `docs/reference/BehavioralMathematicalforGameAI.pdf` (Dave Mark, 2009)  
> PDF pages 304–337 · Book pages 285–318

---

## 12. Response Curves
One of the drawbacks of some of the functions we looked at in Chapter 10 is that we are entirely at the mercy of the mathematics. Even if we could construct a function that gives us close to the shape that we wanted, we are still stuck with every bit of it. We have no way of tweaking a portion here or there to be “just a bit higher” or “not quite as steep in this part.” More importantly, there are plenty of situations where no mathematical function—no matter how convoluted —is going to give us anything close to what we need. Similarly, in Chapter 11, some of the probability distribution functions we examined allowed us to construct pretty curves but did not offer a way for us to extract a random number from them. Being able to do so is important if we are to use those probability distributions to construct decisions and behaviors. It doesn’t matter if we know that choice x should occur y% of the time if we can’t cause x to occur at all. For example, while we know that a coin should land on heads 50% of the time, until we actually toss the coin, we won’t know who wins the coin toss. We know that rolling a 7 on 2d6 occurs 16.7% of the time—and yet the nice man running the craps table will be very annoyed with us if we never actually throw the dice. To address both of these problems, we need to introduce a new method of dealing with functional data. To solve the problem of customizability, we need a way to store the results of a function, tweaking those results at will, and extract what we need out of it. To solve the problem of extracting a random number x, y% of the time as determined by a continuous probability distribution function, we need to do something very similar: store the results in a data structure that allows for retrieval in the appropriate proportion of occurrences. Response curves handle both of these situations admirably.

### CONSTRUCTING RESPONSE CURVES

One of the advantages of implementing response curves is that it gives us a new way of looking at data. By changing our vantage point, so to speak, we are able to process this data in ways that are more conducive to manipulation and selection. We will start with a simple example from the previous chapter… our helpful dentists. As we have recalled a few times, rumor has it that “four out of five dentists recommend sugarless gum.” If we were to generate random dentists from this data, we would want to make sure that 80% of the time, the dentist was of the mind to recommend sugarless gum. According to what we’ve been told, that’s realistic, right? Certainly, there are plenty of ways that we could generate a sugarless dentist 80% of the time. It is actually a rather simple exercise. However, for purposes of this example, let’s look at the histogram from Chapter 11. On the left side of Figure 12.1, we see the histogram representing the dentist data.

*FIGURE 12.1 By laying the histogram bars end-to-end, we lay the results over a number line. This allows us to mark the beginnings and ends of each range.*

I don’t mean to wander into pedantic territory here, but there is an aspect of this histogram that we should make a note of. By the very nature of histograms, we know that the “yes” bar is four times the size of the “no” bar. After all, another way of expressing the recommendations of the dentists is “dentists recommend sugarless gum at a 4-to-1 ratio over gum with sugar in it.” It logically follows that a representation that measures ratios should be ratio-based in its portrayal. However, these two vertical bars don’t do us much good for randomly selecting which camp our prospective dentist is in.

From Bars to Buckets If we were to turn the bars on their sides, however, and lay them end to end, we change our perspective. The bars are still in the same proportion as they were before: 4 to 1. By placing them in this orientation (Figure 12.1, right side), we can see how they lie across what is now an x-axis. In this example, the “yes” answers run from 0 through 4 and the “no” answer is the single unit between 4 and 5. We can refer to these ranges as buckets—a name that makes sense when you extend the metaphor slightly. Imagine a game of randomly dropping a ball into these buckets (not too dissimilar from the game Plinko from the game show, The Price Is Right). If the ball drop is truly random, it would have a 4:1 chance of landing in the “yes” bucket. This is a result of the “yes” bucket being four times as big as the “no” bucket. Now, because these are not real buckets and we are not dropping a real ball, we have to simulate dropping a ball into the buckets. To do this, we generate a random number between 1 and 5. By referring to the number line below our buckets, we can determine which number corresponds to which bucket. For example, if we were to roll a 2 (using our dice terminology again), we would let that signify that our random ball has fallen into the “yes” bucket. In fact, if we were to roll a 1, 3, or 4, our ball would have landed in the “yes” bucket as well. On the other hand, if we were to roll a 5, our ball would have landed in the “no” bucket. The difference is that the ball landed on the other side of the edge that defines the separation between the buckets—in this case, the edge is 4. In such a simple example, all of this seems rather obvious. However, as we shall see, there is a lot of potential wrapped up in this method of approaching random selection.

Adding More Buckets For a slightly more involved scenario, let us return to another example from the previous chapter. When we were trying to re-create the results of the Guess Two-Thirds Game, we identified four segments of the population that had distinct characteristics. Each of those four types of guessers had their own method of approaching the game. We also identified what we believe to be the relative occurrence percentages of the four groups. To reiterate:

| Group | % |
| --- | --- |
| “33” guessers | 4 |
| “22” guessers | 3 |
| Random guessers | 30 |
| Semi-logical guessers | 63 |

We can lay out this data in a similar fashion as we did with our dentist data. Just as we based the sizes of the “dentists’ recommendation” buckets on the ratio of those recommendations, we construct our buckets based on the relative sizes of the four segments of the “guesser” population. When we lay them end-to-end, the total width is 100. Because the figures were percentages of the whole, and we have accounted for all of the groups that make up the whole, it makes sense that they add up to 100. (We will find later that this is not a necessity.) Once again, by laying the buckets side-by-side over our x-axis, we can determine the edges of the buckets (Figure 12.2). By dropping our metaphorical ball into the buckets (by generating a random number between 1 and 100), we determine which population segment our next guesser is going to represent. Theoretically, 63% of the guessers are going to be semi-logical, 30% will be random, and so on. While the relative frequencies of the “33” and “22” guessers are small, there still is a possibility that our ball will find its way into one of those two buckets.

*FIGURE 12.2 The buckets created by arranging the relative population segments of the Guess Two-Thirds Game. Note that the proportional sizes of the buckets persist regardless of in what order we place them.*

Notice that the order that we place the buckets in doesn’t matter. In the bottom half of Figure 12.2, we moved the buckets into a different arrangement. However, because the sizes of the buckets haven’t changed, the odds of our random ball dropping into any one of them do not change either. For example, there is still a 4% chance of a “33” guesser appearing.

The locations of the edges of the buckets do change, however, and this is where our focus must lie.

### BUILDING BUCKETS

The numbers at the bottom of each of the two depictions in Figure 12.2 represent the cumulative sizes of the buckets that we have added. For example, in the top group, the first bucket we added was the probability of the group that guesses “33.” We had determined that the size of that group was 4%. The edge of this bucket is, therefore, 4. If our random number is 1, 2, 3, or 4, we pick the first group. It is important that we notice that the right edge of a bucket is inclusive. For the bucket above, the edge is 4, not 5. We can think of this as being “anything in the 4s is still fair game… but 5 is on the other side.” This will be an important distinction to remember as we write our code later. The second group that we add—the “22” guessers—occurs 3% of the time. We add this 3% to the original 4% from the first group. Therefore, they would occupy the next three slots on the number line—5, 6, and 7. The bucket edge for this group would be 7. As we will explore later, we only need to store one edge for each bucket. We can infer the other edge by the bucket immediately to the left. The third group, the random guessers, represented 30% of the whole. As above, we add 30 to the edge of the preceding group (7). Therefore, the edge of this bucket would be 37. If our randomly generated number falls anywhere in the range of 8 to 37, we select the third bucket. Naturally, we repeat the process for the fourth bucket, the semi-logical guessers. The width of their bucket is 63; their bucket spans the range from 38 to 100. It is important that we do not simply assume that anything that doesn’t land in the first three buckets lands in the fourth. We need to make sure that we keep track of the actual width that we intend the last bucket to be. The reason this is important is that we do not want to assume the total width for the combined buckets. We shall revisit this issue in a moment.

### PUTTING I TIN CODE

In the previous chapter, we laid out some code for selecting which of the four groups of guessers we were going to generate. The code for that was relatively simple. GUESS_TYPE CGuesser::GetGuessType()

{

int index = DieRoller.SingleDie( 100, false );

// 1..4 = 4

if ( index <= 4 ) return GUESS_33;

// 5..7 = 3

if ( index <= 7 ) return GUESS_22;

// 8..37 = 30

if ( index <= 37 ) return GUESS_RANDOM;

// 38..100 = 63

return GUESS_SEMI;

}

By looking closely, we can see some familiar numbers. In each of the if statements, we were checking to see if the random number was less than a specified number. The first one, if ( index <= 4 ), is testing to see if index lands in the first bucket (the “33” guessers). Likewise, the second statement, if ( index <= 7 ), is testing to see if our random number lands in the second bucket—between 5 and 7 inclusive. This continues to the third if statement, checking to see if the number is between 8 and 37 (inclusive). If we have not exited the routine after the third statement, the number is above 37, and we return GUESS_SEMI from the function. This arrangement is a very familiar construct to most programmers. While it is certainly functional, it has one serious drawback. If we want to change the bucket widths—even just one of them, we have to change some (or even all) of the if statements. Specifically, we have to change the statement for the bucket we are changing the size of and all the ones that occur after it. The worst-case scenario occurs if we want to change the first bucket. That means we have to change all of the if statements in the entire function. For example, if we decide that the “33” guessers occur 5% of the time instead of 4% (at the expense of 1% of the semi-logical guessers), our new code would look like this: GUESS_TYPE CGuesser::GetGuessType()

{

int index = DieRoller.SingleDie( 100, false );

// 1..5 = 5

if ( index <= 5 ) return GUESS_33;

// 6..8 = 3

if ( index <= 8 ) return GUESS_22;

// 9..38 = 30

if ( index <= 38 ) return GUESS_RANDOM;

// 39..100 = 62

return GUESS_SEMI;

}

As we can see above, we had to change all three if statements, increasing the test number by one in each case. It is easy to see that this is not a very flexible way of laying out code. Trust me: As I was fine-tuning this example in the last chapter, I changed those numbers a few times… it’s not fun. (The process was made even more annoying by the fact that I had to change my comments as well.) There are a few considerations over and above the time consumption argument. First, it is ridiculously prone to errors. For example, if we forget to change one of those numbers, we are going to skew the probability of two occurrences rather than just the one that we are changing. Second, the difficulty in keeping track of our problems increases with the number of possibilities. The above example had only four selections (which gives us three if statements. If we have dozens… or even scores of possible actions, managing the bucket edges efficiently gets prohibitive quickly. Perhaps the most problematic issue in constructing the probabilities in this manner is the fact that it is hard-coded, however. We have no way of changing the edges during run time. This goes beyond the ability to have data-driven code such as probabilities based on a difficulty setting that a designer sets beforehand. We have no way of efficiently changing these values on the fly. We will address the myriad uses for this later on in the book. The solution to this is to store the edge values in a data structure. For example, we will create a struct named sGUESSER_BUCKET in our project that represents a bucket. Each bucket represents one type of guesser. The components of sGUESSER_BUCKET are simple: a width, an edge (both of type USHORT), and a GUESS_TYPE.

typedef enum {

GUESS_33,

GUESS_22,

GUESS_RANDOM,

GUESS_SEMI,

} GUESS_TYPE;

typedef unsigned short USHORT; // for simplicity of declaration

struct sGUESSER_BUCKET

{

USHORT Width; // the actual width of the bucket

USHORT Edge; // the calculated edge of the bucket

GUESS_TYPE GuessType; // the guess type this bucket represents

};

Once we have defined our bucket structures, we create a vector of them: typedef std::vector< sGUESSER_BUCKET > GUESS_TYPE_LIST;

GUESS_TYPE_LIST mvGuessTypeList;

### NAMING CONVENTIONS

Now that the programming part of this book is starting to get more involved, perhaps it is a good time to reiterate some of the naming conventions that I use in my code.

Type and struct names are in all caps: MY_TYPE Struct names are preceded by a lowercase “s”: sMY_STRUCT. Variables and functions are in initial caps: MyFunction( MyVariable ) Member variables of a class are generally preceded with a lowercase “m”: mMyMemberVariable List and vector names are preceded by a lowercase “l” and “v,” respectively. I combine these when necessary such as in a member of a class that is also a vector. In this case the name is preceded by “mv” such as in mvMyMemberVector.

When we run our program, the buckets do not exist in the vector. We need to set them up with the initial data. We do this by setting the data for each bucket and pushing it onto the vector. We can isolate this process in a function such as this: void CGuesser::AddBucket(GUESS_TYPE GuessType, USHORT Width)

{

sGUESSER_BUCKET CurrentBucket;

mMaxIndex += Width;

CurrentBucket.GuessType = GuessType;

CurrentBucket.Width = Width;

CurrentBucket.Edge = mMaxIndex;

mvGuessTypeList.push_back( CurrentBucket );

}

Notice that, despite the fact that our buckets have three members (GuessType, Width, and Edge), we only pass two variables into the AddBucket function. We don’t need to pass in Edge because it is based on the running total of the bucket sizes that have been pushed before it. We track this with the member variable mMaxIndex, which represents the maximum array index of the vector. When we are finished pushing buckets into our vector, mMaxIndex will represent the combined width of all the buckets. To add our four guesser types to this vector, we call AddBucket() once for each type. It doesn’t matter where we get our data. For simplicity’s sake, in this example, we have hard-coded the probabilities for each of the four types. void CGuesser::InitBuckets()

{

AddBucket( GUESS_33, 4 );

AddBucket( GUESS_22, 3 );

AddBucket( GUESS_RANDOM, 30 );

AddBucket( GUESS_SEMI, 63 );

}

In the function above, we are using the same probabilities that we used in Chapter 11 and again in our initial example above. If we decide that we want to change the probability values, however, our task is much simpler now than it was when we were using the if statements. If we want to make the same change to the

data that we did a few pages back (the “33” guessers being 5% and the semi-logical ones being only 62%), we only need to change the two relevant numbers. Our function would now read: void CGuesser::InitBuckets()

{

AddBucket( GUESS_33, 5 );

AddBucket( GUESS_22, 3 );

AddBucket( GUESS_RANDOM, 30 );

AddBucket( GUESS_SEMI, 62 );

}

The bucket edges would now be different than they were with the original numbers. (mMaxIndex still adds up to 100).

### RETRIEVING A RESULT

Once we have our buckets set up, tossing our ball in to determine a result is a fairly simple process. Originally, we generated our random number and then tested it against three if statements to find out which of our four possibilities was selected. That is not much different than what we are going to do here. Thankfully, by holding our results in vector, we can now perform this search in a loop. GUESS_TYPE CGuesser::GetGuessType()

{

// Generate a random number between 1 and mMaxIndex

USHORT index = DieRoller.SingleDie( mMaxIndex, false );

// Count the number of buckets

USHORT NumBuckets = mvGuessTypeList.size();

// Loop through all the buckets

for ( USHORT i = 0; i < NumBuckets; i++ ) {

// See if index fits in this bucket

if ( index <= mvGuessTypeList[i].Edge ) {

return mvGuessTypeList[i].GuessType;

} // end if

} // end for

// Index didn’t land in a bucket!

assert( 0 && “Index out of range” );

// As a default, however, we will return a random guesser

return GUESS_RANDOM;

}

The first thing we did in the function GetGuessType() is to generate our random number, index. There is one change to this line from the technique we used before. Instead of hard-coding the number 100, we changed our random number call to be between 1 and mMaxIndex. This is important. We are now set up to generate a random number between 1 and whatever the combined width of all of our buckets happens to be. For example, if we decided that our “33” guess bucket was a width of 8 wide rather than 4 and made no other changes, the total width of all buckets would be 104 rather than 100 (notice that we are no longer saying “percent”). If we had continued to generate a random number between 1 and 100, we would not be giving full credit to the bucket that we have now pushed to the right— ending at 104 instead of 100. We will address how dynamic bucket widths can be used to our advantage a little later on. The next statement in the function sets NumBuckets to the number of buckets we have in our vector. Again, this is something that we can leverage. If we decide to add a fifth type of guesser to our experiment, this code would automatically account for it. Once we know the number of buckets that we are going to search, we loop through them. The test is the same as we did before: We check to see if our randomly generated index is less than the edge of the current bucket designated by mvGuessTypeList[i]. If it is, we return the GuessType associated with that bucket. If not, we move on.

Note that this code should always return a GuessType before it exits the loop. I leave it to you, gentle reader, to insert error-trapping code of your choice (such as my assert() function), return a default GuessType, or devise any other manner of graceful exit.

There is another method of finding out in which bucket our metaphorical bouncing ball landed. It will be much more entertaining to set up a few more buckets to search before we open the lid on that method.

### CONVERTING FUNCTIONS TO RESPONSE CURVES

As we touched on at the beginning of this chapter, response curves have another valuable role to play for us. The functions from Chapter 10 are rather inflexible in that we couldn’t tweak specific areas of the curves the way we might want to. We are stuck with whatever was spit out of the equation for a given x value. Similarly, some of the probability distributions in Chapter 11 are function-based. While we can calculate the probability (y) for a given x value, we lack the ability to extract a random x value based on all of the different y probabilities. While these two problems may not seem related, we can actually solve them in much the same manner using response curves. The secret to both solutions is converting the results of the function into a custom response curve. Once we have created the response curve, we have much more flexibility available to us. Before we get too far ahead of ourselves, however, we need to address the methods and code for putting the numbers into the response curve to begin with.

SIMPLE 1-TO-1 MAPPINGS To keep the numbers manageable at first, we will start with a simple equation:

Because we are going to be filling a finite space with our results, it is important that we establish the range with which we are working. In this case, we will limit ourselves to the range 0 ≤ x ≤ 40. As usual, we first need to define our vector. typedef std::vector< double > CURVE_VECTOR;

CURVE_VECTOR mvEquationResults;

The process of filling the vector is rather intuitive.

void CLinearFunction::FillVector( int Size )

{

double y;

for ( int x = 0; x <= Size; x++ ) {

y = ( -2 * x ) + 100;

mvEquationResults.push_back( y );

} // end for

}

To fill mvEquationResults from 0 to 40, we simply call:

FillVector( 41 );

Below is what that data would look like:

| x | y |
| --- | --- |
| 0 | 100 |
| 1 | 98 |
| 2 | 96 |
| 3 | 94 |
| … | … |
| 38 | 24 |
| 39 | 22 |
| 40 | 20 |

Again, this seems ridiculously easy. In fact, it seems like a lot of wasted effort when we could have simply used the equation itself for any value of x. However, this does allow us to manipulate the results of that equation. For instance, if we want y to be the result of the equation except when x = 27, in which case we want y = 1.0, we can change that single entry in the vector.

mvEquationResults[27] = 1.0;

This may seem like an inconsequential benefit at the moment. However, we will soon see that this ability lies at the heart of the power behind response curves. Just for the sake of completeness, we can recover a value by simply retrieving the contents of the vector element.

y = mvEquationResults[x]

Not a lot to it, eh?

ADVANCED 1-TO-1 MAPPINGS The above example is simplified somewhat by the fact that the x range that we are working with is between 0 and 40. We have the luxury of using the vector indices that, for a group of 41 elements, start at 0 and end at 40. We are not able to do this if the range with which we want to deal starts at, for example, 125 and extends to 165.

To accommodate this, we need to abandon using the index of the vector as our x value. Instead, we create a struct that contains data for both x and y. We then can create a vector composed of that struct. struct sELEMENT

{

int x;

double y;

};

typedef std::vector< sELEMENT > ELEMENT_VECTOR;

ELEMENT_VECTOR mvElementVector;

Entering Data Filling the vector with the equation results is not much different with this new twist. Instead of simply pushing a y value onto the next element of the vector, we now store both the x and y value in an sELEMENT and push the whole thing onto the end of the vector. void CLinearFunction::FillVector( int Low, int High )

{

sELEMENT thisElement;

for ( int x = Low; x <= High; x++ ) {

thisElement.x = x;

thisElement.y = ( -2 * x ) + 100;

mvElementVector.push_back( thisElement );

} // end for

}

If, as we stated above, we want to store the data for x values from 125 to 165, we call FillVector with:

FillVector( 125, 165 );

By running this new version of FillVector, we fill mvElementVector with 41 entries. The x values range from 125 to 165, with the corresponding y values being the result of our function.

| For the 41 potential values of vector index i, the corresponding values of the | | |
| --- | --- | --- |
| elements x and y would be: | | |
| i | x | y |
| 0 | 125 | –150 |
| 1 | 126 | –152 |
| 2 | 127 | –154 |
| 3 | 128 | –156 |
| … | … | … |
| 38 | 163 | –226 |
| 39 | 164 | –228 |
| 40 | 165 | –230 |

Extracting a Value Now that the index of the vector no longer corresponds to the x value, recovering the data that we need is a slightly more involved process. There are three primary ways of handling this.

Brute Force The first option we could take is the brute force method. By iterating through the entire vector, we could check all the x values until we find the one we want and return the corresponding y value. This is a perfectly viable solution—especially for small data sets. It looks much like what we did with the guess type function earlier. double CLinearFunction::GetY_BruteForce(int x)

{

for ( int i = 0; i < mvElementVector.size(); i++ ) {

if ( mvElementVector[i].x = x ) {

return mvElementVector[i].y;

} // end if

} // end for

// Code should never get here!

assert( 0 && “Index not found” );

return 0.0;

}

The drawback of this method is that it doesn’t scale well to large data sets. The search time scales linearly with the number of members that we add to our vector. That is, if we have 41 members (as in our example above), we are going to average searching 20.5 members to find our match. If we have 1,000 members in the data set, we are going to be searching 500 of them on average… and as many as all 1,000 in the worst-case scenario.

Offset Certainly, if we know the offset from the vector index to the lowest x value, we could leverage that to find the proper container. For example, we know that our lowest number in this exercise was 125. The lowest index in a vector is 0. Therefore, to find the container that corresponds to any given x value, we subtract 125 from the x value we actually want to look up. If we wanted to find y when x = 130, we could find it this way:

y = mvElementVector[x – 125].y;

If x = 130, the above statement would yield the equivalent of:

y = mvElementVector[5].y;

This method is problematic in that we have to keep track of the offset in a separate location—either hard-coded as a constant such as above or held separately in a variable. Either way, if we change the range of our x values, we have to remember to change this offset. We could avoid this by setting the offset to the x value of the first element in the first place: Offset = mvElementVector[0].x;

y = mvElementVector[x – Offset].y;

The above method works in the scenario that we have set out. It also is significantly faster than the brute force method. There is a better way, however—one that allows us to do some nifty tricks down the road.

Binary Search For those who aren’t familiar with the binary search technique, it is, in essence, a game of “guess the number.” The three possible outcomes are “higher,” “lower,” and “got it!” If you have every played this game, you realize that the most efficient way of guessing the number is to take a “divide and conquer” approach.

For example, if guessing a number between 1 and 100, we want to start at 50. If we are told that the number is higher than 50, we would guess 75. If we are then told “lower,” our next guess would be 62. On each turn, we divide the remaining range in half, thereby maximizing the possibility that it is on either side of an incorrect guess. Compare this to the brute force method we listed above. That approach is analogous to guessing the lowest possible number on each iteration and being told “higher” until, one step at a time, we reach the target. Using that method, the maximum number of guesses is equal to the number of possibilities. If we are guessing a number between 1 and 100, we could end up guessing 100 times to determine the target (if the number was 100). On the other hand, a binary search performs in O(log2 n) time, where n is the number of elements. In the classic “guess the number between 1 and 100” game, we would need a maximum of seven guesses to determine the target. If we were to expand the game from 1 to 200 instead, we would only need one additional guess (eight total). Guessing a number between 1 and 400 requires only nine guesses. Between 1 and 800? Only 10 guesses. It becomes apparent very quickly that a binary search is an efficient way of finding data. The major requirement for a binary search is that the data we are searching is stored in a sorted fashion. If it was not, then we could not determine “higher” or “lower”… only whether or not we were correct. Imagine asking someone to guess the name of an animal. When they did so, we could tell them “correct” or “incorrect.” The answer of “incorrect” doesn’t help them determine what direction they should take on their subsequent guess, however. If we told them that the animal they were attempting to guess was, for example, “earlier in the alphabet” or “heavier” than the one they just guessed, we would be giving them a direction in which to head. With that information, they could use a similar “divide and conquer” strategy to close in on the correct answer. In the example we are working with, the values of x are stored in the vector in sorted order from lowest to highest. That way, when we test the value of x at a particular array index (i), we can determine if we are higher or lower (assuming we are not correct) and move in the proper direction from that point. Therefore, a binary search is a valid approach. The code for a binary search isn’t difficult to write. We need only keep track of the highest and lowest possible bounds at any given time. Then, we calculate the midpoint between them as our guess and check to see if our guess was correct, high, or low. The following searches our vector for the proper value of x to return the corresponding y. double CLinearFunction::GetY_BinarySearch( int x )

{

// Get number of elements in the vector

int iCount = mvElementVector.size();

// Set the boundaries of our search range

// to the first and last elements (remember that

// the vector indices are 0-referenced... that’s

// why we subtract 1 from iCount!)

int iLow = 0;

int iHigh = iCount - 1;

bool found = false;

int i = 0; // the vector index

while ( !found ) {

// use the mid-way point as our index guess

i = iLow + ( ( iHigh - iLow ) / 2 );

if ( x = mvElementVector[i].x ) {

// the guess is correct

return mvElementVector[i].y;

} // end if

if ( x < mvElementVector[i].x ) {

// lower the high boundary to the current guess

iHigh = i - 1;

} else {

// raise the low boundary to the current guess

iLow = i + 1;

} // end if

} // end while

return mvElementVector[i].y;

}

Stepping through the function from the beginning, we first determine the number of elements in the vector. We set our initial bounds for the search at 0 and one less than the number of elements. (Vectors are 0-referenced, so n elements means the last index is n – 1.) In the while loop, we set our guess index (i) to the halfway point between whatever the current high and low are. We then check the value of x held at the point in the vector referenced by i. If it matches the x we are looking for, we are finished and return the corresponding y. If not, we then determine whether our guess was too high or too low and change our upper or lower bounds accordingly. We then repeat the while loop to make a new guess until such time as we guess correctly. Notice that, as written, the while loop should not end because we never change the value of found. If we wanted to, we could write in various error traps to avoid such things as infinite loops. I left them out here for clarity of code. The binary search method has given us a few improvements over the prior methods. As we discussed above, the binary search method is significantly faster than the brute force method—especially when we work with larger data sets. Additionally, unlike the offset method, we no longer have to keep track of the relationship between the vector index and the data contained in the vector. This last part is significant for one last reason: by design, the vector indices necessarily have to increment by one—we may not want to hold our data to the same requirement.

### CONVERTING DISTRIBUTIONS TO RESPONSE CURVES

In the previous examples, we were matching up a single x with a single y. That is, any given input generated an output. However, if we think back to the original (and delightfully simple) dentist example, we encounter a different requirement. We can think of the “ball into bucket” metaphor as having two different types of input. First, we think of the ball in terms of dropping into one of five different segments of the range. In a way, we have recast this as being five buckets rather than two. In this case, buckets 1 through 4 mean that the dentist recommends sugarless gum and bucket 5 indicates that he doesn’t. If we were to create our dentist example using the 1-to-1 methods outlined above, we would be inclined to create a five-unit vector—the first four of which were mapped to one output (“sugarless”) and the fifth mapped to the other (“tooth-rotting”). However, it does seem rather inefficient to have four slots in our vector all pointing to the same outcome. On the other hand, we can also think of the ball dropping into one of only two buckets—the two buckets representing our two choices. It just so happens, of

course, that one of those buckets is four times as large as the other one. The difference is subtle but important from an algorithmic standpoint. It would seem that a more accurate way of modeling this idea would be to truly have only two buckets—that is, two items in our vector. However, we would have to also represent the reality that the first bucket was four times as large as the second one. This is where the edges come into play.

### DATA STRUCTURE

We don’t need to change too much of our data structure to represent this method of thinking. When we use a pattern similar to what we have done already, our dentist recommendation structure would look like this. typedef enum {

SUGARLESS,

SUGARRY

} GUM_RECOMMENDATION;

struct sRECOMMENDATION {

USHORT Size; // The size of this bucket

USHORT Edge; // The edge of this bucket based on its position

GUM_RECOMMENDATION Recommendation; // The actual recommendation

};

typedef std::vector< sRECOMMENDATION > GUM_VECTOR;

We have replaced the x parameter of the struct with two components: Size and Edge. The first one, Size represents the width of the bucket. Edge, on the other hand, represents the position of the edge on the x scale. This is similar to how we were using x in the prior examples. There is not much of a functional difference between the two—the edge value is simply an x value after all. The difference is that we are no longer representing every value of x.

### ENTERING DATA

Entering data into the new structure works from the same premise that we used earlier. The function AddBucket takes a bucket size and a recommendation, places them into a temporary struct, and pushes that struct onto the back of the vector.

void CDentist::AddBucket( USHORT Size,

GUM_RECOMMENDATION Recommendation )

{

sRECOMMENDATION CurrentRecommendation;

mTotalSize += Size; // Calculate the new edge

CurrentRecommendation.Size = Size;

CurrentRecommendation.Edge = mTotalSize;

CurrentRecommendation.Recommendation = Recommendation;

mvRecommendations.push_back( CurrentRecommendation );

}

One important thing to note is how Edge works. As we add each bucket, we retrieve the total size of all the buckets we have added so far. That value represents the right-most edge of the whole collection. Because we are adding our new bucket on the end of the row, the edge of the new bucket is the total size plus the size of the new bucket. We can fill our dentist recommendation list with the following function: void CDentist::InitVector()

{

mTotalSize = 0;

AddBucket( 4, SUGARLESS );

AddBucket( 1, SUGARRY );

}

This adds our two buckets to the vector. After running InitVector(), the data stored in the vector is:

| i | Size | Edge | Recommendation |
| --- | --- | --- | --- |
| 0 | 4 | 4 | SUGARLESS |
| 1 | 1 | 5 | SUGARY |

Converting Functions to Distributions To convert a larger dataset such as the results of a function, we need to automate the process of adding buckets. For this example, we will use an uneven probability distribution applied to 10 items. The probabilities of the 10 items follow the formula:

For a visual reference, the graph of the number of occurrences looks like the one in Figure 12.3.

*FIGURE 12.3 The histogram showing the number of occurrences of each selection is based on the formula y = –1(x – 100) + 12.*

The first thing we do is create the struct that will act as our data bucket. struct sBUCKET {

USHORT Size; // The size of this bucket = probability

USHORT Edge; // The edge of this bucket based on its position

USHORT Result; // The result we are generating = x

};

As with our dentist example, each record holds the size of the bucket, its edge location, and what the result will be. We’ve changed the terminology slightly here.

Rather than referring to an x value as we have done previously, we now call this variable Result. We do this because, with a probability distribution, we are going to be selecting one of our buckets based on the probability represented by Size. We could have also named it something like “Name,” “Selection,” “Action,” or, as in the dentist example, “Recommendation.” What we call it will be case-specific. In any event, it is the name of what the bucket represents. For now, Result it is. As usual, we create a vector to hold our distribution. typedef std::vector< sBUCKET > DIST_VECTOR;

DIST_VECTOR mvDistribution;

We then create our function, InitVector(), that fills our vector with the 10 results that we want to track the probability of. void CDistribution::InitVector()

{

sBUCKET ThisBucket;

USHORT ThisSize;

USHORT MaxItems = 10;

for ( USHORT x = 0; x < MaxItems; x++ ) {

ThisSize = (-1 * ( x -100 ) ) + 12;

ThisBucket[x].Size = ThisSize;

If ( x == 0 ) {

// this is the first entry

ThisBucket[x].Edge = ThisSize;

} else {

ThisBucket[x].Edge = ThisBucket[x-1].Edge + ThisSize;

}

ThisBucket[x].Result = x;

mvDistribution.push_back( ThisBucket );

} // end for

}

In InitVector(), we loop through the 10 items, using the value of x in the equation we specified above to determine the size of the bucket that we then store.

The next step in the above function is slightly different than what we have done before. Rather than hold a separate value for the total size to determine what the edge is, we use the edge of the previous bucket. By adding the size of the current bucket to the prior edge, we determine the edge of the current bucket. (Note that for the first bucket, we don’t have a prior bucket to use. The edge is the same as the size.) Later, when we need to know the last bucket edge—that is, the total width of the group—we can retrieve the edge value of the last bucket. As a last step, we assign the value of x to Result and then push the bucket onto the end of our vector. After running InitVector(), our data will look like this (i is the index of the vector):

| i | Size | Edge | Result |
| --- | --- | --- | --- |
| 0 | 12 | 12 | 100 |
| 1 | 11 | 23 | 101 |
| 2 | 10 | 33 | 102 |
| 3 | 9 | 42 | 103 |
| 4 | 8 | 50 | 104 |
| 5 | 7 | 57 | 105 |
| 6 | 6 | 63 | 106 |
| 7 | 5 | 68 | 107 |
| 8 | 4 | 72 | 108 |
| 9 | 3 | 75 | 109 |

As we can see from the above table and from Figure 12.4, the edges represent the cumulative sizes of the buckets as we “lay them end to end.” The total width of all 10 buckets is 75.

### SELECTING A RESULT

We can retrieve a random result out of the distribution using the binary search method outlined above. The function GetResult() is entirely self-sufficient. That is, we don’t need to pass it or otherwise store the number of items in our distribution or the value of the right-most edge. When we call the function, it returns a random result from mvDistribution determined by where the generated random number lands in the distribution.

*FIGURE 12.4 The histogram in Figure 12.3 rearranged to show the data generated by the InitVector() function. The terminology of the response curve data structure is labeled.*

USHORT CDistribution::GetResult()

{

// The number of buckets in the disribution

USHORT NumBuckets = mvDistribution.size();

// The maximum roll is the edge of the last bucket

USHORT MaxRoll = mvDistribution[NumBuckets - 1].Edge;

// The random number we are looking for

USHORT Target = DiceRoller.SingleDie( MaxRoll, false );

// Bucket indexes

USHORT iHigh = mvDistribution.size() - 1;

USHORT iLow = 0;

USHORT iGuess;

bool found = false;

while ( !found ) {

// Guess is halfway between the low and high indexes

iGuess = iLow + ( ( iHigh - iLow ) / 2 );

// Check for correct guess

if ( InBucket( iGuess, Target ) ) {

return mvDistribution[iGuess].Result;

} // end if

// If not correct...

if ( Target > mvDistribution[iGuess].Edge ) {

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

There is one main difference between this function and the binary search we used earlier. Because the bucket is a range rather than a discrete point on the x-axis, we must perform a slightly more involved check to see if our random number lands in it. To do this, we create a function InBucket that takes our current bucket guess and the random target as parameters. bool CDistribution::InBucket( USHORT i, USHORT Target )

{

if ( i == 0 && Target <= mvDistribution[i].Edge ) {

return true;

} // end if

if ( Target <= mvDistribution[i].Edge &&

Target > mvDistribution[i-1].Edge ) {

return true;

} else {

return false;

} // end if

}

This Boolean function checks to see if the random number Target is between the edge of the specified bucket mvDistribution[i] and the edge of the bucket to the left of it, mvDistribution[i–1]. We need to take care with the operators in the two statements. Because the edge of a bucket is inclusive, we also need to test for equality on the current bucket but not equality of the previous bucket. It is also important that we trap for the possibility that this is the lowest bucket (that is, i is 0). If that is the case, we cannot utilize the edge of the bucket below it without generating an index that is out of bounds (e.g., –1). We only check to see if Target is less than the edge of the bucket. Going back to GetResult(), if InBucket returns false, then we know that our current guess, iGuess, is not correct. We then test to see if our random number (Target) is higher than the edge of our current bucket. If it is, we move the lowest bucket to search up to the current bucket. If Target is lower, we move the highest bucket to search down to the current bucket.

### ADJUSTING DATA

In all of the above examples, we create the response curve at the beginning and do not adjust it afterward. There are times, however, when we would want to adjust the contents of the response curve. It would be inefficient for us to erase all the contents of the existing vector and rebuild it from scratch. Depending on the change that is made, we can use a number of approaches to modify the existing data without having to start over. One of the most common (and most useful) adjustments we can make to the data in a response curve is to adjust the weights of one or more of the buckets. When we are dealing with a 1-to-1 response curve, adjusting the data held in one of the buckets is inconsequential. We return the data held in the bucket that we find at the selected index. The only difference to this approach is when we introduce the binary search. Because we are searching for our result by the edge values rather than the indices, we need to maintain the integrity of the edge values. With the distribution-based

| response curves, when we change the size of one bucket, we also affect the edge | | | |
| --- | --- | --- | --- |
| values of all the buckets after the changed one. It is a requirement that we rebuild | | | |
| the edge values in the vector any time the data changes so the change is reflected in | | | |
| all the edge values. For example, let’s look again at the data from the above uneven | | | |
| distribution: | | | |
| i | Size | Edge | Result |
| 0 | 12 | 12 | 100 |
| 1 | 11 | 23 | 101 |
| 2 | 10 | 33 | 102 |
| 3 | 9 | 42 | 103 |
| 4 | 8 | 50 | 104 |
| 5 | 7 | 57 | 105 |
| 6 | 6 | 63 | 106 |
| 7 | 5 | 68 | 107 |
| 8 | 4 | 72 | 108 |
| 9 | 3 | 75 | 109 |

If we arbitrarily decide to change the weight (“size”) of the result “104” from 8 to 10, we need to make a number of changes. Obviously, the first change is that the bucket at index 4 would now have an edge of 52 rather than 50. (It now stretches from 43 to 52.) However, looking now at the bucket at index 5, when we add its size of 7 to the new edge of 4, we arrive at 59 rather than 57. This process cascades down to the last bucket, whose edge we increase by 2 to a value of 77. The contents of the new vector are (changes from above are emphasized in bold): i Size Edge Result 0 12 12 100 1 11 23 101 2 10 33 102 3 9 42 103 4 10 52 104 5 7 59 105 6 6 65 106 7 5 70 107 8 4 74 108 9 3 77 109

To accomplish this properly, we need to construct a function that rebuilds the edges. Rather than rebuild all of the edges, however, it is more efficient (especially in larger data sets) to rebuild only from the changed bucket forward to the end of the vector. We can do this with a function such as this one: void CDistribution::RebuildEdges( USHORT iStartBucket /*= 0*/ )

{

USHORT VectorSize = mvDistribution.size();

for ( USHORT i = iStartBucket; i < VectorSize; i++ ) {

if ( i > 0 ) {

mvDistribution[i].Edge =

mvDistribution[i-1].Edge + mvDistribution[i].Size;

} else {

mvDistribution[i].Edge = mvDistribution[i].Size;

} // end if

} // end for

}

The function RebuildEdges( USHORT iStartBucket ) will rebuild the edges from the array starting point identified by iStartBucket and continuing on to the end of the vector. If we do not specify a value for iStartBucket, the default of 0 is used and the entire vector is rebuilt. As with our earlier example, we need to account for the possibility that the index is 0. In that case, attempting to access the data at mvDistribution[i–1] would generate an error. If the index is 0, we know that the edge is equal to the size anyway. We can use this function every time a data element in the vector is changed. There is an exception to this approach, however. If we are going to be changing a number of elements at the same time (that is, before we try to extract data from it again), it would be redundant to keep recalculating the edges for each change. It is very likely that we will be rewriting the edge data over and over to account for each change. It is much more efficient to make all of our changes first and then recalculate the necessary edges. (We could either rebuild all the edges at that point or keep track of the lowest numbered index that was changed and start from there.)

### SEARCH OPTIMIZATION

Because we are using the edge values to search for the chosen bucket, we don’t necessarily have to have the buckets sorted by size. However, we can optimize searches through having our buckets sorted by size. This method works very well when there are many buckets of widely disparate sizes. We can construct an example of when this optimization is useful by using a quadratic distribution such as:

If we use this formula over the x range of 0 to 30, we find that the probabilities (y) of any given x range from 100 to 10. That gives us a large difference in bucket sizes. This is even more apparent when we realize that the final edge of these 31 buckets will be at 2155. The first bucket (x = 0) has a 4.6% chance of being picked. On the other end of the spectrum the last bucket has only a 0.5% chance. The important factor to address, however, is the starting point of our “divide and conquer” approach. The purpose of guessing the midpoint is to reduce the amount of space left to search regardless of whether our guess was high or low. By starting with the middle bucket, we are reducing the number of buckets on each side of our guess to a joint minimum. No matter which way we missed (too high or too low), we know that we have reduced the remaining buckets to search as low as we can. In the above examples, we started at the middle bucket found by using the formula:

iGuess = iLow + ( ( iHigh - iLow ) / 2 );

Applied to this example, our initial index would be 15 (Figure 12.5). Upon a little examination, however, we find that the edge of bucket 14 is 1399. That means that almost 65% of the data is below what we calculated as the midpoint of the vector. That also tells us that, if our initial guess of 15 is incorrect, we are going to be moving left more often than right. If that is the case, we would want to optimize for quicker searches on the side of the vector that we are going to be landing in more often. By changing our initial guess to the bucket that is in the middle of the possible choices rather than the middle bucket, we can achieve better results. That is, rather than selecting the middle bucket, we want to select the bucket at the point where we know the ball will fall half the time on one side and half the time on the other side. We can do this by determining a theoretical bucket edge that is half the value of the farthest edge.

*FIGURE 12.5 We can improve the performance of searching for the selected bucket by starting in the middle of the data distribution rather than the middle bucket. Most of the data occurrences are on the left side of the histogram. Therefore, we bias our search to that side by starting with bucket 11 rather than bucket 15.*

In this example, the edge of bucket 30 is 2155. Half of that is 1077. The value 1077 falls into bucket 11. If we examine the bucket below number 11, we find that the edge of bucket 10 is 1062. That tells us that 49.3% (1062/2155) of the values are in bucket 10 or below. On the other hand, the edge of bucket 11 is 1149. There are 1005 (2155 – 1149) numbers above bucket 11. That represents 46.7% of the total. (Bucket 11 is the missing 4.1%.) This means, if the answer is not bucket 11, we have about an even chance of being either too high or too low. To accomplish this, we need a new way to determine our starting bucket. As we explained, this is simply a matter of dividing the highest edge value in half and then searching for it via the original, slightly inefficient method. We then store that value for use in all of our regular searches. We don’t need to do this every time we search. If we did, it would be very inefficient anyway because we would be searching twice for every real search. As long as the data does not change, the suggested starting point does not change. In fact, because the binary search is so fast, as long as the data doesn’t change much, we can still use that suggested starting point.

This point exposes the fact that these optimizations are very situation-dependant. In general, we can follow these rules:

| Any change to a bucket size: | Rebuild edges |
| --- | --- |
| Large changes to bucket sizes: | Re-sort vector |
| Change in lopsided distribution: | Recalculate starting bucket |

By no means is the above an exhaustive list. Depending on the number of elements in a vector, the relative sizes of the elements, the nature of the data, and even how often the data is changed or accessed, we can select our optimization and search methods to provide the best possible response. For example, the only time that we would want to re-sort the vector and change the start bucket is if we are dealing with very large data sets (e.g., hundreds of buckets) or are doing many searches on a data set that changes rarely (or both). In either case, we can see some minor gains in performance. However, we need to be aware that the sort and recalculate optimizations should happen very rarely lest we give back the gained time through the time it takes to perform the actual optimization.

H AND -C RAFTED R ESPONSE C URVES

Other than the occasional manual tweak, we have generated most of the response curves in this chapter through a function of some sort. This is not a necessary limitation. It is not only possible, but very useful to hand-craft a distribution to match a desired effect. Thinking back to our five dentists, we manually created a two-bucket response curve representing the two bits of masticatory advice a dentist could offer to us: sugarless gum or sugared gum. We also manually set our two sizes of four and one, respectively to simulate the legendary “four out of five dentists surveyed” phenomenon. We also used this same approach to model the four types of guessers in the Guess Two-Thirds Game. These are very small examples of hand-crafted distribution. Applying this process to a more extreme example, rather than go through the great pains that we went to in Chapter 11 to model the results of the Guess Two- Thirds contest run by the University of Copenhagen that we explored in Chapter 6, we could craft a 101-bucket response curve that exactly duplicated the results of the study. Rather than breaking down the four groups and then modeling the distribution of the guesses for each group, we can simply create a single response curve that holds the data for all 101 possible selections. By sizing the buckets according to how the results appeared in the actual contest, we can generate a strikingly similar distribution.

The most common method of constructing hand-crafted response curves is to save the data in a file. At run time, we read the data from the file into the response curve in almost the same looping fashion as generating it from a function. Retrieving the data uses the same process as we have discussed above.

### DYNAMIC RESPONSE CURVES

We can also generate data for a response curve at any point during run time. In fact, as we will find, response curves are at their best when storing and selecting data that is constantly changing. If we think of the buckets as decisions (e.g., “sugarless” vs. “sugary”) and the sizes of the buckets as weights, priorities, or utilities for those decisions, we can see how being able to selected a dynamically weighted decision opens up many methods of adjusting and selecting behaviors. As the game runs, we can change the weights by any method we choose. Any time the weights change, we update the edges, re-sort the vector if necessary, and go about the business of selecting our behaviors. We will deal with this process in more detail in the next few chapters. This page intentionally left blank

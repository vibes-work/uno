# Rules

[Back to README](../README.md)

This game uses house rules rather than the official UNO rules. In particular,
Draw Twos stack, normal draws never pass the turn, and Reverse has no effect
with two players.

## Deck, turns, and winning

The deck contains **108 classic cards plus four wild `?` cards** (112 total).
Each of the 2–10 players receives seven cards. A random dealer is selected for
the first round; dealing rotates between rounds. Play proceeds clockwise unless
reversed.

Match the active color or the discard's number/action face, or play a wild.
Play one card at a time. Jump-ins, 7–0 swaps, and stacking Wild Draw Four or `?`
penalties are not supported.

Empty your hand to win the round. The winner scores all cards remaining in
opponents' hands:

| Card | Points |
| --- | ---: |
| Number | Printed value |
| Skip, Reverse, Draw Two | 20 |
| Wild, Wild Draw Four, `?` | 50 |

The first player to reach **500 points** wins the match. The host advances rounds
or starts a fresh match.

Final-card penalties, including a final Wild Draw Four decision, resolve
**before** scoring. If several players empty their hands while stacking +2 cards,
the first of them whose hand is still empty after the debt resolves wins. A player
who receives cards from the returning penalty no longer qualifies.

## Drawing: one card, no voluntary pass

Click the deck to draw **exactly one card**, even if you already hold a matching
card or have just drawn one. Then play **any matching card in your hand**, or
click the deck again. There is no Keep & pass action: a normal turn ends only
after a valid play.

An invalid play leaves the hand, turn, and penalties unchanged. The discard pile
is recycled except for its top card. If no drawable cards remain, an attempted
draw reports an empty deck without passing. Pending +2 and +4 attacks use their
penalty-resolution rules instead of normal drawing.

## Action cards

- **Skip:** the next player loses their turn.
- **Reverse:** reverse direction. With two players it has no effect; the turn passes to the other player.
- **Draw Two:** the next player may stack a Draw Two of any color, increasing the debt by two and passing it on. Alternatively, **Accept +N** draws the entire accumulated penalty and passes the turn, even if the player holds a +2. No other card can answer this attack.
- **Wild:** choose the next color.
- **Wild Draw Four:** choose a color; the next player accepts four and loses their turn, or challenges (see below).
- **`?`:** choose the next color and any one other player to draw two immediately. The target does not lose their turn; play continues to the next player in the current direction. This card is playable on any discard and cannot be challenged or stacked.

### Wild Draw Four challenges

Wild Draw Four is legal only if the actor held **no card of the previously active
color**. Holding a matching number/action in a different color does not make it
illegal. The client permits a challengeable bluff, as in the physical game.
The host checks the pre-play hand privately:

- **Successful challenge:** the actor draws four; the challenger takes their normal turn.
- **Failed challenge:** the challenger draws six and loses their turn.

No opponent's full hand is exposed to resolve a challenge; the host reports the result.

## Calling and checking UNO

Enable **Call UNO** before playing your second-last card. It stays armed while
you draw during that turn, but resets at the next turn—even when a two-player
Skip returns the turn to you.

The next player in the actual turn order may use **Check UNO**. If the declaration
did not leave exactly one card, the declaring player draws two. A valid declaration
has no penalty for either player. Checking does not consume the next player's turn
or resolve a pending +4.

If you forget to declare with a one-card hand, you may still use **Say UNO** before
being caught. Otherwise, only the next player may press **Missed UNO**, making you
draw two. Declared and missed UNO are mutually exclusive.

The check/catch window closes when the next turn's first valid play, draw, or
Wild Draw Four decision begins. Unplayable cards, empty-deck draws, malformed
requests, and out-of-turn requests do not close it. A false declaration on the
final card delays scoring until checked or waived by the next valid action.
The server serializes simultaneous actions in arrival-processing order.
Automatic penalty draws are not separate UNO declarations.

## Opening discard

| Opening card | Effect |
| --- | --- |
| Skip | Skip the first player. |
| Draw Two | The first player must stack or accept a +2 debt. |
| Reverse | With three or more players, start with the dealer and reverse direction. With two players, the normal first player starts. |
| Wild | The first player chooses a color before playing. |
| Wild Draw Four or `?` | Shuffle it back and select another opener. |

# Controls

[Back to README](../README.md) · [Rules](rules.md)

## Cards and actions

| Control | Action |
| --- | --- |
| Drag a card onto the discard pile | Attempt to play it; wilds open color/target selection |
| Release outside the discard / `Esc` while dragging | Return the card without playing or penalty |
| Click the deck | Draw one card; during a +2 attack, accept the accumulated penalty |
| Call UNO / UNO armed | Toggle UNO with play for this turn only |
| Accept +N | Take the accumulated Draw Two penalty and pass |
| Say UNO | Correct a missed declaration before being caught |
| Accept +4 / Challenge | Respond to Wild Draw Four |
| Missed UNO | Catch an undeclared one-card hand (+2); only the next player may do this |
| Check UNO | Verify a declaration; false UNO costs its author +2 |
| Players / view | Choose table names or a roster with card counts and scores |
| History / `H` | Show recent events; Close or `Esc` dismisses it |
| `Esc` in Rules / wild-card selection | Close Rules or cancel the wild play; at the opening color choice, ask to leave |
| `F11` | Toggle fullscreen (may need `Fn` on Mac) |

Hover over action icons for tooltips. There are no legal-move hints on cards.
Unplayable cards return to the hand without a penalty or turn change.

## Camera

| Control | Action |
| --- | --- |
| Right-edge camera icons | Reveal camera controls and preset tooltips |
| My seat / `1` / `R` | Return to your seat; reset pan, angle, and zoom |
| Above / `2` | Overview of the table |
| My cards / `3` | Close view of your hand |
| Camera `<` / `>` buttons | View from previous/next seats without revealing their cards |
| Right mouse drag / arrow keys | Orbit and tilt |
| Shift + right drag / middle drag / `W A S D` | Move the camera's focus across the table |
| Mouse wheel / `+` / `-` | Zoom in/out |

Vertical mouse movement is inverted for both orbit and pan: dragging upward
lowers the viewing angle or moves the focus away from you. Horizontal mouse pan
is also inverted.

All cards are on the table; large hands use overlapping rows, not pages. Zoom,
pan, or use **My cards** to inspect them. Opponents' cards remain face-down from
every camera position. In multiplayer, turns do not move your camera.

**Players / view** switches between names at the table and a fixed roster showing
card counts, scores, turn, and offline/UNO indicators. Hover a roster row for the
full name; click it to focus the camera on that seat without changing hand ownership.

## Text fields

| Shortcut | Action |
| --- | --- |
| `Ctrl/Cmd+V` | Replace the field with clipboard contents |
| `Ctrl/Cmd+C` | Copy the field's contents |
| `Ctrl/Cmd+X` | Cut the field's contents |
| `Ctrl/Cmd+A` | Select all; typing or Backspace replaces the selection |

The built-in pixel font has limited Unicode coverage. Use Latin-character names
if a glyph is missing. Color names are printed as well as shown visually.

## Self-play

Self-play uses the multiplayer rules engine without a server. You control every
seat; there are no bots.

- After a successful action, the displayed hand follows the turn, including Skip, Reverse, and Wild Draw Four decisions.
- Drawing takes one card and keeps the same hand visible. You may play any matching card already in the hand.
- **Next hand** cycles through seats to inspect cards or return to a player who forgot to say UNO. It does not advance the turn.
- **Turn hand** returns to the player whose turn it is.
- Switching hands resets the camera to that seat. Camera controls alone never change hand ownership.
- **Next round / Start a new match** works from any hand.
- **Leave table** discards the local match and returns to the home screen.

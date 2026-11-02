# Rules

Each player has two 4x4 grids. One is the player's own board, where they place
their "glass" and "mirrors"; the other is where they record guesses about the
opponent's board.

The objective is to figure out the opponent's board before they figure out
yours.

A grid has 16 numbered ports around its edge — light enters and exits through
these. An empty grid looks like this:

```
   1  2  3  4
16 -  -  -  - 5
15 -  -  -  - 6
14 -  -  -  - 7
13 -  -  -  - 8
   12 11 10 9
```

And here it is filled with glass and mirrors:

```
   1  2  3  4
16 -  -  B ↑→ 5
15 -  R  -  Y 6
14 █  -  -  - 7
13 -  -  /  - 8
   12 11 10 9
```

Each letter is a colored glass, `/` is the slash mirror and `↑→` is the arrow
mirror.

## Pieces

Each player places exactly seven pieces — one of each below, plus a second slash
mirror:

- **Red (R)**, **Yellow (Y)**, **Blue (B)** — colored glass.
- **Black (█)** — absorbs all light.
- **Slash mirror (`/` or `\`)** — bends light 90 degrees. Each player gets two.
- **Arrow mirror (`↑→`, `↓→`, `↓←`, `↑←`)** — a corner of two arrows.

## Functionality

Light "shines through" the grid, entering one port and exiting another. A player
calls a port number, and the opponent answers with the exit port and the color.

**Color.** Light starts white. Each colored glass it passes through adds that
color, and colors mix:

| passes through | result |
| -------------- | ------ |
| red + yellow | orange |
| red + blue | purple |
| yellow + blue | green |
| red + yellow + blue | brown |

The **black** piece absorbs the light completely, so any ray touching it comes
out black regardless of what else it passed through.

**Slash mirror.** Bends light 90 degrees. A `/` sends light travelling right
upward (and up→right, left→down, down→left); a `\` sends light travelling right
downward (and down→right, left→up, up→left).

**Arrow mirror.** A corner of two arrows meeting at a right angle (it looks like
two arrows laid on top of each other). Each arrow bounces light that hits its
point head-on straight back the way it came; light arriving from the other two
directions passes straight through. For example, the `↑→` corner sends light
travelling down (into the up arrow) back up, and light travelling left (into the
right arrow) back right, while light travelling up or right passes through
untouched.

Pieces can be stacked along a ray to create complicated paths — light may pass
through a color, then bounce off a mirror, then pass through another color.

### Worked example

For the filled board above:

| call | out | color |
| ---- | --- | ----- |
| 1 | 12 | Black |
| 2 | 11 | Red |
| 3 | 13 | Blue |
| 4 | 4 | White |
| 5 | 5 | White |
| 6 | 15 | Orange |
| 7 | 14 | Black |
| 8 | 10 | White |
| 9 | 4 | Yellow |
| 10 | 8 | White |
| 11 | 2 | Red |
| 12 | 1 | Black |
| 13 | 3 | Blue |
| 14 | 7 | Black |
| 15 | 6 | Orange |
| 16 | 5 | Blue |

For instance, calling 5 enters going left and immediately hits the right arrow
of the `↑→` corner, bouncing straight back out of 5 as white. Calling 4 enters
going down into the up arrow and bounces back out of 4. Calling 16 enters going
right, passes through the blue glass, passes through the `↑→` corner untouched
(it is not hit head-on), and exits at 5 as blue.

## Setup

Each player arranges all six pieces on their own board — the three colors, the
black piece, the slash mirror and the arrow mirror — then locks the board. Play
begins once both players have locked.

## Play

Players alternate turns. On your turn you may, once, **flip** a mirror: pick a
cell on the opponent's board, and if it holds a slash mirror it rotates 90
degrees (`/` ↔ `\`). The opponent is told which cell you flipped whether or not
you guessed right. (The arrow mirror's orientation is fixed at setup and cannot
be flipped.)

You then **shoot a ray** through a port, and the opponent answers with the exit
port and color. Shooting ends your turn.

When you believe you know the opponent's entire board, **call the game** with
your guess. The opponent's board is checked against it: if it matches exactly,
you win; if not, play continues.

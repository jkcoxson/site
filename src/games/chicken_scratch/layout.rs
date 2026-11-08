// Jackson Coxson
// Where every tile sits on the Chicken Scratch board, worked out from the
// order tiles were played. It's the same on every screen, and a tile never
// moves once it's down: each new tile is fitted against the ones already
// there. Lines run straight while they can and bend (90°, then 45°) to get
// around other lines. Doubles lie crosswise, and a chicken foot's toes splay
// out from the double's far side.
//
// World units: a domino is 2 long and 1 wide. +x is right, +y is down, and
// angles are degrees clockwise from +x, the same as CSS `rotate()`.

use std::f64::consts::PI;

pub use crate::games::board::Bounds;

/// Where a tile sits: its center and rotation. Tiles are drawn lying
/// horizontally (2×1) with the number that matched its line on the left,
/// then rotated by `angle` around the center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
}

/// An oriented box: center, half sizes along its own axes, rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Obb {
    cx: f64,
    cy: f64,
    hx: f64,
    hy: f64,
    /// Radians.
    angle: f64,
}

impl Obb {
    fn axes(&self) -> [(f64, f64); 2] {
        let (s, c) = self.angle.sin_cos();
        [(c, s), (-s, c)]
    }

    /// Half the box's extent along a unit axis.
    fn radius(&self, axis: (f64, f64)) -> f64 {
        let [u, v] = self.axes();
        self.hx * dot(u, axis).abs() + self.hy * dot(v, axis).abs()
    }

    fn overlaps(&self, o: &Obb) -> bool {
        let d = (o.cx - self.cx, o.cy - self.cy);
        self.axes()
            .into_iter()
            .chain(o.axes())
            .all(|axis| dot(d, axis).abs() < self.radius(axis) + o.radius(axis))
    }

    fn bounds(&self) -> Bounds {
        let rx = self.radius((1.0, 0.0));
        let ry = self.radius((0.0, 1.0));
        Bounds {
            min_x: self.cx - rx,
            min_y: self.cy - ry,
            max_x: self.cx + rx,
            max_y: self.cy + ry,
        }
    }

    fn shrunk(mut self, by: f64) -> Self {
        self.hx -= by;
        self.hy -= by;
        self
    }
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn unit(deg: f64) -> (f64, f64) {
    let (s, c) = (deg * PI / 180.0).sin_cos();
    (c, s)
}

fn add(p: (f64, f64), d: (f64, f64), k: f64) -> (f64, f64) {
    (p.0 + d.0 * k, p.1 + d.1 * k)
}

/// The open end of a line: where the next tile touches, and which way the
/// line is heading.
#[derive(Debug, Clone, Copy, PartialEq)]
struct End {
    tip: (f64, f64),
    heading: f64,
}

/// When the way straight on is blocked, a line turns a little at a time,
/// this many degrees per step, up to `MAX_TURN`.
const TURN_STEP: f64 = 15.0;
const MAX_TURN: f64 = 135.0;
/// The room kept clear in front of every other open end, so lines don't
/// box each other in: how far out, and how wide.
const KEEP_LEN: f64 = 2.0;
const KEEP_WIDTH: f64 = 1.0;
/// Tiles of room ahead that make a direction good enough.
const LOOK_AHEAD: usize = 2;
/// Tiles that only touch don't count as overlapping.
const SLACK: f64 = 0.03;

/// A tile being fitted: where it goes and the line ends it leaves behind.
struct Fit {
    pose: Pose,
    obb: Obb,
    /// The line's new end, or the three toes for a double.
    ends: Vec<End>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub center: Pose,
    /// Every played tile, in play order.
    pub tiles: Vec<Pose>,
    ends: Vec<End>,
    /// Lines that ended in a double.
    closed: Vec<bool>,
    shapes: Vec<Obb>,
}

impl Layout {
    /// The middle double lies horizontally at the origin with `sides` lines
    /// off it, clockwise; `placed` is each played tile's line and whether
    /// it's a double, in order.
    pub fn new(sides: u8, placed: &[(u8, bool)]) -> Self {
        // Diagonal lines grow from the double's corners.
        let (dx, dy) = (1.0, 0.5);
        let end = |x: f64, y: f64, heading: f64| End {
            tip: (x, y),
            heading,
        };
        let ends = match sides {
            3 => vec![
                end(0.0, 0.5, 90.0),
                end(-dx, -dy, -135.0),
                end(dx, -dy, -45.0),
            ],
            6 => vec![
                end(1.0, 0.0, 0.0),
                end(dx, dy, 45.0),
                end(-dx, dy, 135.0),
                end(-1.0, 0.0, 180.0),
                end(-dx, -dy, -135.0),
                end(dx, -dy, -45.0),
            ],
            8 => vec![
                end(1.0, 0.0, 0.0),
                end(dx, dy, 45.0),
                end(0.0, 0.5, 90.0),
                end(-dx, dy, 135.0),
                end(-1.0, 0.0, 180.0),
                end(-dx, -dy, -135.0),
                end(0.0, -0.5, -90.0),
                end(dx, -dy, -45.0),
            ],
            _ => vec![
                end(0.0, -0.5, -90.0),
                end(1.0, 0.0, 0.0),
                end(0.0, 0.5, 90.0),
                end(-1.0, 0.0, 180.0),
            ],
        };
        let mut layout = Layout {
            center: Pose {
                x: 0.0,
                y: 0.0,
                angle: 0.0,
            },
            tiles: Vec::new(),
            closed: vec![false; ends.len()],
            ends,
            shapes: vec![Obb {
                cx: 0.0,
                cy: 0.0,
                hx: 1.0,
                hy: 0.5,
                angle: 0.0,
            }],
        };
        for &(line, double) in placed {
            layout.place(line, double);
        }
        layout
    }

    /// Where a tile would go on `line`, without putting it there.
    pub fn preview(&self, line: u8, double: bool) -> Pose {
        self.fit(line, double).pose
    }

    /// The point a line grows from.
    pub fn tip(&self, line: u8) -> (f64, f64) {
        self.ends.get(line as usize).map_or((0.0, 0.0), |e| e.tip)
    }

    /// Everything on the board.
    pub fn bounds(&self) -> Bounds {
        self.shapes
            .iter()
            .map(Obb::bounds)
            .reduce(Bounds::union)
            .unwrap_or(Bounds::around(0.0, 0.0, 1.0))
    }

    /// The box around one tile at `pose`.
    pub fn tile_bounds(pose: Pose) -> Bounds {
        Obb {
            cx: pose.x,
            cy: pose.y,
            hx: 1.0,
            hy: 0.5,
            angle: pose.angle * PI / 180.0,
        }
        .bounds()
    }

    fn place(&mut self, line: u8, double: bool) {
        let fit = self.fit(line, double);
        self.tiles.push(fit.pose);
        self.shapes.push(fit.obb);
        let mut ends = fit.ends.into_iter();
        if let (Some(e), Some(slot)) = (ends.next(), self.ends.get_mut(line as usize)) {
            if double {
                // The line ends at the double; its toes are new lines.
                self.closed[line as usize] = true;
                self.ends.push(e);
            } else {
                *slot = e;
            }
        }
        self.ends.extend(ends);
        self.closed.resize(self.ends.len(), false);
    }

    fn clear(&self, obb: &Obb) -> bool {
        let me = obb.shrunk(SLACK);
        !self.shapes.iter().any(|s| s.shrunk(SLACK).overlaps(&me))
    }

    /// The room just past every other open end, so a line doesn't grow
    /// across the front of another and box it in.
    fn keep_out(&self, line: u8) -> Vec<Obb> {
        self.ends
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != line as usize && !self.closed[i])
            .map(|(_, e)| {
                let c = add(e.tip, unit(e.heading), KEEP_LEN / 2.0);
                Obb {
                    cx: c.0,
                    cy: c.1,
                    hx: KEEP_LEN / 2.0,
                    hy: KEEP_WIDTH / 2.0,
                    angle: e.heading * PI / 180.0,
                }
            })
            .collect()
    }

    fn fit(&self, line: u8, double: bool) -> Fit {
        let end = self.ends.get(line as usize).copied().unwrap_or(End {
            tip: (0.0, 0.0),
            heading: 0.0,
        });
        // Straight on first, then turning a little more each try. Of each
        // pair, the side heading away from the middle goes first, so the
        // board spreads out instead of in.
        let mut turns = vec![0.0];
        let mut t = TURN_STEP;
        while t <= MAX_TURN {
            let out = |turn: f64| {
                let p = add(end.tip, unit(end.heading + turn), 2.0);
                p.0 * p.0 + p.1 * p.1
            };
            if out(t) >= out(-t) {
                turns.extend([t, -t]);
            } else {
                turns.extend([-t, t]);
            }
            t += TURN_STEP;
        }
        let keep_out = self.keep_out(line);
        // First keeping clear of other lines' ends (and, for a double, of
        // where its foot will go), then just clear of tiles.
        for polite in [true, false] {
            // Look ahead like a player would: take the gentlest turn that
            // leaves room to keep going, or else the one with the most room.
            let mut best: Option<(usize, Fit)> = None;
            for &turn in &turns {
                let fit = self.shape(end, turn, double);
                let me = fit.obb.shrunk(SLACK);
                if !self.clear(&fit.obb)
                    || (polite
                        && (keep_out.iter().any(|k| k.overlaps(&me))
                            || !self.foot_fits(&fit, double)))
                {
                    continue;
                }
                let room = self.room(&fit, double);
                if room >= LOOK_AHEAD {
                    return fit;
                }
                if best.as_ref().is_none_or(|(r, _)| room > *r) {
                    best = Some((room, fit));
                }
            }
            if let Some((_, fit)) = best {
                return fit;
            }
        }
        // Boxed in: go straight on over whatever's there. Tiles always
        // touch their line, never leave a gap.
        self.shape(end, 0.0, double)
    }

    /// How many more tiles fit straight on past `fit`, up to `LOOK_AHEAD`
    /// (the fewest of a double's toes).
    fn room(&self, fit: &Fit, double: bool) -> usize {
        let me = fit.obb.shrunk(SLACK);
        let run = |mut end: End| {
            let mut n = 0;
            while n < LOOK_AHEAD {
                let next = self.shape(end, 0.0, false);
                if !self.clear(&next.obb) || next.obb.shrunk(SLACK).overlaps(&me) {
                    break;
                }
                end = next.ends[0];
                n += 1;
            }
            n
        };
        let runs = fit.ends.iter().map(|&e| run(e));
        if double {
            runs.min().unwrap_or(0)
        } else {
            runs.max().unwrap_or(0)
        }
    }

    /// Whether a double's three toes would have room, straight out.
    fn foot_fits(&self, fit: &Fit, double: bool) -> bool {
        if !double {
            return true;
        }
        let me = fit.obb.shrunk(SLACK);
        fit.ends.iter().all(|&e| {
            let toe = self.shape(e, 0.0, false).obb;
            self.clear(&toe) && !toe.shrunk(SLACK).overlaps(&me)
        })
    }

    /// A tile on `end` turned `turn` degrees. A turned tile touches the
    /// line's end with its corner, the way a real one does when you angle
    /// it against its neighbor.
    fn shape(&self, end: End, turn: f64, double: bool) -> Fit {
        let heading = end.heading + turn;
        let across = unit(heading);
        // The tile's own half sizes, along its heading and across it.
        let (hx, hy) = if double { (0.5, 1.0) } else { (1.0, 0.5) };
        // Straight on, the tile's near edge sits square on the tip. Turned,
        // it pivots on the tip at its back corner, so it always touches.
        let corner = if turn > 0.0 {
            hy
        } else if turn < 0.0 {
            -hy
        } else {
            0.0
        };
        let center = add(add(end.tip, across, hx), unit(heading + 90.0), -corner);
        let obb = Obb {
            cx: center.0,
            cy: center.1,
            hx,
            hy,
            angle: heading * PI / 180.0,
        };
        let ends = if double {
            let base = add(center, across, hx);
            let side = unit(heading + 90.0);
            vec![
                End {
                    tip: add(base, side, -1.0),
                    heading: heading - 45.0,
                },
                End { tip: base, heading },
                End {
                    tip: add(base, side, 1.0),
                    heading: heading + 45.0,
                },
            ]
        } else {
            vec![End {
                tip: add(center, across, 1.0),
                heading,
            }]
        };
        Fit {
            pose: Pose {
                x: center.0,
                y: center.1,
                angle: if double { heading + 90.0 } else { heading },
            },
            obb,
            ends,
        }
    }

    /// Tiles that overlap another (should be none, barring a boxed-in end).
    #[cfg(test)]
    fn overlaps(&self) -> usize {
        let mut n = 0;
        for i in 0..self.shapes.len() {
            for j in 0..i {
                if self.shapes[i]
                    .shrunk(SLACK)
                    .overlaps(&self.shapes[j].shrunk(SLACK))
                {
                    n += 1;
                }
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::chicken_scratch::logic::{Board, TOP};
    use crate::games::dominoes::full_set;
    use rand::RngExt;

    #[test]
    fn boxes_overlap() {
        let a = Obb {
            cx: 0.0,
            cy: 0.0,
            hx: 1.0,
            hy: 0.5,
            angle: 0.0,
        };
        let b = Obb { cx: 2.0, ..a };
        assert!(
            !a.shrunk(SLACK).overlaps(&b.shrunk(SLACK)),
            "touching is fine"
        );
        let c = Obb { cx: 1.5, ..a };
        assert!(a.overlaps(&c));
        let d = Obb {
            cx: 1.6,
            cy: 1.6,
            angle: PI / 4.0,
            ..a
        };
        assert!(!a.overlaps(&d));
    }

    #[test]
    fn middle_sides_dont_overlap() {
        for sides in [3u8, 4, 6, 8] {
            let placed: Vec<(u8, bool)> = (0..sides).map(|l| (l, false)).collect();
            let l = Layout::new(sides, &placed);
            assert_eq!(l.overlaps(), 0, "{sides} sides");
            // Close in around the middle.
            for p in &l.tiles {
                assert!(p.x.hypot(p.y) < 2.6, "{sides} sides: {p:?}");
            }
        }
    }

    #[test]
    fn a_foot_splays_without_overlapping() {
        let l = Layout::new(
            4,
            &[(0, false), (1, false), (2, false), (3, false), (1, true)],
        );
        let double = l.tiles[4];
        assert_eq!(double.angle, 90.0, "crosswise to a line heading right");
        let l = Layout::new(
            4,
            &[
                (0, false),
                (1, false),
                (2, false),
                (3, false),
                (1, true),
                (4, false),
                (5, false),
                (6, false),
            ],
        );
        assert_eq!(l.overlaps(), 0);
        let angles: Vec<f64> = l.tiles[5..].iter().map(|p| p.angle).collect();
        assert_eq!(angles, vec![-45.0, 0.0, 45.0]);
    }

    #[test]
    fn a_long_line_bends_around_its_neighbor() {
        // Line 1 heads right, then comes a foot pointing up into line 0.
        let mut placed = vec![(0, false), (1, false), (2, false), (3, false)];
        for _ in 0..12 {
            placed.push((0, false));
        }
        let l = Layout::new(4, &placed);
        assert_eq!(l.overlaps(), 0);
    }

    /// A random legal board of up to `max` tiles: its placements' lines
    /// and whether each is a double.
    fn random_board(sides: u8, max: usize) -> Vec<(u8, bool)> {
        let mut rng = rand::rng();
        let set = full_set(TOP);
        let center = *set.iter().find(|t| t.a == 12 && t.b == 12).unwrap();
        let mut pool: Vec<_> = set.into_iter().filter(|t| t.id != center.id).collect();
        let mut board = Board::new(center, sides);
        while board.placed.len() < max {
            let moves: Vec<(usize, u8)> = pool
                .iter()
                .enumerate()
                .flat_map(|(i, t)| board.targets(t).into_iter().map(move |l| (i, l)))
                .collect();
            if moves.is_empty() {
                break;
            }
            let (i, line) = moves[rng.random_range(0..moves.len())];
            let t = pool.remove(i);
            board.place(t, line, 0);
        }
        board
            .placed
            .iter()
            .map(|p| (p.line, p.tile.is_double()))
            .collect()
    }

    /// Every tile touches the tile it was played on: no gaps, ever.
    #[test]
    fn tiles_always_touch_their_line() {
        for game in 0..300 {
            let sides = [3u8, 4, 6, 8][game % 4];
            let placed = random_board(sides, 80);
            let l = Layout::new(sides, &placed);
            // The shape each line grows from: the middle (shape 0) for the
            // middle's sides, the double for a foot's toes, then each tile
            // in turn.
            let mut parent: Vec<usize> = vec![0; sides as usize];
            for (i, &(line, double)) in placed.iter().enumerate() {
                let me = i + 1;
                let from = l.shapes[parent[line as usize]];
                let mut near = l.shapes[me];
                near.hx += 0.06;
                near.hy += 0.06;
                assert!(
                    near.overlaps(&from),
                    "{sides} sides, tile {i} floats away from its line: {:?}",
                    l.tiles[i]
                );
                parent[line as usize] = me;
                if double {
                    parent.extend([me; 3]);
                }
            }
        }
    }

    /// Lay out random legal games and count the tiles that found no clear
    /// spot and had to go over others. Crowded boards may have a few; more
    /// means the fitting is broken.
    #[test]
    fn random_boards_rarely_box_in() {
        let (mut boxed, mut total) = (0, 0);
        for game in 0..400 {
            let sides = [3u8, 4, 6, 8][game % 4];
            let placed = random_board(sides, 70);
            let mut l = Layout::new(sides, &[]);
            for &(line, double) in &placed {
                let fit = l.fit(line, double);
                if !l.clear(&fit.obb) {
                    boxed += 1;
                }
                l.place(line, double);
            }
            total += placed.len();
        }
        assert!(boxed * 100 < total, "{boxed} of {total} tiles boxed in");
    }
}

// Jackson Coxson
// Chicken Scratch: the board rules, the wire messages shared by the server
// and the browser, and how tiles move.
//
// The board is a set of lines. Each round starts with that round's double in
// the middle and an empty line off each of its sides. A tile goes on the open
// end of a line, matching it. A double played on a line ends that line and
// starts three new ones off its far side: the chicken foot. While any line is
// empty (the middle's sides, or a fresh foot's toes), tiles may only go on
// empty lines, so the middle and every foot are filled before play moves on.
// Each number has one double, so a toe or a side of the middle can never be
// a double itself.

use serde::{Deserialize, Serialize};

use crate::games::cards::{Motion, Spot, Standing, TableView};
pub use crate::games::dominoes::Tile;

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 8;
/// Double-12 dominoes: rounds start on the double 12 and count down to 0-0.
pub const TOP: u8 = 12;
pub const ROUNDS: u8 = TOP + 1;
/// Tiles on a chicken foot.
pub const TOES: usize = 3;

/// The round's starting double: 12 in round 1, down to 0 in round 13.
pub fn round_double(round: u8) -> u8 {
    TOP.saturating_sub(round.saturating_sub(1))
}

/// Tiles in each starting hand.
pub fn hand_size(players: usize) -> usize {
    match players {
        0..=4 => 15,
        5 | 6 => 12,
        _ => 10,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Lines off the middle double that must be filled first: 3, 4, 6 or 8.
    pub sides: u8,
    /// The double blank counts 50 when it's left in your hand.
    pub blank_fifty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sides: 4,
            blank_fifty: true,
        }
    }
}

impl Options {
    pub const SIDES: [u8; 4] = [3, 4, 6, 8];

    /// Settings from the lobby, with anything odd put back to the default.
    pub fn checked(mut self) -> Self {
        if !Self::SIDES.contains(&self.sides) {
            self.sides = Options::default().sides;
        }
        self
    }

    pub fn describe(&self) -> String {
        let mut s = format!(
            "Double-12 set, 13 rounds. {} sides on the middle double.",
            self.sides
        );
        if self.blank_fifty {
            s.push_str(" A leftover 0-0 counts 50.");
        }
        s
    }
}

/// What a tile left in hand at the end of a round counts.
pub fn tile_points(tile: &Tile, options: &Options) -> i32 {
    if options.blank_fifty && tile.a == 0 && tile.b == 0 {
        50
    } else {
        tile.pips()
    }
}

/// One line of tiles growing off the middle or off a chicken foot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// The number to match at the end.
    pub open: u8,
    /// Tiles on it so far.
    pub len: u8,
    /// A double was played on it, so it ends there; its toes are new lines.
    pub closed: bool,
}

/// A tile on the board: which line it went on and which of its numbers
/// matched. Doubles start `TOES` new lines, numbered in play order after the
/// middle's sides.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub tile: Tile,
    pub line: u8,
    /// The matching number, against the line; the other faces out.
    pub near: u8,
    /// Who played it.
    pub by: u8,
}

impl Placement {
    pub fn far(&self) -> u8 {
        self.tile.other(self.near)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    /// The round's double, in the middle.
    pub center: Tile,
    /// Lines `0..sides` are the middle's sides.
    pub sides: u8,
    pub lines: Vec<Line>,
    pub placed: Vec<Placement>,
}

impl Board {
    pub fn new(center: Tile, sides: u8) -> Self {
        Self {
            center,
            sides,
            lines: (0..sides)
                .map(|_| Line {
                    open: center.a,
                    len: 0,
                    closed: false,
                })
                .collect(),
            placed: Vec::new(),
        }
    }

    /// Whether an empty line (a side of the middle, a toe) has to be filled
    /// before anything else.
    pub fn filling(&self) -> bool {
        self.lines.iter().any(|l| l.len == 0 && !l.closed)
    }

    /// The lines `tile` can go on right now.
    pub fn targets(&self, tile: &Tile) -> Vec<u8> {
        let filling = self.filling();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.closed && (!filling || l.len == 0) && tile.has(l.open))
            .map(|(i, _)| i as u8)
            .collect()
    }

    pub fn can_play(&self, tile: &Tile) -> bool {
        !self.targets(tile).is_empty()
    }

    /// Put `tile` on `line`. The caller checks it's allowed.
    pub fn place(&mut self, tile: Tile, line: u8, by: u8) -> Placement {
        let l = &mut self.lines[line as usize];
        let near = l.open;
        l.len += 1;
        if tile.is_double() {
            l.closed = true;
            for _ in 0..TOES {
                self.lines.push(Line {
                    open: near,
                    len: 0,
                    closed: false,
                });
            }
        } else {
            l.open = tile.other(near);
        }
        let p = Placement {
            tile,
            line,
            near,
            by,
        };
        self.placed.push(p);
        p
    }
}

// ── Views ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    /// Tiles in hand.
    pub tiles: u8,
    /// Points from each finished round.
    pub rounds: Vec<i32>,
    pub total: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundResult {
    pub player: u8,
    pub left: Vec<Tile>,
    pub points: i32,
    pub went_out: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Stage {
    Playing,
    RoundOver {
        results: Vec<RoundResult>,
        /// Nobody could play and the boneyard was empty.
        blocked: bool,
    },
    GameOver {
        results: Vec<RoundResult>,
        standings: Vec<Standing>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub seq: u32,
    pub player: u8,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionKind {
    /// A new round: hands dealt, and `player` put the double in the middle
    /// (after drawing `drawn` tiles looking for it, when nobody had it).
    Deal {
        drawn: u8,
    },
    /// `player` played the board's latest tile.
    Played,
    /// `player` drew a tile; `passed` when it didn't fit either.
    Drew {
        passed: bool,
    },
    RoundOver,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    /// 1-based.
    pub round: u8,
    pub options: Options,
    pub current: u8,
    pub players: Vec<PlayerView>,
    pub hand: Vec<Tile>,
    pub board: Option<Board>,
    pub boneyard: u8,
    /// The current player drew this turn (and so must play what they drew).
    pub drew: bool,
    /// Who put the middle double down this round.
    pub opener: u8,
    pub stage: Stage,
    pub log: Vec<String>,
    pub last: Option<Action>,
}

impl GameView {
    pub fn my_turn(&self) -> bool {
        matches!(self.stage, Stage::Playing) && self.current == self.you
    }

    /// Lines `tile` can go on, if it's your turn.
    pub fn targets(&self, tile: &Tile) -> Vec<u8> {
        match &self.board {
            Some(b) if self.my_turn() => b.targets(tile),
            _ => Vec::new(),
        }
    }

    pub fn can_play_any(&self) -> bool {
        self.my_turn() && self.hand.iter().any(|t| !self.targets(t).is_empty())
    }
}

pub type ServerMsg = crate::games::cards::ServerMsg<GameView>;

/// The `data-pile` name of the boneyard.
pub const BONEYARD: &str = "boneyard";

impl TableView for GameView {
    fn you(&self) -> u8 {
        self.you
    }

    fn last_move(&self) -> Option<(u32, bool)> {
        self.last
            .map(|a| (a.seq, matches!(a.kind, ActionKind::Deal { .. })))
    }

    fn placeholder() -> Self {
        GameView {
            you: 0,
            host: 0,
            round: 1,
            options: Options::default(),
            current: 0,
            players: Vec::new(),
            hand: Vec::new(),
            board: None,
            boneyard: 0,
            drew: false,
            opener: 0,
            stage: Stage::Playing,
            log: Vec::new(),
            last: None,
        }
    }

    /// Hand tiles carry `data-card`, so they slide on their own and new ones
    /// fly in from the boneyard. The board animates itself.
    fn motion(&self) -> Motion {
        let Some(a) = self.last else {
            return Motion::default();
        };
        match a.kind {
            ActionKind::Deal { .. } => Motion {
                arrive_from: Some(Spot::Pile(BONEYARD)),
                deal: true,
                ..Motion::default()
            },
            ActionKind::Drew { .. } => Motion {
                arrive_from: Some(Spot::Pile(BONEYARD)),
                ..Motion::default()
            },
            _ => Motion::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    SetNickname {
        name: String,
    },
    StartGame,
    /// Put a tile from your hand on a line.
    Play {
        tile: u8,
        line: u8,
    },
    /// Draw from the boneyard when nothing in your hand fits.
    Draw,
    /// The host starts the next round.
    NextRound,
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::dominoes::full_set;

    fn tile(a: u8, b: u8) -> Tile {
        *full_set(TOP)
            .iter()
            .find(|t| t.a == a.min(b) && t.b == a.max(b))
            .unwrap()
    }

    #[test]
    fn middle_fills_before_anything_else() {
        let mut b = Board::new(tile(12, 12), 4);
        assert!(b.filling());
        assert_eq!(b.targets(&tile(12, 5)), vec![0, 1, 2, 3]);
        b.place(tile(12, 5), 0, 0);
        // A 5 fits line 0 now, but the empty sides come first.
        assert_eq!(b.targets(&tile(5, 3)), Vec::<u8>::new());
        assert_eq!(b.targets(&tile(12, 3)), vec![1, 2, 3]);
        b.place(tile(12, 3), 1, 0);
        b.place(tile(12, 4), 2, 0);
        b.place(tile(12, 6), 3, 0);
        assert!(!b.filling());
        assert_eq!(b.targets(&tile(5, 3)), vec![0, 1]);
    }

    #[test]
    fn a_double_grows_a_foot_that_must_be_filled() {
        let mut b = Board::new(tile(12, 12), 3);
        for (i, n) in [5, 3, 4].into_iter().enumerate() {
            b.place(tile(12, n), i as u8, 0);
        }
        let p = b.place(tile(5, 5), 0, 0);
        assert_eq!(p.near, 5);
        assert!(b.lines[0].closed);
        assert_eq!(b.lines.len(), 3 + TOES);
        // Only the toes take tiles until all three are down.
        assert_eq!(b.targets(&tile(3, 1)), Vec::<u8>::new());
        assert_eq!(b.targets(&tile(5, 1)), vec![3, 4, 5]);
        b.place(tile(5, 1), 3, 0);
        b.place(tile(5, 2), 4, 0);
        assert_eq!(b.targets(&tile(5, 0)), vec![5]);
        b.place(tile(5, 0), 5, 0);
        assert!(!b.filling());
        let mut open: Vec<u8> = b.targets(&tile(1, 3));
        open.sort();
        assert_eq!(open, vec![1, 3]);
    }

    #[test]
    fn rounds_count_down() {
        assert_eq!(round_double(1), 12);
        assert_eq!(round_double(13), 0);
    }

    #[test]
    fn options_are_checked() {
        let o = Options {
            sides: 5,
            blank_fifty: false,
        }
        .checked();
        assert_eq!(o.sides, 4);
        let o: Options = serde_json::from_str("{}").unwrap();
        assert_eq!(o, Options::default());
    }
}

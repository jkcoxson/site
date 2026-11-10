// Jackson Coxson
// 9 Holes of Golf: scoring, the wire messages shared by the server and the
// browser, and how cards move.

use serde::{Deserialize, Serialize};

use crate::games::cards::{Motion, Spot, Standing, TableView};
pub use crate::games::number_cards::Card;

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 6;
/// A 3×3 grid, row by row; slot `i` is in column `i % 3`.
pub const GRID: usize = 9;
/// Cards each player turns up before the hole starts.
pub const OPENING_FLIPS: usize = 2;

/// What one card is worth: 2s are -2, Wilds are 0, the rest face value.
pub fn card_points(card: &Card) -> i32 {
    match card.rank {
        0 => 0,
        2 => -2,
        r => r as i32,
    }
}

/// The slots in column `col`, top to bottom.
pub fn column(col: usize) -> [usize; 3] {
    [col, col + 3, col + 6]
}

/// Whether a column of three cards is the same number and so scores 0.
/// Wilds don't match anything, and a column of 2s keeps its -6.
pub fn column_cleared(cards: [Card; 3]) -> bool {
    let rank = cards[0].rank;
    rank != 0 && rank != 2 && cards.iter().all(|c| c.rank == rank)
}

/// A full grid's score.
pub fn grid_score(grid: &[Card]) -> i32 {
    (0..3)
        .map(|col| {
            let cards = column(col).map(|i| grid[i]);
            if column_cleared(cards) {
                0
            } else {
                cards.iter().map(card_points).sum()
            }
        })
        .sum()
}

/// The score so far from what's face up: cleared columns count only once
/// all three are showing. `None` slots are face down.
pub fn showing_score(grid: &[Option<Card>]) -> i32 {
    (0..3)
        .map(|col| {
            let slots = column(col).map(|i| grid[i]);
            match slots {
                [Some(a), Some(b), Some(c)] if column_cleared([a, b, c]) => 0,
                _ => slots.iter().flatten().map(card_points).sum(),
            }
        })
        .sum()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    /// Face-up cards; `None` is face down.
    pub grid: Vec<Option<Card>>,
    /// Points from each finished hole.
    pub holes: Vec<i32>,
    pub total: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleResult {
    pub player: u8,
    pub grid: Vec<Card>,
    pub score: i32,
    pub went_out: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Stage {
    /// Everyone picks two cards to turn up.
    Opening,
    Playing,
    HoleOver {
        results: Vec<HoleResult>,
    },
    GameOver {
        results: Vec<HoleResult>,
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
    Deal,
    Flipped {
        slot: u8,
    },
    DrewDeck,
    TookDiscard,
    /// The held card went into `slot`; the card that was there was
    /// discarded.
    Swapped {
        slot: u8,
        out: Card,
    },
    /// The card drawn from the deck went straight to the discard pile.
    Tossed,
    /// The hole ended and every card was turned up.
    Revealed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    pub hole: u8,
    pub dealer: u8,
    pub current: u8,
    pub players: Vec<PlayerView>,
    pub discard_top: Option<Card>,
    pub deck_size: u8,
    /// The card the current player is holding, and whether it came from
    /// the discard pile (and so must be used).
    pub held: Option<(Card, bool)>,
    /// The current player discarded a card drawn from the deck and now has
    /// to turn up one of their face-down cards.
    pub must_flip: bool,
    /// Who turned up their last card first; everyone else gets one more turn.
    pub out: Option<u8>,
    pub stage: Stage,
    pub log: Vec<String>,
    pub last: Option<Action>,
}

pub type ServerMsg = crate::games::cards::ServerMsg<GameView>;

/// The `data-pile` name of a player's grid slot, so face-down cards can be
/// sent flying from it.
pub fn slot_pile(player: u8, slot: u8) -> &'static str {
    const NAMES: [[&str; GRID]; MAX_PLAYERS] = [
        [
            "g0-0", "g0-1", "g0-2", "g0-3", "g0-4", "g0-5", "g0-6", "g0-7", "g0-8",
        ],
        [
            "g1-0", "g1-1", "g1-2", "g1-3", "g1-4", "g1-5", "g1-6", "g1-7", "g1-8",
        ],
        [
            "g2-0", "g2-1", "g2-2", "g2-3", "g2-4", "g2-5", "g2-6", "g2-7", "g2-8",
        ],
        [
            "g3-0", "g3-1", "g3-2", "g3-3", "g3-4", "g3-5", "g3-6", "g3-7", "g3-8",
        ],
        [
            "g4-0", "g4-1", "g4-2", "g4-3", "g4-4", "g4-5", "g4-6", "g4-7", "g4-8",
        ],
        [
            "g5-0", "g5-1", "g5-2", "g5-3", "g5-4", "g5-5", "g5-6", "g5-7", "g5-8",
        ],
    ];
    NAMES[player as usize % MAX_PLAYERS][slot as usize % GRID]
}

impl TableView for GameView {
    fn you(&self) -> u8 {
        self.you
    }

    fn last_move(&self) -> Option<(u32, bool)> {
        self.last.map(|a| (a.seq, a.kind == ActionKind::Deal))
    }

    fn placeholder() -> Self {
        GameView {
            you: 0,
            host: 0,
            hole: 1,
            dealer: 0,
            current: 0,
            players: Vec::new(),
            discard_top: None,
            deck_size: 0,
            held: None,
            must_flip: false,
            out: None,
            stage: Stage::Opening,
            log: Vec::new(),
            last: None,
        }
    }

    /// Face-up cards carry `data-card` and slide between the grid, the held
    /// spot and the discard pile on their own. A face-down card swapped out
    /// appears on the discard pile, so it flies there from its slot.
    fn motion(&self) -> Motion {
        let Some(a) = self.last else {
            return Motion::default();
        };
        match a.kind {
            ActionKind::Deal => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                deal: true,
                ..Motion::default()
            },
            ActionKind::DrewDeck => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                ..Motion::default()
            },
            ActionKind::Swapped { slot, .. } => Motion {
                arrive_from: Some(Spot::Pile(slot_pile(a.player, slot))),
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
    /// Turn a face-down card up: one of your opening two, or the one you
    /// have to turn up after discarding a card drawn from the deck.
    Flip {
        slot: u8,
    },
    /// Take the top of the draw pile, or the top of the discard pile.
    Draw {
        from_discard: bool,
    },
    /// Put the held card in `slot`, discarding the card that was there.
    Swap {
        slot: u8,
    },
    /// Discard the card drawn from the deck instead of using it; you then
    /// turn up one of your face-down cards.
    Toss,
    /// The host plays another hole, or ends the game.
    NextHole,
    EndGame,
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(ranks: [u8; 9]) -> Vec<Card> {
        ranks
            .iter()
            .enumerate()
            .map(|(i, &rank)| Card { id: i as u8, rank })
            .collect()
    }

    #[test]
    fn points() {
        assert_eq!(card_points(&Card { id: 0, rank: 2 }), -2);
        assert_eq!(card_points(&Card { id: 0, rank: 0 }), 0);
        assert_eq!(card_points(&Card { id: 0, rank: 1 }), 1);
        assert_eq!(card_points(&Card { id: 0, rank: 12 }), 12);
    }

    #[test]
    fn matching_columns_score_zero() {
        // Columns: (5,5,5) cleared, (1,4,7), (9,0,2).
        let g = grid([5, 1, 9, 5, 4, 0, 5, 7, 2]);
        assert_eq!(grid_score(&g), 0 + 12 + 7);
    }

    #[test]
    fn twos_keep_their_minus_six() {
        let g = grid([2, 1, 1, 2, 1, 1, 2, 1, 1]);
        // Two columns of 1s clear; the 2s are -6.
        assert_eq!(grid_score(&g), -6);
    }

    #[test]
    fn wilds_dont_match() {
        let g = grid([0, 3, 3, 0, 3, 3, 0, 3, 3]);
        assert_eq!(grid_score(&g), 0);
        let g = grid([0, 3, 3, 7, 3, 3, 7, 3, 3]);
        assert_eq!(grid_score(&g), 14);
    }

    #[test]
    fn showing_score_waits_for_whole_columns() {
        let c = |rank| Some(Card { id: 0, rank });
        let g = vec![c(8), None, None, c(8), None, None, None, None, c(2)];
        assert_eq!(showing_score(&g), 16 - 2);
        let g = vec![c(8), None, None, c(8), None, None, c(8), None, c(2)];
        assert_eq!(showing_score(&g), -2);
    }
}

// Jackson Coxson
// Stack Attack: options, the wire messages shared by the server and the
// browser, and how cards move.

use serde::{Deserialize, Serialize};

use crate::games::cards::{Motion, Spot, Standing, TableView};
pub use crate::games::number_cards::Card;

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 6;
pub const HAND_SIZE: usize = 5;
pub const BUILD_PILES: usize = 4;
pub const DISCARD_PILES: usize = 4;
/// A building pile is finished when it reaches 12.
pub const TOP_RANK: u8 = 12;
/// In a scored match, the first to this many points wins.
pub const MATCH_POINTS: i32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Options {
    /// The rulebook's short game: stock piles of 10.
    pub short: bool,
    /// Partners sit across from each other and share stock and discard
    /// piles. Needs four or six players.
    pub partners: bool,
    /// Keep score over several games; first to 500 wins.
    pub scored: bool,
}

impl Options {
    /// Cards dealt to each stock pile.
    pub fn stock_size(&self, players: usize) -> usize {
        if self.short {
            10
        } else if players <= 4 {
            30
        } else {
            20
        }
    }

    pub fn describe(&self) -> String {
        let mut parts = vec![if self.short {
            "Short game: stock piles of 10."
        } else {
            "Stock piles of 30 (20 with five or more players)."
        }];
        if self.partners {
            parts.push("Partners sit across from each other.");
        }
        if self.scored {
            parts.push("Scored match to 500.");
        }
        parts.join(" ")
    }
}

/// Who sits across from `player`, when playing in partnerships.
pub fn partner_of(player: usize, players: usize) -> usize {
    (player + players / 2) % players
}

/// Whether a card may go on a building pile that already holds `height`
/// cards (0 for an empty spot).
pub fn fits(card: &Card, height: usize) -> bool {
    height < TOP_RANK as usize && (card.is_wild() || card.rank as usize == height + 1)
}

/// One building pile in the middle of the table.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BuildTop {
    pub card: Card,
    /// The number the pile has reached; a wild on top counts as that number.
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    pub stock_left: u8,
    pub stock_top: Option<Card>,
    /// Four discard piles, bottom card first.
    pub discards: Vec<Vec<Card>>,
    pub hand_size: u8,
    /// Match points (shared by partners).
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Stage {
    Playing,
    /// Someone emptied their stock. `points` is what the winners scored in a
    /// match; `match_over` is whether the match (or the single game) is done.
    GameOver {
        winners: Vec<u8>,
        points: Option<i32>,
        match_over: bool,
        standings: Vec<Standing>,
    },
}

/// Where a played card came from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Source {
    Hand,
    Stock(u8),
    Discard(u8),
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
    /// `player` drew cards to fill their hand.
    Drew,
    /// A card went onto building pile `pile`. `finished` when that made 12
    /// and the pile was cleared away.
    Played {
        card: Card,
        from: Source,
        pile: u8,
        finished: bool,
    },
    Discarded {
        card: Card,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    /// Games played in this match, counting the current one.
    pub game: u8,
    pub current: u8,
    pub options: Options,
    pub partner: Option<u8>,
    pub hand: Vec<Card>,
    pub draw_count: u8,
    pub building: Vec<Option<BuildTop>>,
    pub players: Vec<PlayerView>,
    pub stage: Stage,
    pub log: Vec<String>,
    pub last: Option<Action>,
}

pub type ServerMsg = crate::games::cards::ServerMsg<GameView>;

impl GameView {
    /// Players whose stock and discard piles `you` may play from.
    pub fn usable_seats(&self) -> Vec<u8> {
        let mut seats = vec![self.you];
        seats.extend(self.partner);
        seats
    }
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
            game: 1,
            current: 0,
            options: Options::default(),
            partner: None,
            hand: Vec::new(),
            draw_count: 0,
            building: vec![None; BUILD_PILES],
            players: Vec::new(),
            stage: Stage::Playing,
            log: Vec::new(),
            last: None,
        }
    }

    /// Every face-up card carries `data-card`, so cards moving between a
    /// hand, stock, discard and building pile slide on their own.
    fn motion(&self) -> Motion {
        let Some(a) = self.last else {
            return Motion::default();
        };
        let mine = a.player == self.you;
        match a.kind {
            ActionKind::Deal => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                deal: true,
                ..Motion::default()
            },
            ActionKind::Drew if mine => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                deal: true,
                ..Motion::default()
            },
            ActionKind::Drew => Motion {
                leave: vec![(Spot::Pile("deck"), Some(a.player))],
                ..Motion::default()
            },
            ActionKind::Played { card, finished, .. } => Motion {
                arrive_from: (!mine).then_some(Spot::Seat(a.player)),
                leave: if finished {
                    vec![(Spot::Card(card.id), None)]
                } else {
                    Vec::new()
                },
                ..Motion::default()
            },
            ActionKind::Discarded { .. } if !mine => Motion {
                arrive_from: Some(Spot::Seat(a.player)),
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
    /// Put `card` (from your hand, or the top of a stock or discard pile you
    /// may use) on building pile `pile`.
    Play {
        card: u8,
        pile: u8,
    },
    /// End your turn by putting `card` from your hand on your discard pile.
    Discard {
        card: u8,
        pile: u8,
    },
    /// End your turn with nothing to discard (an empty hand and no cards
    /// left to draw).
    Pass,
    Arrange {
        order: Vec<u8>,
    },
    /// The host deals the next game of a match.
    NextGame,
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(rank: u8) -> Card {
        Card { id: 0, rank }
    }

    #[test]
    fn building_runs_one_to_twelve() {
        assert!(fits(&card(1), 0));
        assert!(!fits(&card(2), 0));
        assert!(fits(&card(0), 0));
        assert!(fits(&card(5), 4));
        assert!(!fits(&card(6), 4));
        assert!(fits(&card(12), 11));
        assert!(!fits(&card(0), 12));
    }

    #[test]
    fn stock_sizes() {
        let o = Options::default();
        assert_eq!(o.stock_size(2), 30);
        assert_eq!(o.stock_size(4), 30);
        assert_eq!(o.stock_size(5), 20);
        assert_eq!(o.stock_size(6), 20);
        let short = Options { short: true, ..o };
        assert_eq!(short.stock_size(3), 10);
    }

    #[test]
    fn partners_sit_across() {
        assert_eq!(partner_of(0, 4), 2);
        assert_eq!(partner_of(3, 4), 1);
        assert_eq!(partner_of(1, 6), 4);
        assert_eq!(partner_of(5, 6), 2);
    }
}

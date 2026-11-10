// Jackson Coxson
// Ten Stages: cards, the ten stages, laid-down groups and hitting, the
// lay-down finder, and the wire messages shared by the server and the browser.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::games::cards::melds::{self, Natural};
use crate::games::cards::{HandCard, Motion, Spot, Standing, TableView};

pub const STAGES: u8 = 10;
pub const HAND_SIZE: usize = 10;
pub const MIN_PLAYERS: usize = 2;
/// 108 cards deal six hands of ten with plenty left to draw.
pub const MAX_PLAYERS: usize = 6;
/// The highest number on a card; runs can't go past it.
pub const TOP_RANK: u8 = 12;

// ── Cards ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Color {
    Red,
    Blue,
    Green,
    Yellow,
}

impl Color {
    pub const ALL: [Color; 4] = [Color::Red, Color::Blue, Color::Green, Color::Yellow];

    pub fn name(self) -> &'static str {
        match self {
            Color::Red => "red",
            Color::Blue => "blue",
            Color::Green => "green",
            Color::Yellow => "yellow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Face {
    Number { rank: u8, color: Color },
    Wild,
    Skip,
}

/// One card. The `id` tells the two copies of each number apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub face: Face,
}

impl Card {
    pub fn is_wild(&self) -> bool {
        self.face == Face::Wild
    }

    pub fn is_skip(&self) -> bool {
        self.face == Face::Skip
    }

    pub fn rank(&self) -> Option<u8> {
        match self.face {
            Face::Number { rank, .. } => Some(rank),
            _ => None,
        }
    }

    pub fn color(&self) -> Option<Color> {
        match self.face {
            Face::Number { color, .. } => Some(color),
            _ => None,
        }
    }

    /// How melds see the card. Skips never go in a meld, so check first.
    fn natural(&self) -> Natural {
        match self.face {
            Face::Number { rank, color } => Some((rank, color as u8)),
            _ => None,
        }
    }

    /// Points against you if it's still in your hand when the round ends.
    pub fn points(&self) -> u32 {
        match self.face {
            Face::Number { rank, .. } if rank < 10 => 5,
            Face::Number { .. } => 10,
            Face::Skip => 15,
            Face::Wild => 25,
        }
    }

    /// "a red 7", "a Wild", "a Skip".
    pub fn label(&self) -> String {
        match self.face {
            Face::Number { rank, color } => format!("a {} {rank}", color.name()),
            Face::Wild => "a Wild".into(),
            Face::Skip => "a Skip".into(),
        }
    }

    /// Numbers by color then number, then wilds, then skips.
    pub fn by_color(&self) -> (u8, Option<Color>, Option<u8>, u8) {
        (self.order(), self.color(), self.rank(), self.id)
    }

    /// Numbers by number then color, then wilds, then skips.
    pub fn by_number(&self) -> (u8, Option<u8>, Option<Color>, u8) {
        (self.order(), self.rank(), self.color(), self.id)
    }

    fn order(&self) -> u8 {
        match self.face {
            Face::Number { .. } => 0,
            Face::Wild => 1,
            Face::Skip => 2,
        }
    }
}

impl HandCard for Card {
    fn id(&self) -> u8 {
        self.id
    }
}

/// The deck in order: two of each number 1 to 12 in each color, eight
/// wilds and four skips.
pub fn full_deck() -> Vec<Card> {
    let mut faces = Vec::with_capacity(108);
    for color in Color::ALL {
        for rank in 1..=TOP_RANK {
            faces.push(Face::Number { rank, color });
            faces.push(Face::Number { rank, color });
        }
    }
    faces.extend([Face::Wild; 8]);
    faces.extend([Face::Skip; 4]);
    faces
        .into_iter()
        .enumerate()
        .map(|(id, face)| Card { id: id as u8, face })
        .collect()
}

// ── Stages ────────────────────────────────────────────────────────────────────

/// One part of a stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Need {
    /// Cards of one number.
    Set(u8),
    /// Cards numbered in a row, any colors.
    Run(u8),
    /// Cards of one color, any numbers.
    Color(u8),
}

impl Need {
    pub fn size(self) -> usize {
        match self {
            Need::Set(n) | Need::Run(n) | Need::Color(n) => n as usize,
        }
    }
}

const LIST: [&[Need]; STAGES as usize] = [
    &[Need::Set(3), Need::Set(3)],
    &[Need::Set(3), Need::Run(4)],
    &[Need::Set(4), Need::Run(4)],
    &[Need::Run(7)],
    &[Need::Run(8)],
    &[Need::Run(9)],
    &[Need::Set(4), Need::Set(4)],
    &[Need::Color(7)],
    &[Need::Set(5), Need::Set(2)],
    &[Need::Set(5), Need::Set(3)],
];

/// What stage `stage` (1 to 10) asks for.
pub fn needs(stage: u8) -> &'static [Need] {
    LIST[(stage.clamp(1, STAGES) - 1) as usize]
}

/// "2 sets of 3", "1 set of 3 + 1 run of 4", "7 cards of one color".
pub fn describe(stage: u8) -> String {
    let list = needs(stage);
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < list.len() {
        let mut count = 1;
        while i + count < list.len() && list[i + count] == list[i] {
            count += 1;
        }
        let plural = if count > 1 { "s" } else { "" };
        parts.push(match list[i] {
            Need::Set(n) => format!("{count} set{plural} of {n}"),
            Need::Run(n) => format!("{count} run{plural} of {n}"),
            Need::Color(n) => format!("{n} cards of one color"),
        });
        i += count;
    }
    parts.join(" + ")
}

// ── Groups on the table ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Set {
        rank: u8,
    },
    /// `low` is the number the first card stands for.
    Run {
        low: u8,
    },
    Color {
        color: Color,
    },
}

/// A laid-down set, run or color group. A run's cards are in order, so the
/// card at `i` stands for `low + i`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub kind: Kind,
    pub cards: Vec<Card>,
}

impl Group {
    /// The group `cards` make for `need`, if they make one. Every group needs
    /// at least one numbered card, which also settles what the wilds are.
    pub fn new(need: Need, cards: &[Card]) -> Option<Group> {
        if cards.len() != need.size() || cards.iter().any(Card::is_skip) {
            return None;
        }
        if cards.iter().all(Card::is_wild) {
            return None;
        }
        let naturals: Vec<Natural> = cards.iter().map(Card::natural).collect();
        let mut cards = cards.to_vec();
        let kind = match need {
            Need::Set(_) => {
                let rank = melds::set_rank(&naturals)??;
                cards.sort_by_key(Card::by_color);
                Kind::Set { rank }
            }
            Need::Color(_) => {
                let color = cards.iter().find_map(Card::color)?;
                melds::suit_of(&naturals)??;
                cards.sort_by_key(Card::by_number);
                Kind::Color { color }
            }
            Need::Run(_) => {
                let low = melds::run_start(&naturals, 1..=TOP_RANK, false)?;
                let mut slots: Vec<Option<Card>> = vec![None; cards.len()];
                let mut wilds = Vec::new();
                for card in cards.iter() {
                    match card.rank() {
                        Some(r) => slots[(r - low) as usize] = Some(*card),
                        None => wilds.push(*card),
                    }
                }
                cards = slots
                    .into_iter()
                    .map(|s| s.or_else(|| wilds.pop()))
                    .collect::<Option<Vec<Card>>>()?;
                Kind::Run { low }
            }
        };
        Some(Group { kind, cards })
    }

    /// Whether `card` can be hit onto the low end (the start of a run) and
    /// the high end (the end of a run, or anywhere in a set or color group).
    pub fn fits(&self, card: &Card) -> [bool; 2] {
        if card.is_skip() {
            return [false, false];
        }
        match self.kind {
            Kind::Set { rank } => [false, card.is_wild() || card.rank() == Some(rank)],
            Kind::Color { color } => [false, card.is_wild() || card.color() == Some(color)],
            Kind::Run { low } => {
                let high = low + self.cards.len() as u8 - 1;
                match card.rank() {
                    None => [low > 1, high < TOP_RANK],
                    Some(r) => [low > 1 && r == low - 1, high < TOP_RANK && r == high + 1],
                }
            }
        }
    }

    pub fn can_take(&self, card: &Card) -> bool {
        self.fits(card).contains(&true)
    }

    /// Add `card`, at the low end of a run if `low` and it fits there,
    /// otherwise wherever it fits. Returns whether it went on.
    pub fn hit(&mut self, card: Card, low: bool) -> bool {
        let [at_low, at_high] = self.fits(&card);
        if at_low && (low || !at_high) {
            self.cards.insert(0, card);
            if let Kind::Run { low } = &mut self.kind {
                *low -= 1;
            }
            true
        } else if at_high {
            match self.kind {
                Kind::Run { .. } => self.cards.push(card),
                // Numbers before the wilds.
                _ => {
                    let at = if card.is_wild() {
                        self.cards.len()
                    } else {
                        self.cards
                            .iter()
                            .position(|c| c.is_wild())
                            .unwrap_or(self.cards.len())
                    };
                    self.cards.insert(at, card);
                }
            }
            true
        } else {
            false
        }
    }

    /// "Set of 7s", "Run 3–9", "All red".
    pub fn label(&self) -> String {
        match self.kind {
            Kind::Set { rank } => format!("Set of {rank}s"),
            Kind::Run { low } => format!("Run {low}–{}", low + self.cards.len() as u8 - 1),
            Kind::Color { color } => format!("All {}", color.name()),
        }
    }
}

// ── Finding plays ─────────────────────────────────────────────────────────────

/// A hit: `card` onto `groups[group]`, at the low end of a run if `low`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub card: Card,
    pub group: usize,
    pub low: bool,
}

/// Every card of `hand` that can go onto `groups`, as hits in an order that
/// works, applying them to `groups` as it goes. Numbers go first; then each
/// wild goes where it lets the most leftover numbers follow.
pub fn plan_hits(groups: &mut [Group], hand: &[Card]) -> Vec<Hit> {
    let mut left: Vec<Card> = hand.iter().copied().filter(|c| !c.is_skip()).collect();
    let mut hits = Vec::new();
    loop {
        let natural = left.iter().enumerate().find_map(|(i, c)| {
            if c.is_wild() {
                return None;
            }
            groups.iter().position(|g| g.can_take(c)).map(|g| (i, g))
        });
        if let Some((i, g)) = natural {
            let card = left.remove(i);
            let low = groups[g].fits(&card)[0];
            groups[g].hit(card, low);
            hits.push(Hit {
                card,
                group: g,
                low,
            });
            continue;
        }
        let Some(w) = left.iter().position(Card::is_wild) else {
            break;
        };
        let wild = left[w];
        let mut best: Option<(usize, usize, bool)> = None;
        for (g, group) in groups.iter().enumerate() {
            for (end, open) in group.fits(&wild).into_iter().enumerate() {
                if !open {
                    continue;
                }
                let mut trial = group.clone();
                trial.hit(wild, end == 0);
                let follow = left
                    .iter()
                    .filter(|c| !c.is_wild() && trial.can_take(c))
                    .count();
                if best.is_none_or(|(f, _, _)| follow > f) {
                    best = Some((follow, g, end == 0));
                }
            }
        }
        let Some((_, g, low)) = best else {
            break;
        };
        left.remove(w);
        groups[g].hit(wild, low);
        hits.push(Hit {
            card: wild,
            group: g,
            low,
        });
    }
    hits
}

/// One way to lay a stage down: the groups, and how the hand would look
/// after it and every hit that follows.
#[derive(Debug, Clone, PartialEq)]
pub struct LayDown {
    pub groups: Vec<Group>,
    /// Cards still in hand once every possible hit is made.
    pub stuck: usize,
    pub stuck_points: u32,
    pub wilds: usize,
}

/// The most ways `lay_downs` offers.
pub const MAX_OPTIONS: usize = 12;

/// The different ways `hand` can make `stage`, best first: the ones that
/// leave the fewest cards (then points) in hand once you've also hit
/// everything you can onto `table` and your new groups, then the ones using
/// the fewest wilds. Ways that differ only in which copy of a card they use
/// count once.
pub fn lay_downs(stage: u8, hand: &[Card], table: &[Group]) -> Vec<LayDown> {
    let list = needs(stage);
    let usable: Vec<usize> = (0..hand.len()).filter(|&i| !hand[i].is_skip()).collect();
    let cards: Vec<Card> = usable.iter().map(|&i| hand[i]).collect();
    if cards.len() > 16 {
        return Vec::new();
    }
    let candidates: Vec<Vec<(u32, Group)>> = list
        .iter()
        .map(|&need| {
            melds::masks_of_size(cards.len(), need.size())
                .filter_map(|m| Group::new(need, &melds::pick(&cards, m)).map(|g| (m, g)))
                .collect()
        })
        .collect();

    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    let mut chosen: Vec<(u32, Group)> = Vec::new();
    search(list, &candidates, 0, 0, &mut chosen, &mut |chosen| {
        let mut key: Vec<Vec<Face>> = chosen
            .iter()
            .map(|(_, g)| {
                let mut faces: Vec<Face> = g.cards.iter().map(|c| c.face).collect();
                faces.sort();
                faces
            })
            .collect();
        key.sort();
        if !seen.insert(key) {
            return;
        }
        let used = chosen.iter().fold(0, |acc, (m, _)| acc | m);
        let rest: Vec<Card> = (0..cards.len())
            .filter(|i| used & (1 << i) == 0)
            .map(|i| cards[i])
            .chain(hand.iter().copied().filter(Card::is_skip))
            .collect();
        let groups: Vec<Group> = chosen.iter().map(|(_, g)| g.clone()).collect();
        let mut all: Vec<Group> = table
            .iter()
            .cloned()
            .chain(groups.iter().cloned())
            .collect();
        let hits = plan_hits(&mut all, &rest);
        let stuck: Vec<&Card> = rest
            .iter()
            .filter(|c| !hits.iter().any(|h| h.card.id == c.id))
            .collect();
        found.push(LayDown {
            wilds: groups
                .iter()
                .flat_map(|g| &g.cards)
                .filter(|c| c.is_wild())
                .count(),
            stuck: stuck.len(),
            stuck_points: stuck.iter().map(|c| c.points()).sum(),
            groups,
        });
    });
    found.sort_by_key(|l| (l.stuck, l.stuck_points, l.wilds));
    found.truncate(MAX_OPTIONS);
    found
}

/// Try each candidate for need `at` that doesn't share cards with `used`.
/// Two identical needs in a row take their groups in mask order, so the
/// same pair isn't found twice.
fn search(
    list: &[Need],
    candidates: &[Vec<(u32, Group)>],
    at: usize,
    used: u32,
    chosen: &mut Vec<(u32, Group)>,
    found: &mut dyn FnMut(&[(u32, Group)]),
) {
    if at == list.len() {
        found(chosen);
        return;
    }
    let after = (at > 0 && list[at] == list[at - 1]).then(|| chosen[at - 1].0);
    for (mask, group) in &candidates[at] {
        if mask & used != 0 || after.is_some_and(|a| *mask <= a) {
            continue;
        }
        chosen.push((*mask, group.clone()));
        search(list, candidates, at + 1, used | mask, chosen, found);
        chosen.pop();
    }
}

// ── Options ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Stages to finish (1 to `goal`). Ten is the full game.
    pub goal: u8,
    /// Everyone moves to the next stage each round whether they finished it
    /// or not, for `goal` rounds; lowest score wins.
    pub fixed: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            goal: STAGES,
            fixed: false,
        }
    }
}

impl Options {
    pub fn describe(&self) -> String {
        if self.fixed {
            format!(
                "{} rounds, one stage each; everyone moves on every round. Lowest score wins.",
                self.goal
            )
        } else if self.goal == STAGES {
            "First to finish all ten stages wins.".into()
        } else {
            format!("First to finish stages 1 to {} wins.", self.goal)
        }
    }
}

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    pub total: u32,
    /// Points scored in each finished round.
    pub rounds: Vec<u32>,
    /// Stages finished so far.
    pub done: u8,
    /// The stage they're working on this round.
    pub stage: u8,
    pub cards: u8,
    /// What they've laid down this round; empty until they make their stage.
    pub laid: Vec<Group>,
    /// They lose their next turn.
    pub skipped: bool,
    /// They can't be skipped until they've had a normal turn.
    pub skip_proof: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundResult {
    pub player: u8,
    pub stage: u8,
    /// They made their stage and move on.
    pub advanced: bool,
    pub went_out: bool,
    pub hand: Vec<Card>,
    pub points: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Status {
    /// `drawn` is whether the current player has drawn yet this turn.
    Playing {
        drawn: bool,
    },
    RoundOver {
        results: Vec<RoundResult>,
    },
    GameOver {
        results: Vec<RoundResult>,
        standings: Vec<Standing>,
    },
}

/// The latest move at the table, so clients can animate it. `seq` counts up
/// with every move.
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
    DrewDeck,
    DrewDiscard {
        card: Card,
    },
    Discarded {
        card: Card,
    },
    /// Cards went from the player's hand onto the table.
    Played,
}

/// Everything one player may see, sent after every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    pub round: u8,
    pub dealer: u8,
    pub current: u8,
    pub options: Options,
    pub hand: Vec<Card>,
    pub discard_top: Option<Card>,
    pub deck_size: u8,
    pub players: Vec<PlayerView>,
    pub status: Status,
    /// Recent happenings, newest last.
    pub log: Vec<String>,
    pub last: Option<Action>,
}

impl GameView {
    pub fn me(&self) -> Option<&PlayerView> {
        self.players.iter().find(|p| p.id == self.you)
    }

    /// Every group on the table, in player order, with its owner and index.
    pub fn table(&self) -> Vec<(u8, u8, Group)> {
        self.players
            .iter()
            .flat_map(|p| {
                p.laid
                    .iter()
                    .enumerate()
                    .map(move |(i, g)| (p.id, i as u8, g.clone()))
            })
            .collect()
    }
}

pub type ServerMsg = crate::games::cards::ServerMsg<GameView>;

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
            round: 1,
            dealer: 0,
            current: 0,
            options: Options::default(),
            hand: Vec::new(),
            discard_top: None,
            deck_size: 0,
            players: Vec::new(),
            status: Status::Playing { drawn: false },
            log: Vec::new(),
            last: None,
        }
    }

    /// Hand, table and discard cards all carry `data-card`, so your own
    /// cards slide where they go; other players' cards fly from their seat.
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
            ActionKind::DrewDeck if mine => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                ..Motion::default()
            },
            ActionKind::DrewDeck => Motion {
                leave: vec![(Spot::Pile("deck"), Some(a.player))],
                ..Motion::default()
            },
            ActionKind::DrewDiscard { card } if !mine => Motion {
                leave: vec![(Spot::Card(card.id), Some(a.player))],
                ..Motion::default()
            },
            ActionKind::Discarded { .. } | ActionKind::Played if !mine => Motion {
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
    /// Take the top of the draw pile, or the top of the discard pile.
    Draw {
        from_discard: bool,
    },
    /// Lay down this round's stage: card ids for each of its parts, in order.
    LayDown {
        groups: Vec<Vec<u8>>,
    },
    /// Put `card` on `owner`'s group `group`, at the low end of a run if `low`.
    Hit {
        card: u8,
        owner: u8,
        group: u8,
        low: bool,
    },
    /// Make every hit the hand allows.
    HitAll,
    /// End the turn. A Skip names who loses a turn.
    Discard {
        card: u8,
        skip: Option<u8>,
    },
    /// Reorder the hand; `order` lists every card id in the hand.
    Arrange {
        order: Vec<u8>,
    },
    NextRound,
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;
    use Color::*;

    fn hand(faces: &[Face]) -> Vec<Card> {
        faces
            .iter()
            .enumerate()
            .map(|(i, &face)| Card { id: i as u8, face })
            .collect()
    }

    fn n(rank: u8, color: Color) -> Face {
        Face::Number { rank, color }
    }

    const W: Face = Face::Wild;
    const S: Face = Face::Skip;

    #[test]
    fn deck_has_108_cards() {
        let deck = full_deck();
        assert_eq!(deck.len(), 108);
        assert_eq!(deck.iter().filter(|c| c.is_wild()).count(), 8);
        assert_eq!(deck.iter().filter(|c| c.is_skip()).count(), 4);
        assert!(deck.iter().enumerate().all(|(i, c)| c.id as usize == i));
    }

    #[test]
    fn stage_names() {
        assert_eq!(describe(1), "2 sets of 3");
        assert_eq!(describe(2), "1 set of 3 + 1 run of 4");
        assert_eq!(describe(6), "1 run of 9");
        assert_eq!(describe(8), "7 cards of one color");
        assert_eq!(describe(9), "1 set of 5 + 1 set of 2");
    }

    #[test]
    fn groups_need_a_number() {
        let h = hand(&[W, W, W]);
        assert!(Group::new(Need::Set(3), &h).is_none());
        let h = hand(&[W, n(4, Red), W]);
        assert_eq!(
            Group::new(Need::Set(3), &h).unwrap().kind,
            Kind::Set { rank: 4 }
        );
        let h = hand(&[S, n(4, Red), n(4, Blue)]);
        assert!(Group::new(Need::Set(3), &h).is_none());
    }

    #[test]
    fn runs_fill_gaps_and_stop_at_twelve() {
        let h = hand(&[n(3, Red), n(6, Blue), W, n(4, Green)]);
        let g = Group::new(Need::Run(4), &h).unwrap();
        assert_eq!(g.kind, Kind::Run { low: 3 });
        assert_eq!(g.cards[2].face, W);
        let h = hand(&[n(11, Red), n(12, Blue), W, W]);
        let g = Group::new(Need::Run(4), &h).unwrap();
        assert_eq!(g.kind, Kind::Run { low: 9 });
        assert_eq!(g.fits(&Card { id: 9, face: W }), [true, false]);
        assert_eq!(g.label(), "Run 9–12");
    }

    #[test]
    fn hitting() {
        let h = hand(&[n(3, Red), n(4, Blue), n(5, Red), n(6, Red)]);
        let mut run = Group::new(Need::Run(4), &h).unwrap();
        assert!(run.hit(
            Card {
                id: 10,
                face: n(2, Yellow)
            },
            false
        ));
        assert_eq!(run.kind, Kind::Run { low: 2 });
        assert!(!run.hit(
            Card {
                id: 11,
                face: n(9, Yellow)
            },
            false
        ));
        assert!(run.hit(Card { id: 12, face: W }, true));
        assert_eq!(run.kind, Kind::Run { low: 1 });
        assert!(!run.hit(Card { id: 13, face: S }, false));

        let h = hand(&[
            n(5, Red),
            n(9, Red),
            W,
            n(1, Red),
            n(2, Red),
            n(3, Red),
            n(4, Red),
        ]);
        let mut red = Group::new(Need::Color(7), &h).unwrap();
        assert!(red.hit(
            Card {
                id: 20,
                face: n(12, Red)
            },
            false
        ));
        assert!(!red.hit(
            Card {
                id: 21,
                face: n(12, Blue)
            },
            false
        ));
        assert_eq!(red.cards.last().unwrap().face, W);
    }

    #[test]
    fn hits_chain_through_wilds() {
        // Run 3–6; holding 8 and a wild: the wild goes on as 7 so the 8 fits.
        let h = hand(&[n(3, Red), n(4, Blue), n(5, Red), n(6, Red)]);
        let mut groups = vec![Group::new(Need::Run(4), &h).unwrap()];
        let held = hand(&[n(8, Green), W, n(2, Blue)]);
        let hits = plan_hits(&mut groups, &held);
        assert_eq!(hits.len(), 3);
        assert_eq!(groups[0].label(), "Run 2–8");
    }

    #[test]
    fn finds_stage_one() {
        let h = hand(&[
            n(7, Red),
            n(7, Blue),
            n(7, Green),
            n(10, Red),
            n(10, Yellow),
            W,
            n(2, Red),
            n(9, Blue),
            S,
            n(7, Yellow),
            n(4, Red),
        ]);
        let options = lay_downs(1, &h, &[]);
        assert!(!options.is_empty());
        let best = &options[0];
        assert_eq!(best.groups.len(), 2);
        // Sevens without the wild, tens with it; the spare 7 hits afterwards.
        assert_eq!(best.wilds, 1);
        // 2, 9, 4 and the skip are stuck.
        assert_eq!(best.stuck, 4);
        let none = lay_downs(4, &h, &[]);
        assert!(none.is_empty());
    }

    #[test]
    fn finds_runs_and_colors() {
        let h = hand(&[
            n(1, Red),
            n(2, Blue),
            n(3, Red),
            n(5, Green),
            n(6, Red),
            n(7, Red),
            W,
            n(9, Red),
            n(9, Blue),
            n(12, Red),
        ]);
        let runs = lay_downs(4, &h, &[]);
        assert!(!runs.is_empty());
        assert_eq!(runs[0].groups[0].cards.len(), 7);
        let reds = lay_downs(8, &h, &[]);
        assert!(!reds.is_empty());
        assert!(lay_downs(6, &h, &[]).is_empty());
    }

    #[test]
    fn worst_case_hand_is_fast() {
        // Eleven cards that make sets every which way.
        let h = hand(&[
            n(5, Red),
            n(5, Blue),
            n(5, Green),
            n(5, Yellow),
            n(5, Red),
            n(5, Blue),
            W,
            W,
            W,
            n(6, Red),
            n(6, Blue),
        ]);
        let start = std::time::Instant::now();
        for stage in 1..=STAGES {
            let _ = lay_downs(stage, &h, &[]);
        }
        assert!(start.elapsed().as_millis() < 500);
    }
}

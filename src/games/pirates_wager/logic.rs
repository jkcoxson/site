// Jackson Coxson
// Pirate's Wager: cards, trick resolution, scoring and the wire messages
// shared by the server and the browser.

use serde::{Deserialize, Serialize};

use crate::games::cards::{HandCard, Motion, Spot, Standing, TableView};

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 8;
/// Stands in for a player id when the ghost hand of a two-player game plays.
pub const GHOST: u8 = 255;

// ── Cards ─────────────────────────────────────────────────────────────────────

/// The four numbered suits. Cutlasses are trumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Suit {
    Palms,
    Pearls,
    Coins,
    Cutlasses,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Palms, Suit::Pearls, Suit::Coins, Suit::Cutlasses];

    pub fn symbol(self) -> &'static str {
        match self {
            Suit::Palms => "🌴",
            Suit::Pearls => "🐚",
            Suit::Coins => "🪙",
            Suit::Cutlasses => "⚔️",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Suit::Palms => "palms",
            Suit::Pearls => "pearls",
            Suit::Coins => "coins",
            Suit::Cutlasses => "cutlasses",
        }
    }

    pub fn is_trump(self) -> bool {
        self == Suit::Cutlasses
    }
}

/// The five pirates. With crew abilities on, winning a trick with one
/// unlocks its ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Crew {
    /// Choose who leads the next trick.
    Navigator,
    /// Draw two cards from the undealt deck, then discard two.
    Smuggler,
    /// Bet 0, 10 or 20 points on making your bid.
    Gambler,
    /// Look through the cards nobody was dealt.
    Seer,
    /// Change your bid by one either way. Works after the last trick too.
    Brute,
}

impl Crew {
    pub const ALL: [Crew; 5] = [
        Crew::Navigator,
        Crew::Smuggler,
        Crew::Gambler,
        Crew::Seer,
        Crew::Brute,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Crew::Navigator => "the Navigator",
            Crew::Smuggler => "the Smuggler",
            Crew::Gambler => "the Gambler",
            Crew::Seer => "the Seer",
            Crew::Brute => "the Brute",
        }
    }

    pub fn ability(self) -> &'static str {
        match self {
            Crew::Navigator => "Choose who leads the next trick.",
            Crew::Smuggler => "Draw two from the deck, then discard two.",
            Crew::Gambler => "Bet 10 or 20 on making your bid.",
            Crew::Seer => "Peek at the cards nobody was dealt.",
            Crew::Brute => "Change your bid by one, up or down.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Face {
    Number {
        suit: Suit,
        rank: u8,
    },
    /// Always loses, unless everyone flees.
    Flag,
    Pirate(Crew),
    /// Played as a pirate or as a flag, the player's choice.
    Turncoat,
    /// Beats pirates; only mermaids beat him.
    Captain,
    Mermaid,
    /// Destroys the trick: nobody wins it.
    Kraken,
    /// Destroys every special card; the highest number wins, suits ignored.
    Whale,
    /// Plays as a flag and forms an alliance with the trick's winner.
    Loot,
}

/// One physical card. `id` tells cards apart on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub face: Face,
}

impl Card {
    pub fn suit(&self) -> Option<Suit> {
        match self.face {
            Face::Number { suit, .. } => Some(suit),
            _ => None,
        }
    }

    pub fn is_number(&self) -> bool {
        self.suit().is_some()
    }

    pub fn name(&self) -> String {
        match self.face {
            Face::Number { suit, rank } => format!("the {rank} of {}", suit.name()),
            Face::Flag => "a white flag".into(),
            Face::Pirate(crew) => crew.name().into(),
            Face::Turncoat => "the Turncoat".into(),
            Face::Captain => "the Captain".into(),
            Face::Mermaid => "a mermaid".into(),
            Face::Kraken => "the Kraken".into(),
            Face::Whale => "the White Whale".into(),
            Face::Loot => "a loot card".into(),
        }
    }

    /// Order for sorting a hand: suits by rank, then the specials.
    pub fn sort_key(&self) -> (u8, u8) {
        match self.face {
            Face::Number { suit, rank } => (suit as u8, rank),
            Face::Flag => (10, 0),
            Face::Loot => (11, 0),
            Face::Mermaid => (12, 0),
            Face::Pirate(crew) => (13, crew as u8),
            Face::Turncoat => (14, 0),
            Face::Captain => (15, 0),
            Face::Kraken => (16, 0),
            Face::Whale => (17, 0),
        }
    }
}

impl HandCard for Card {
    fn id(&self) -> u8 {
        self.id
    }
}

/// Which optional cards and rules a room plays with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub kraken: bool,
    pub whale: bool,
    /// Two loot cards. Never used with two players.
    pub loot: bool,
    /// Pirates' special abilities.
    pub crew: bool,
    pub length: Length,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            kraken: false,
            whale: false,
            loot: false,
            crew: false,
            length: Length::Classic,
        }
    }
}

impl Options {
    pub fn describe(&self) -> String {
        let mut extras = Vec::new();
        if self.kraken {
            extras.push("Kraken");
        }
        if self.whale {
            extras.push("White Whale");
        }
        if self.loot {
            extras.push("Loot");
        }
        if self.crew {
            extras.push("crew abilities");
        }
        let mut s = self.length.label().to_string();
        if !extras.is_empty() {
            s.push_str(" · ");
            s.push_str(&extras.join(", "));
        }
        s
    }
}

/// How many cards are dealt in each round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Length {
    /// 1 to 10.
    Classic,
    /// 2, 4, 6, 8, 10.
    Even,
    /// 6 to 10.
    Brawl,
    /// Five rounds of 5.
    Skirmish,
    /// Ten rounds of 10.
    Barrage,
    /// 9, 7, 5, 3, 1 twice.
    Whirlpool,
}

impl Length {
    pub const ALL: [Length; 6] = [
        Length::Classic,
        Length::Even,
        Length::Brawl,
        Length::Skirmish,
        Length::Barrage,
        Length::Whirlpool,
    ];

    pub fn rounds(self) -> Vec<u8> {
        match self {
            Length::Classic => (1..=10).collect(),
            Length::Even => vec![2, 4, 6, 8, 10],
            Length::Brawl => (6..=10).collect(),
            Length::Skirmish => vec![5; 5],
            Length::Barrage => vec![10; 10],
            Length::Whirlpool => vec![9, 7, 5, 3, 1, 9, 7, 5, 3, 1],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Length::Classic => "Ten rounds, 1 to 10 cards",
            Length::Even => "Even keel: 2, 4, 6, 8, 10 cards",
            Length::Brawl => "Straight to the brawl: 6 to 10 cards",
            Length::Skirmish => "Skirmish: five rounds of 5",
            Length::Barrage => "Barrage: ten rounds of 10",
            Length::Whirlpool => "Whirlpool: 9, 7, 5, 3, 1, twice",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Length::Classic => "classic",
            Length::Even => "even",
            Length::Brawl => "brawl",
            Length::Skirmish => "skirmish",
            Length::Barrage => "barrage",
            Length::Whirlpool => "whirlpool",
        }
    }
}

/// The deck for these options: four suits of 1-14, five flags, five
/// pirates, the Turncoat, the Captain, two mermaids, plus any extras.
pub fn full_deck(options: &Options, players: usize) -> Vec<Card> {
    let mut faces = Vec::with_capacity(74);
    for suit in Suit::ALL {
        for rank in 1..=14 {
            faces.push(Face::Number { suit, rank });
        }
    }
    faces.extend([Face::Flag; 5]);
    faces.extend(Crew::ALL.map(Face::Pirate));
    faces.extend([Face::Turncoat, Face::Captain, Face::Mermaid, Face::Mermaid]);
    if options.kraken {
        faces.push(Face::Kraken);
    }
    if options.whale {
        faces.push(Face::Whale);
    }
    if options.loot && players > 2 {
        faces.extend([Face::Loot; 2]);
    }
    faces
        .into_iter()
        .enumerate()
        .map(|(id, face)| Card { id: id as u8, face })
        .collect()
}

/// Cards per hand: the round's count, capped so every hand can be dealt.
pub fn hand_size(scheduled: u8, deck: usize, hands: usize) -> u8 {
    scheduled.min((deck / hands.max(1)) as u8)
}

// ── Tricks ────────────────────────────────────────────────────────────────────

/// A card on the table. `pirate` is the Turncoat's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Played {
    pub player: u8,
    pub card: Card,
    pub pirate: bool,
}

/// What a card counts as in a trick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Number(Suit, u8),
    Flag,
    Pirate,
    Captain,
    Mermaid,
    Kraken,
    Whale,
}

fn role(p: &Played) -> Role {
    match p.card.face {
        Face::Number { suit, rank } => Role::Number(suit, rank),
        Face::Flag | Face::Loot => Role::Flag,
        Face::Pirate(_) => Role::Pirate,
        Face::Turncoat if p.pirate => Role::Pirate,
        Face::Turncoat => Role::Flag,
        Face::Captain => Role::Captain,
        Face::Mermaid => Role::Mermaid,
        Face::Kraken => Role::Kraken,
        Face::Whale => Role::Whale,
    }
}

/// The suit everyone must follow, if any: set by the first numbered card,
/// unless a character (pirate, mermaid, Captain) or a Kraken or Whale came
/// first. A Kraken or Whale anywhere frees everyone after it.
pub fn lead_suit(trick: &[Played]) -> Option<Suit> {
    if trick
        .iter()
        .any(|p| matches!(role(p), Role::Kraken | Role::Whale))
    {
        return None;
    }
    for p in trick {
        match role(p) {
            Role::Flag => continue,
            Role::Number(suit, _) => return Some(suit),
            _ => return None,
        }
    }
    None
}

/// Whether `card` may be played from `hand` onto `trick`. Specials can always
/// be played; a numbered card must follow the lead suit if the hand has it.
pub fn can_play(hand: &[Card], trick: &[Played], card: &Card) -> bool {
    let Some(suit) = card.suit() else {
        return true;
    };
    match lead_suit(trick) {
        Some(lead) if suit != lead => !hand.iter().any(|c| c.suit() == Some(lead)),
        _ => true,
    }
}

/// The ordinary winner, ignoring any Kraken or Whale: mermaids take the
/// Captain, the Captain takes pirates, pirates take mermaids, all of them
/// take numbers, trumps take the other suits, and the lead suit takes the
/// rest. Ties go to whoever played first. If everyone fled, the first flag wins.
fn usual_winner(trick: &[Played]) -> usize {
    let roles: Vec<Role> = trick
        .iter()
        .map(|p| match role(p) {
            Role::Kraken | Role::Whale => Role::Flag,
            r => r,
        })
        .collect();
    let first = |want: Role| roles.iter().position(|&r| r == want);
    let mermaid = first(Role::Mermaid);
    let captain = first(Role::Captain);
    if let (Some(m), Some(_)) = (mermaid, captain) {
        return m;
    }
    if let Some(c) = captain {
        return c;
    }
    if let Some(p) = first(Role::Pirate) {
        return p;
    }
    if let Some(m) = mermaid {
        return m;
    }
    let best_of = |suit: Suit| {
        roles
            .iter()
            .enumerate()
            .filter_map(|(i, r)| match r {
                Role::Number(s, rank) if *s == suit => Some((i, *rank)),
                _ => None,
            })
            .max_by_key(|&(i, rank)| (rank, std::cmp::Reverse(i)))
            .map(|(i, _)| i)
    };
    if let Some(t) = best_of(Suit::Cutlasses) {
        return t;
    }
    let lead = roles.iter().find_map(|r| match r {
        Role::Number(s, _) => Some(*s),
        _ => None,
    });
    lead.and_then(best_of).unwrap_or(0)
}

/// How a finished trick came out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// Index into the trick of the winning card, or `None` when the trick
    /// was destroyed.
    pub winner: Option<usize>,
    /// Index of the card whose player leads next: the winner, or whoever
    /// would have won a destroyed trick.
    pub next: usize,
    /// Bonus points the winner banks if they make their bid.
    pub bonus: i32,
    /// Why the trick went the way it did, in words.
    pub why: String,
}

pub fn resolve(trick: &[Played]) -> Outcome {
    let usual = usual_winner(trick);
    // When both the Kraken and the Whale are played, the later one counts.
    let beast = trick
        .iter()
        .rposition(|p| matches!(role(p), Role::Kraken | Role::Whale));
    let (winner, why) = match beast.map(|b| role(&trick[b])) {
        Some(Role::Kraken) => (None, "The Kraken dragged the trick under.".to_string()),
        Some(Role::Whale) => {
            let best = trick
                .iter()
                .enumerate()
                .filter_map(|(i, p)| match p.card.face {
                    Face::Number { rank, .. } => Some((i, rank)),
                    _ => None,
                })
                .max_by_key(|&(i, rank)| (rank, std::cmp::Reverse(i)));
            match best {
                Some((i, rank)) => (
                    Some(i),
                    format!("The White Whale swallowed the specials; the {rank} was highest."),
                ),
                None => (
                    None,
                    "The White Whale swallowed the whole trick.".to_string(),
                ),
            }
        }
        _ => (Some(usual), String::new()),
    };
    let bonus = winner.map_or(0, |w| bonus(trick, w));
    Outcome {
        winner,
        next: winner.unwrap_or(usual),
        bonus,
        why,
    }
}

/// Points for what the winning card captured: 10 for each 14 (20 for the
/// trump 14), 20 per mermaid a pirate takes, 30 per pirate the Captain
/// takes, 40 for a mermaid taking the Captain.
fn bonus(trick: &[Played], winner: usize) -> i32 {
    let mut points = 0;
    for p in trick {
        if let Face::Number { suit, rank: 14 } = p.card.face {
            points += if suit.is_trump() { 20 } else { 10 };
        }
    }
    let count = |want: Role| trick.iter().filter(|p| role(p) == want).count() as i32;
    points += match role(&trick[winner]) {
        Role::Pirate => 20 * count(Role::Mermaid),
        Role::Captain => 30 * count(Role::Pirate),
        Role::Mermaid if count(Role::Captain) > 0 => 40,
        _ => 0,
    };
    points
}

/// A round's points for one player. `dealt` is the cards per hand.
pub fn round_points(bid: u8, won: u8, dealt: u8, bonus: i32) -> (i32, bool) {
    let made = bid == won;
    let base = match (bid, made) {
        (0, true) => 10 * dealt as i32,
        (0, false) => -10 * dealt as i32,
        (_, true) => 20 * bid as i32,
        (_, false) => -10 * (bid as i32 - won as i32).abs(),
    };
    (if made { base + bonus } else { base }, made)
}

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    pub total: i32,
    /// This round's bid, once everyone has bid (always for yourself).
    pub bid: Option<u8>,
    pub has_bid: bool,
    pub won: u8,
    /// Points bet with the Gambler.
    pub bet: i32,
}

/// The last finished trick, shown until the next one starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrickView {
    pub cards: Vec<Played>,
    /// Who took it, `None` if it was destroyed.
    pub winner: Option<u8>,
    pub next: u8,
    pub bonus: i32,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundResult {
    pub player: u8,
    pub bid: u8,
    pub won: u8,
    /// Points for the bid alone.
    pub base: i32,
    /// Capture bonuses, counted only when the bid was made.
    pub bonus: i32,
    /// Loot alliance points.
    pub loot: i32,
    /// The Gambler's bet, won or lost.
    pub bet: i32,
    pub total: i32,
    pub made: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Stage {
    /// Everyone bids at once; bids are revealed together.
    Bidding,
    Playing,
    /// A pirate's ability waits on `player`. `cards` are the undealt cards
    /// for the Seer, shown only to that player.
    Ability {
        player: u8,
        crew: Crew,
        cards: Vec<Card>,
    },
    RoundOver {
        results: Vec<RoundResult>,
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
    Deal,
    Bid,
    /// `done` when this card finished the trick.
    Played {
        card: Card,
        done: bool,
    },
    /// The Smuggler drew and discarded.
    Smuggled,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    /// 1-based. `rounds` grows if a tie forces an extra round.
    pub round: u8,
    pub rounds: u8,
    /// Cards per hand this round.
    pub dealt: u8,
    pub options: Options,
    /// Whether the ghost hand plays (two-player games).
    pub ghost: bool,
    pub players: Vec<PlayerView>,
    pub hand: Vec<Card>,
    pub trick: Vec<Played>,
    pub last_trick: Option<TrickView>,
    /// Whose card is next, `GHOST` for the ghost hand.
    pub current: Option<u8>,
    /// Who led the first trick of this round.
    pub starter: u8,
    pub tricks_left: u8,
    pub stage: Stage,
    pub log: Vec<String>,
    pub last: Option<Action>,
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
            rounds: 10,
            dealt: 1,
            options: Options::default(),
            ghost: false,
            players: Vec::new(),
            hand: Vec::new(),
            trick: Vec::new(),
            last_trick: None,
            current: None,
            starter: 0,
            tricks_left: 0,
            stage: Stage::Bidding,
            log: Vec::new(),
            last: None,
        }
    }

    /// Cards on the table carry `data-card` like the hand does, so your own
    /// card slides from your hand onto the table by itself.
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
            ActionKind::Played { .. } if a.player != self.you => Motion {
                arrive_from: Some(Spot::Seat(a.player)),
                ..Motion::default()
            },
            ActionKind::Smuggled if a.player == self.you => Motion {
                arrive_from: Some(Spot::Pile("deck")),
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
    Bid {
        bid: u8,
    },
    /// `pirate` is the Turncoat's choice; ignored for other cards.
    Play {
        card: u8,
        pirate: bool,
    },
    /// The Navigator's pick to lead the next trick.
    ChooseLeader {
        player: u8,
    },
    /// The Smuggler's two discards.
    Discard {
        cards: Vec<u8>,
    },
    /// The Gambler's bet: 0, 10 or 20.
    Bet {
        points: u8,
    },
    /// The Brute's change: -1, 0 or 1.
    ChangeBid {
        by: i8,
    },
    /// Done looking (the Seer).
    Done,
    Arrange {
        order: Vec<u8>,
    },
    NextRound,
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(player: u8, suit: Suit, rank: u8) -> Played {
        Played {
            player,
            card: Card {
                id: 0,
                face: Face::Number { suit, rank },
            },
            pirate: false,
        }
    }

    fn special(player: u8, face: Face) -> Played {
        Played {
            player,
            card: Card { id: 0, face },
            pirate: false,
        }
    }

    fn winner(trick: &[Played]) -> Option<u8> {
        resolve(trick).winner.map(|i| trick[i].player)
    }

    #[test]
    fn deck_sizes() {
        let base = Options::default();
        assert_eq!(full_deck(&base, 4).len(), 70);
        let all = Options {
            kraken: true,
            whale: true,
            loot: true,
            crew: true,
            length: Length::Classic,
        };
        assert_eq!(full_deck(&all, 4).len(), 74);
        assert_eq!(full_deck(&all, 2).len(), 72);
        // Eight players get at most eight cards each.
        assert_eq!(hand_size(10, 70, 8), 8);
        assert_eq!(hand_size(10, 70, 7), 10);
    }

    #[test]
    fn suits_and_trumps() {
        use Suit::*;
        assert_eq!(
            winner(&[num(0, Palms, 7), num(1, Palms, 12), num(2, Palms, 8)]),
            Some(1)
        );
        // Off-suit loses even when higher.
        assert_eq!(
            winner(&[num(0, Coins, 12), num(1, Coins, 5), num(2, Pearls, 14)]),
            Some(0)
        );
        // Any trump beats the lead suit.
        assert_eq!(
            winner(&[num(0, Coins, 12), num(1, Coins, 5), num(2, Cutlasses, 2)]),
            Some(2)
        );
    }

    #[test]
    fn characters() {
        use Suit::*;
        let pirate = Face::Pirate(Crew::Navigator);
        // First pirate wins among pirates.
        assert_eq!(
            winner(&[
                num(0, Cutlasses, 14),
                special(1, pirate),
                special(2, Face::Pirate(Crew::Brute))
            ]),
            Some(1)
        );
        // Pirate beats mermaid.
        assert_eq!(
            winner(&[special(0, Face::Mermaid), special(1, pirate)]),
            Some(1)
        );
        // Captain beats pirates.
        assert_eq!(
            winner(&[special(0, pirate), special(1, Face::Captain)]),
            Some(1)
        );
        // Mermaid takes the Captain, even with a pirate in the trick.
        let trick = [
            num(0, Coins, 14),
            special(1, pirate),
            special(2, Face::Captain),
            special(3, Face::Mermaid),
        ];
        let out = resolve(&trick);
        assert_eq!(out.winner, Some(3));
        assert_eq!(out.bonus, 10 + 40);
        // First mermaid wins between two.
        assert_eq!(
            winner(&[special(0, Face::Mermaid), special(1, Face::Mermaid)]),
            Some(0)
        );
    }

    #[test]
    fn flags_and_turncoat() {
        use Suit::*;
        assert_eq!(
            winner(&[special(0, Face::Flag), special(1, Face::Flag)]),
            Some(0)
        );
        assert_eq!(winner(&[special(0, Face::Flag), num(1, Palms, 1)]), Some(1));
        let mut turncoat = special(1, Face::Turncoat);
        turncoat.pirate = true;
        assert_eq!(winner(&[special(0, Face::Mermaid), turncoat]), Some(1));
        turncoat.pirate = false;
        assert_eq!(winner(&[num(0, Palms, 3), turncoat]), Some(0));
        // The Captain takes a Turncoat played as a pirate: 30 points.
        turncoat.pirate = true;
        let out = resolve(&[turncoat, special(0, Face::Captain)]);
        assert_eq!(out.bonus, 30);
    }

    #[test]
    fn beasts() {
        use Suit::*;
        let trick = [
            num(0, Palms, 5),
            special(1, Face::Captain),
            special(2, Face::Kraken),
        ];
        let out = resolve(&trick);
        assert_eq!(out.winner, None);
        assert_eq!(trick[out.next].player, 1);

        // The rulebook's example: the 14 wins, specials gone.
        let trick = [
            num(0, Cutlasses, 2),
            special(1, Face::Pirate(Crew::Seer)),
            num(2, Coins, 14),
            special(3, Face::Captain),
            special(4, Face::Whale),
        ];
        let out = resolve(&trick);
        assert_eq!(out.winner, Some(2));
        assert_eq!(out.bonus, 10);

        // Whale with only specials: discarded.
        let trick = [special(0, Face::Mermaid), special(1, Face::Whale)];
        assert_eq!(resolve(&trick).winner, None);

        // Both beasts: the later one decides.
        let trick = [
            num(0, Palms, 9),
            special(1, Face::Kraken),
            special(2, Face::Whale),
        ];
        assert_eq!(winner(&trick), Some(0));
        let trick = [
            num(0, Palms, 9),
            special(1, Face::Whale),
            special(2, Face::Kraken),
        ];
        assert_eq!(winner(&trick), None);
    }

    #[test]
    fn following_suit() {
        use Suit::*;
        let hand = [
            Card {
                id: 1,
                face: Face::Number {
                    suit: Palms,
                    rank: 3,
                },
            },
            Card {
                id: 2,
                face: Face::Number {
                    suit: Coins,
                    rank: 9,
                },
            },
            Card {
                id: 3,
                face: Face::Mermaid,
            },
        ];
        let trick = [num(0, Palms, 10)];
        assert!(can_play(&hand, &trick, &hand[0]));
        assert!(!can_play(&hand, &trick, &hand[1]));
        assert!(can_play(&hand, &trick, &hand[2]));
        // A flag lead lets the next card set the suit.
        let trick = [special(0, Face::Flag), num(1, Coins, 2)];
        assert!(!can_play(&hand, &trick, &hand[0]));
        // A character lead means no suit to follow.
        let trick = [special(0, Face::Mermaid), num(1, Coins, 2)];
        assert!(can_play(&hand, &trick, &hand[0]));
    }

    #[test]
    fn scoring() {
        assert_eq!(round_points(3, 3, 5, 0), (60, true));
        assert_eq!(round_points(2, 4, 5, 30), (-20, false));
        assert_eq!(round_points(0, 0, 7, 0), (70, true));
        assert_eq!(round_points(0, 2, 9, 10), (-90, false));
        assert_eq!(round_points(1, 1, 3, 50), (70, true));
    }
}

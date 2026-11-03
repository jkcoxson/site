// Jackson Coxson
// Age of Monuments: the buildings, the cities and their monuments, trading
// with neighbors, scoring and the wire messages shared by the server and
// the browser.
//
// Three ages of six turns. Each turn everyone picks a card from their hand
// at once and builds it, uses it to raise a stage of their monument, or
// sells it for coins; then hands pass to the next player. Resources you
// lack can be bought from the cities on either side.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::games::cards::seating::{left_of, right_of, Pass};
use crate::games::cards::{Motion, Spot, Standing, TableView};

pub const MIN_PLAYERS: usize = 3;
pub const MAX_PLAYERS: usize = 7;
pub const AGES: u8 = 3;
/// Turns per age; each hand starts with one more card than that.
pub const TURNS: u8 = 6;
pub const HAND: usize = 7;
pub const START_COINS: i32 = 3;
pub const SELL_COINS: i32 = 3;

// ── Resources ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Res {
    Wood,
    Stone,
    Clay,
    Ore,
    Glass,
    Cloth,
    Papyrus,
}

impl Res {
    pub const ALL: [Res; 7] = [
        Res::Wood,
        Res::Stone,
        Res::Clay,
        Res::Ore,
        Res::Glass,
        Res::Cloth,
        Res::Papyrus,
    ];
    pub const RAW: [Res; 4] = [Res::Wood, Res::Stone, Res::Clay, Res::Ore];
    pub const GOODS: [Res; 3] = [Res::Glass, Res::Cloth, Res::Papyrus];

    pub fn idx(self) -> usize {
        self as usize
    }

    /// Raw materials, as opposed to crafted goods.
    pub fn is_raw(self) -> bool {
        self.idx() < 4
    }

    pub fn icon(self) -> &'static str {
        match self {
            Res::Wood => "🪵",
            Res::Stone => "🪨",
            Res::Clay => "🧱",
            Res::Ore => "⛏️",
            Res::Glass => "🫙",
            Res::Cloth => "🧵",
            Res::Papyrus => "📜",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Res::Wood => "wood",
            Res::Stone => "stone",
            Res::Clay => "clay",
            Res::Ore => "ore",
            Res::Glass => "glass",
            Res::Cloth => "cloth",
            Res::Papyrus => "papyrus",
        }
    }
}

const W: Res = Res::Wood;
const S: Res = Res::Stone;
const C: Res = Res::Clay;
const O: Res = Res::Ore;
const G: Res = Res::Glass;
const L: Res = Res::Cloth;
const P: Res = Res::Papyrus;

/// Icons for a list of resources, e.g. "🪵🪵🧱".
pub fn icons(cost: &[Res]) -> String {
    cost.iter().map(|r| r.icon()).collect()
}

// ── Cards ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    /// Raw materials.
    Brown,
    /// Crafted goods.
    Gray,
    /// Civic buildings: points.
    Blue,
    /// Commerce: coins and trade.
    Yellow,
    /// Military: shields.
    Red,
    /// Science.
    Green,
    /// Leagues: points for what you and your neighbors built.
    Purple,
}

impl Color {
    pub const ALL: [Color; 7] = [
        Color::Brown,
        Color::Gray,
        Color::Yellow,
        Color::Red,
        Color::Blue,
        Color::Green,
        Color::Purple,
    ];

    pub fn kind(self) -> &'static str {
        match self {
            Color::Brown => "Raw materials",
            Color::Gray => "Crafted goods",
            Color::Blue => "Civic building",
            Color::Yellow => "Commerce",
            Color::Red => "Military",
            Color::Green => "Science",
            Color::Purple => "League",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            Color::Brown => "brown cards",
            Color::Gray => "gray cards",
            Color::Blue => "blue cards",
            Color::Yellow => "yellow cards",
            Color::Red => "red cards",
            Color::Green => "green cards",
            Color::Purple => "purple cards",
        }
    }
}

/// The three fields of science.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sci {
    Stars,
    Gears,
    Quills,
}

impl Sci {
    pub const ALL: [Sci; 3] = [Sci::Stars, Sci::Gears, Sci::Quills];

    pub fn icon(self) -> &'static str {
        match self {
            Sci::Stars => "🔭",
            Sci::Gears => "⚙️",
            Sci::Quills => "🪶",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sci::Stars => "astronomy",
            Sci::Gears => "engineering",
            Sci::Quills => "letters",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discount {
    /// Raw materials from your left neighbor cost 1.
    RawLeft,
    /// Raw materials from your right neighbor cost 1.
    RawRight,
    /// Crafted goods from either neighbor cost 1.
    Goods,
}

/// What an Age III commerce card pays for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Per {
    Cards(Color),
    /// Stages of your monument.
    Stages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum League {
    /// Points per card of a color in both neighboring cities.
    Neighbors(Color, u8),
    /// A point per defeat in both neighboring cities.
    Defeats,
    /// A point per brown, gray and purple card in your own city.
    Shipwrights,
    /// A science symbol of your choice.
    Scholars,
    /// A point per monument stage in your city and both neighbors'.
    Masons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// One of these resources each turn, `n` units of it. Brown and gray
    /// cards can be bought from by neighbors; others can't.
    Make(&'static [Res], u8),
    Points(u8),
    Shields(u8),
    Science(Sci),
    /// Coins from the bank once, when built.
    Coins(u8),
    Discount(Discount),
    /// Coins once, per card of the color in your city and both neighbors'.
    CoinsAround(Color, u8),
    /// Coins once when built and points at the end, per thing in your city.
    Bounty(Per, u8, u8),
    League(League),
}

#[derive(Debug)]
pub struct CardDef {
    pub name: &'static str,
    pub color: Color,
    /// Coins paid to the bank to build it.
    pub coins: u8,
    pub cost: &'static [Res],
    pub effect: Effect,
    /// Built free when you already have one of these.
    pub free_with: &'static [&'static str],
    /// (age, fewest players it's used with). Leagues say 0: a few are picked
    /// at random for Age III.
    pub copies: &'static [(u8, u8)],
}

const fn card(
    name: &'static str,
    color: Color,
    coins: u8,
    cost: &'static [Res],
    effect: Effect,
    free_with: &'static [&'static str],
    copies: &'static [(u8, u8)],
) -> CardDef {
    CardDef {
        name,
        color,
        coins,
        cost,
        effect,
        free_with,
        copies,
    }
}

use Color::{Blue, Brown, Gray, Green, Purple, Red, Yellow};
use Effect::{Bounty, CoinsAround, Make, Points, Science, Shields};

const LEAGUE: &[(u8, u8)] = &[(3, 0)];

#[rustfmt::skip]
pub static CARDS: &[CardDef] = &[
    // Age I raw materials.
    card("Woodcutter's Camp", Brown, 0, &[], Make(&[W], 1), &[], &[(1, 3), (1, 4)]),
    card("Boulder Field", Brown, 0, &[], Make(&[S], 1), &[], &[(1, 3), (1, 5)]),
    card("Mud Flats", Brown, 0, &[], Make(&[C], 1), &[], &[(1, 3), (1, 5)]),
    card("Copper Seam", Brown, 0, &[], Make(&[O], 1), &[], &[(1, 3), (1, 4)]),
    card("Orchard", Brown, 1, &[], Make(&[W, C], 1), &[], &[(1, 6)]),
    card("Dig Site", Brown, 1, &[], Make(&[S, C], 1), &[], &[(1, 4)]),
    card("Red Hills", Brown, 1, &[], Make(&[C, O], 1), &[], &[(1, 3)]),
    card("Hillside Grove", Brown, 1, &[], Make(&[S, W], 1), &[], &[(1, 3)]),
    card("Forest Hollow", Brown, 1, &[], Make(&[W, O], 1), &[], &[(1, 5)]),
    card("Old Mine", Brown, 1, &[], Make(&[S, O], 1), &[], &[(1, 6)]),
    // Age II raw materials.
    card("Timber Mill", Brown, 1, &[], Make(&[W], 2), &[], &[(2, 3), (2, 4)]),
    card("Great Quarry", Brown, 1, &[], Make(&[S], 2), &[], &[(2, 3), (2, 4)]),
    card("Kilnworks", Brown, 1, &[], Make(&[C], 2), &[], &[(2, 3), (2, 4)]),
    card("Smeltery", Brown, 1, &[], Make(&[O], 2), &[], &[(2, 3), (2, 4)]),
    // Crafted goods, Ages I and II.
    card("Weavery", Gray, 0, &[], Make(&[L], 1), &[], &[(1, 3), (1, 6), (2, 3), (2, 5)]),
    card("Glassblower", Gray, 0, &[], Make(&[G], 1), &[], &[(1, 3), (1, 6), (2, 3), (2, 5)]),
    card("Reed Mill", Gray, 0, &[], Make(&[P], 1), &[], &[(1, 3), (1, 6), (2, 3), (2, 5)]),
    // Civic buildings.
    card("Moneylender", Blue, 0, &[], Points(3), &[], &[(1, 4), (1, 7)]),
    card("Fountain", Blue, 0, &[S], Points(3), &[], &[(1, 3), (1, 7)]),
    card("Shrine", Blue, 0, &[], Points(2), &[], &[(1, 3), (1, 5)]),
    card("Playhouse", Blue, 0, &[], Points(2), &[], &[(1, 3), (1, 6)]),
    card("Cistern", Blue, 0, &[S, S, S], Points(5), &["Fountain"], &[(2, 3), (2, 7)]),
    card("Sanctuary", Blue, 0, &[W, C, G], Points(3), &["Shrine"], &[(2, 3), (2, 6)]),
    card("Bronze Hero", Blue, 0, &[O, O, W], Points(4), &["Playhouse"], &[(2, 3), (2, 7)]),
    card("Hall of Laws", Blue, 0, &[C, C, L], Points(4), &["Scribe House"], &[(2, 3), (2, 5)]),
    card("Hall of Gods", Blue, 0, &[C, C, O, G, P, L], Points(7), &["Sanctuary"], &[(3, 3), (3, 6)]),
    card("Pleasure Gardens", Blue, 0, &[C, C, W], Points(5), &["Bronze Hero"], &[(3, 3), (3, 4)]),
    card("Council House", Blue, 0, &[S, S, O, G], Points(6), &[], &[(3, 3), (3, 5), (3, 6)]),
    card("High Palace", Blue, 0, &[S, O, W, C, G, P, L], Points(8), &[], &[(3, 3), (3, 7)]),
    card("Assembly", Blue, 0, &[W, W, S, O], Points(6), &["Archive"], &[(3, 3), (3, 5)]),
    // Commerce.
    card("Inn", Yellow, 0, &[], Effect::Coins(5), &[], &[(1, 4), (1, 5), (1, 7)]),
    card("Eastern Road", Yellow, 0, &[], Effect::Discount(Discount::RawRight), &[], &[(1, 3), (1, 7)]),
    card("Western Road", Yellow, 0, &[], Effect::Discount(Discount::RawLeft), &[], &[(1, 3), (1, 7)]),
    card("Merchant Square", Yellow, 0, &[], Effect::Discount(Discount::Goods), &[], &[(1, 3), (1, 6)]),
    card("Agora", Yellow, 0, &[C, C], Make(&[G, L, P], 1), &["Eastern Road", "Western Road"], &[(2, 3), (2, 6), (2, 7)]),
    card("Caravan Yard", Yellow, 0, &[W, W], Make(&[W, S, C, O], 1), &["Merchant Square"], &[(2, 3), (2, 5), (2, 6)]),
    card("Olive Groves", Yellow, 0, &[], CoinsAround(Brown, 1), &[], &[(2, 3), (2, 6)]),
    card("Spice Market", Yellow, 0, &[], CoinsAround(Gray, 2), &[], &[(2, 4), (2, 7)]),
    card("Trade Harbor", Yellow, 0, &[W, O, L], Bounty(Per::Cards(Brown), 1, 1), &["Agora"], &[(3, 3), (3, 4)]),
    card("Beacon Tower", Yellow, 0, &[S, G], Bounty(Per::Cards(Yellow), 1, 1), &["Caravan Yard"], &[(3, 3), (3, 6)]),
    card("Merchants' Exchange", Yellow, 0, &[C, C, P], Bounty(Per::Cards(Gray), 2, 2), &[], &[(3, 4), (3, 6)]),
    card("Hippodrome", Yellow, 0, &[S, S, O], Bounty(Per::Stages, 3, 1), &["Star Tower"], &[(3, 3), (3, 5), (3, 7)]),
    // Military.
    card("Palisade", Red, 0, &[W], Shields(1), &[], &[(1, 3), (1, 7)]),
    card("Militia Hall", Red, 0, &[O], Shields(1), &[], &[(1, 3), (1, 5)]),
    card("Watchtower", Red, 0, &[C], Shields(1), &[], &[(1, 3), (1, 4)]),
    card("City Walls", Red, 0, &[S, S, S], Shields(2), &[], &[(2, 3), (2, 7)]),
    card("Drill Field", Red, 0, &[O, O, W], Shields(2), &[], &[(2, 4), (2, 6), (2, 7)]),
    card("Horse Pens", Red, 0, &[O, C, W], Shields(2), &["Stargazer"], &[(2, 3), (2, 5)]),
    card("Bowyers' Yard", Red, 0, &[W, W, O], Shields(2), &["Tinkerer"], &[(2, 3), (2, 6)]),
    card("Citadel", Red, 0, &[O, O, O, S], Shields(3), &["City Walls"], &[(3, 3), (3, 7)]),
    card("Legion Camp", Red, 0, &[S, S, S, O], Shields(3), &["Drill Field"], &[(3, 4), (3, 5), (3, 6)]),
    card("Armory", Red, 0, &[W, W, O, L], Shields(3), &[], &[(3, 3), (3, 4), (3, 7)]),
    card("Siege Engines", Red, 0, &[C, C, C, W], Shields(3), &["Gearworks"], &[(3, 3), (3, 5)]),
    // Science.
    card("Stargazer", Green, 0, &[L], Science(Sci::Stars), &[], &[(1, 3), (1, 5)]),
    card("Tinkerer", Green, 0, &[G], Science(Sci::Gears), &[], &[(1, 3), (1, 7)]),
    card("Scribe House", Green, 0, &[P], Science(Sci::Quills), &[], &[(1, 3), (1, 4)]),
    card("Star Tower", Green, 0, &[O, O, G], Science(Sci::Stars), &["Stargazer"], &[(2, 3), (2, 4)]),
    card("Gearworks", Green, 0, &[C, C, P], Science(Sci::Gears), &["Tinkerer"], &[(2, 3), (2, 5)]),
    card("Archive", Green, 0, &[S, S, L], Science(Sci::Quills), &["Scribe House"], &[(2, 3), (2, 6)]),
    card("Lyceum", Green, 0, &[W, P], Science(Sci::Quills), &[], &[(2, 3), (2, 7)]),
    card("Moon Temple", Green, 0, &[C, C, L, P], Science(Sci::Stars), &["Star Tower"], &[(3, 3), (3, 6)]),
    card("Clocktower", Green, 0, &[O, O, G, L], Science(Sci::Gears), &["Gearworks"], &[(3, 3), (3, 7)]),
    card("Athenaeum", Green, 0, &[W, W, P, G], Science(Sci::Quills), &["Archive"], &[(3, 3), (3, 4)]),
    card("Orrery Hall", Green, 0, &[S, S, S, G], Science(Sci::Stars), &["Lyceum"], &[(3, 3), (3, 7)]),
    card("Drafting Room", Green, 0, &[W, P, L], Science(Sci::Gears), &["Lyceum"], &[(3, 3), (3, 5)]),
    // Leagues.
    card("Miners' League", Purple, 0, &[O, O, C, S, W], Effect::League(League::Neighbors(Brown, 1)), &[], LEAGUE),
    card("Artisans' League", Purple, 0, &[O, O, S, S], Effect::League(League::Neighbors(Gray, 2)), &[], LEAGUE),
    card("Merchants' League", Purple, 0, &[G, P, L], Effect::League(League::Neighbors(Yellow, 1)), &[], LEAGUE),
    card("Thinkers' League", Purple, 0, &[C, C, C, L, P], Effect::League(League::Neighbors(Green, 1)), &[], LEAGUE),
    card("Watchers' League", Purple, 0, &[C, C, C, G], Effect::League(League::Neighbors(Red, 1)), &[], LEAGUE),
    card("Judges' League", Purple, 0, &[W, W, W, S, L], Effect::League(League::Neighbors(Blue, 1)), &[], LEAGUE),
    card("Generals' League", Purple, 0, &[O, O, S, L], Effect::League(League::Defeats), &[], LEAGUE),
    card("Shipwrights' League", Purple, 0, &[W, W, W, G, P], Effect::League(League::Shipwrights), &[], LEAGUE),
    card("Scholars' League", Purple, 0, &[W, W, O, O, P], Effect::League(League::Scholars), &[], LEAGUE),
    card("Masons' League", Purple, 0, &[S, S, C, C, G], Effect::League(League::Masons), &[], LEAGUE),
];

/// One physical card: which building, in which age's deck, and the fewest
/// players it's used with (0 for leagues). Its id is its place in `deck()`.
#[derive(Debug, Clone, Copy)]
pub struct Copy {
    pub def: u8,
    pub age: u8,
    pub players: u8,
}

/// Every physical card in the game, in id order.
pub fn deck() -> &'static [Copy] {
    static DECK: OnceLock<Vec<Copy>> = OnceLock::new();
    DECK.get_or_init(|| {
        let mut deck = Vec::new();
        for (def, c) in CARDS.iter().enumerate() {
            for &(age, players) in c.copies {
                deck.push(Copy {
                    def: def as u8,
                    age,
                    players,
                });
            }
        }
        deck
    })
}

/// The building a card id shows.
pub fn def(id: u8) -> &'static CardDef {
    &CARDS[deck()[id as usize].def as usize]
}

pub fn def_index(id: u8) -> u8 {
    deck()[id as usize].def
}

pub fn age_of(id: u8) -> u8 {
    deck()[id as usize].age
}

#[cfg(test)]
fn def_named(name: &str) -> Option<&'static CardDef> {
    CARDS.iter().find(|c| c.name == name)
}

/// Buildings the given one makes free in later ages.
pub fn leads_to(def: &CardDef) -> Vec<&'static str> {
    CARDS
        .iter()
        .filter(|c| c.free_with.contains(&def.name))
        .map(|c| c.name)
        .collect()
}

/// The card ids for one age's deck with `players` at the table, leagues not
/// included.
pub fn age_cards(age: u8, players: usize) -> Vec<u8> {
    deck()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.age == age && c.players > 0 && c.players as usize <= players)
        .map(|(id, _)| id as u8)
        .collect()
}

/// Every league card id.
pub fn league_cards() -> Vec<u8> {
    deck()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.players == 0)
        .map(|(id, _)| id as u8)
        .collect()
}

// ── Cities ────────────────────────────────────────────────────────────────────

/// What a stage of a monument gives once built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gift {
    Points(u8),
    Coins(u8),
    Shields(u8),
    /// One raw material of your choice each turn (not for sale).
    AnyRaw,
    /// One crafted good of your choice each turn (not for sale).
    AnyGoods,
    /// A science symbol of your choice at the end.
    Science,
    /// Once per age, build a card for free.
    FreeBuild,
    /// Build a card from the discards for free, right away.
    Salvage,
    /// Play the last card of each age instead of discarding it.
    Seventh,
    /// Raw materials from both neighbors cost 1.
    CheapRaw,
    /// At the end, copy a league from a neighbor.
    CopyLeague,
}

#[derive(Debug)]
pub struct StageDef {
    pub cost: &'static [Res],
    pub gifts: &'static [Gift],
}

#[derive(Debug)]
pub struct Board {
    pub city: &'static str,
    pub monument: &'static str,
    /// What the city makes from the start (for sale to neighbors).
    pub makes: Res,
    pub a: &'static [StageDef],
    pub b: &'static [StageDef],
}

const fn stage(cost: &'static [Res], gifts: &'static [Gift]) -> StageDef {
    StageDef { cost, gifts }
}

pub static BOARDS: [Board; 7] = [
    Board {
        city: "Mycenae",
        monument: "Lion Gate",
        makes: O,
        a: &[
            stage(&[W, W], &[Gift::Points(3)]),
            stage(&[C, C, C], &[Gift::Shields(2)]),
            stage(&[O, O, O, O], &[Gift::Points(7)]),
        ],
        b: &[
            stage(
                &[S, S, S],
                &[Gift::Shields(1), Gift::Coins(3), Gift::Points(3)],
            ),
            stage(
                &[O, O, O, O],
                &[Gift::Shields(1), Gift::Coins(4), Gift::Points(4)],
            ),
        ],
    },
    Board {
        city: "Tyre",
        monument: "Temple of Melqart",
        makes: G,
        a: &[
            stage(&[S, S], &[Gift::Points(3)]),
            stage(&[O, O], &[Gift::AnyRaw]),
            stage(&[G, G], &[Gift::Points(7)]),
        ],
        b: &[
            stage(&[C, C], &[Gift::AnyRaw]),
            stage(&[W, W], &[Gift::AnyGoods]),
            stage(&[S, S, S], &[Gift::Points(7)]),
        ],
    },
    Board {
        city: "Carthage",
        monument: "The Cothon",
        makes: P,
        a: &[
            stage(&[S, S], &[Gift::Points(3)]),
            stage(&[W, W], &[Gift::Coins(9)]),
            stage(&[P, P], &[Gift::Points(7)]),
        ],
        b: &[
            stage(&[S, S], &[Gift::Points(2), Gift::Coins(4)]),
            stage(&[W, W], &[Gift::Points(3), Gift::Coins(4)]),
            stage(&[P, G, L], &[Gift::Points(5), Gift::Coins(4)]),
        ],
    },
    Board {
        city: "Ur",
        monument: "Great Ziggurat",
        makes: C,
        a: &[
            stage(&[C, C], &[Gift::Points(3)]),
            stage(&[W, W, W], &[Gift::Science]),
            stage(&[C, C, C, C], &[Gift::Points(7)]),
        ],
        b: &[
            stage(&[L, C], &[Gift::Points(3)]),
            stage(&[G, W, W], &[Gift::Seventh]),
            stage(&[C, C, C, P], &[Gift::Science]),
        ],
    },
    Board {
        city: "Knossos",
        monument: "Palace of Minos",
        makes: W,
        a: &[
            stage(&[W, W], &[Gift::Points(3)]),
            stage(&[S, S], &[Gift::FreeBuild]),
            stage(&[O, O], &[Gift::Points(7)]),
        ],
        b: &[
            stage(&[W, W], &[Gift::CheapRaw]),
            stage(&[S, S], &[Gift::Points(5)]),
            stage(&[L, O, O], &[Gift::CopyLeague]),
        ],
    },
    Board {
        city: "Petra",
        monument: "Rock Treasury",
        makes: L,
        a: &[
            stage(&[C, C], &[Gift::Points(3)]),
            stage(&[O, O, O], &[Gift::Salvage]),
            stage(&[L, L], &[Gift::Points(7)]),
        ],
        b: &[
            stage(&[O, O], &[Gift::Points(2), Gift::Salvage]),
            stage(&[C, C, C], &[Gift::Points(1), Gift::Salvage]),
            stage(&[G, P, L], &[Gift::Salvage]),
        ],
    },
    Board {
        city: "Persepolis",
        monument: "Gate of All Nations",
        makes: S,
        a: &[
            stage(&[S, S], &[Gift::Points(3)]),
            stage(&[W, W, W], &[Gift::Points(5)]),
            stage(&[S, S, S, S], &[Gift::Points(7)]),
        ],
        b: &[
            stage(&[W, W], &[Gift::Points(3)]),
            stage(&[S, S, S], &[Gift::Points(5)]),
            stage(&[C, C, C], &[Gift::Points(5)]),
            stage(&[S, S, S, S, P], &[Gift::Points(7)]),
        ],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    A,
    B,
}

/// A player's city: everything about it is public.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct City {
    pub board: u8,
    pub side: Side,
    /// Card ids, in the order built.
    pub built: Vec<u8>,
    /// Monument stages built.
    pub stages: u8,
    pub coins: i32,
    /// Military tokens: +1/+3/+5 victories, -1 defeats.
    pub tokens: Vec<i8>,
    /// The once-per-age free build has been used this age.
    pub power_used: bool,
}

impl City {
    pub fn new(board: u8, side: Side) -> Self {
        City {
            board,
            side,
            built: Vec::new(),
            stages: 0,
            coins: START_COINS,
            tokens: Vec::new(),
            power_used: false,
        }
    }

    pub fn board(&self) -> &'static Board {
        &BOARDS[self.board as usize]
    }

    pub fn stage_defs(&self) -> &'static [StageDef] {
        match self.side {
            Side::A => self.board().a,
            Side::B => self.board().b,
        }
    }

    /// Gifts from the stages built so far.
    pub fn gifts(&self) -> impl Iterator<Item = Gift> + '_ {
        self.stage_defs()
            .iter()
            .take(self.stages as usize)
            .flat_map(|s| s.gifts.iter().copied())
    }

    pub fn has_gift(&self, gift: Gift) -> bool {
        self.gifts().any(|g| g == gift)
    }

    pub fn cards(&self) -> impl Iterator<Item = &'static CardDef> + '_ {
        self.built.iter().map(|&id| def(id))
    }

    pub fn count(&self, color: Color) -> i32 {
        self.cards().filter(|c| c.color == color).count() as i32
    }

    pub fn owns(&self, name: &str) -> bool {
        self.cards().any(|c| c.name == name)
    }

    pub fn shields(&self) -> i32 {
        let cards: i32 = self
            .cards()
            .map(|c| match c.effect {
                Shields(n) => n as i32,
                _ => 0,
            })
            .sum();
        let gifts: i32 = self
            .gifts()
            .map(|g| match g {
                Gift::Shields(n) => n as i32,
                _ => 0,
            })
            .sum();
        cards + gifts
    }

    pub fn defeats(&self) -> i32 {
        self.tokens.iter().filter(|&&t| t < 0).count() as i32
    }

    /// Science symbols from cards, and wild symbols chosen at the end.
    pub fn science(&self) -> ([i32; 3], i32) {
        let mut counts = [0; 3];
        for c in self.cards() {
            if let Science(s) = c.effect {
                counts[s as usize] += 1;
            }
        }
        let wild = self.gifts().filter(|&g| g == Gift::Science).count() as i32
            + self
                .cards()
                .filter(|c| c.effect == Effect::League(League::Scholars))
                .count() as i32;
        (counts, wild)
    }

    pub fn has_discount(&self, d: Discount) -> bool {
        self.cards().any(|c| c.effect == Effect::Discount(d))
    }

    /// What the city makes each turn: one entry per unit, each a set of
    /// resources it may be (bit per `Res`). With `for_sale`, only what
    /// neighbors may buy: the city's own resource and brown and gray cards.
    pub fn producers(&self, for_sale: bool) -> Vec<u8> {
        let mut out = vec![mask(&[self.board().makes])];
        for c in self.cards() {
            if let Make(res, n) = c.effect {
                if for_sale && !matches!(c.color, Brown | Gray) {
                    continue;
                }
                for _ in 0..n {
                    out.push(mask(res));
                }
            }
        }
        if !for_sale {
            for g in self.gifts() {
                match g {
                    Gift::AnyRaw => out.push(mask(&Res::RAW)),
                    Gift::AnyGoods => out.push(mask(&Res::GOODS)),
                    _ => {}
                }
            }
        }
        out
    }

    /// What one unit of `res` costs from the neighbor on `left` or right.
    pub fn price(&self, res: Res, left: bool) -> u8 {
        let cheap = if res.is_raw() {
            self.has_gift(Gift::CheapRaw)
                || self.has_discount(if left {
                    Discount::RawLeft
                } else {
                    Discount::RawRight
                })
        } else {
            self.has_discount(Discount::Goods)
        };
        if cheap {
            1
        } else {
            2
        }
    }
}

pub fn mask(res: &[Res]) -> u8 {
    res.iter().fold(0, |m, r| m | (1 << r.idx()))
}

pub fn mask_res(mask: u8) -> Vec<Res> {
    Res::ALL
        .into_iter()
        .filter(|r| mask & (1 << r.idx()) != 0)
        .collect()
}

// ── Trading ───────────────────────────────────────────────────────────────────

/// Coins a build costs: to the left neighbor, the right neighbor and the bank.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pay {
    pub left: u8,
    pub right: u8,
    pub bank: u8,
}

impl Pay {
    pub fn total(&self) -> i32 {
        (self.left + self.right + self.bank) as i32
    }
}

type Need = [u8; 7];

/// Every cheapest way to cover `cost` as (coins to left, coins to right):
/// none of them can be beaten on both sides at once. Empty when the
/// resources can't be had at any price.
pub fn trade_options(cities: &[City], me: usize, cost: &[Res]) -> Vec<(u8, u8)> {
    let n = cities.len();
    let city = &cities[me];
    let mut need: Need = [0; 7];
    for r in cost {
        need[r.idx()] += 1;
    }
    // Sources: (resources it can be, price of each, bought from the left).
    // Our own cost nothing. Single resources of our own are used up front;
    // they never hurt.
    let mut sources: Vec<(u8, [u8; 7], bool)> = Vec::new();
    for m in city.producers(false) {
        if m.count_ones() == 1 {
            let i = m.trailing_zeros() as usize;
            if need[i] > 0 {
                need[i] -= 1;
                continue;
            }
        }
        sources.push((m, [0; 7], true));
    }
    if need.iter().all(|&x| x == 0) {
        return vec![(0, 0)];
    }
    for (left, nb) in [(true, left_of(n, me)), (false, right_of(n, me))] {
        let mut prices = [0u8; 7];
        for r in Res::ALL {
            prices[r.idx()] = city.price(r, left);
        }
        for m in cities[nb].producers(true) {
            sources.push((m, prices, left));
        }
    }
    // Sources of nothing we need only slow the search down.
    let wanted = need
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .fold(0u8, |m, (i, _)| m | (1 << i));
    sources.retain(|(m, _, _)| m & wanted != 0);
    let mut out = search(&sources, 0, need, &mut HashMap::new());
    out.sort_by_key(|&(l, r)| (l as u16 + r as u16, l));
    out
}

/// The cheapest ways to cover `need` with `sources[i..]`.
fn search(
    sources: &[(u8, [u8; 7], bool)],
    i: usize,
    need: Need,
    memo: &mut HashMap<(usize, Need), Vec<(u8, u8)>>,
) -> Vec<(u8, u8)> {
    let missing: u8 = need.iter().sum();
    if missing == 0 {
        return vec![(0, 0)];
    }
    if sources.len() - i < missing as usize {
        return Vec::new();
    }
    if let Some(v) = memo.get(&(i, need)) {
        return v.clone();
    }
    let (m, prices, left) = sources[i];
    let mut out = search(sources, i + 1, need, memo);
    for k in 0..7 {
        if m & (1 << k) == 0 || need[k] == 0 {
            continue;
        }
        let mut rest = need;
        rest[k] -= 1;
        for (l, r) in search(sources, i + 1, rest, memo) {
            out.push(if left {
                (l + prices[k], r)
            } else {
                (l, r + prices[k])
            });
        }
    }
    let out = pareto(out);
    memo.insert((i, need), out.clone());
    out
}

/// Drop options another one beats on both sides.
fn pareto(mut opts: Vec<(u8, u8)>) -> Vec<(u8, u8)> {
    opts.sort_unstable();
    opts.dedup();
    let all = opts.clone();
    opts.retain(|&(l, r)| {
        !all.iter()
            .any(|&(l2, r2)| (l2, r2) != (l, r) && l2 <= l && r2 <= r)
    });
    opts
}

/// What it takes to build something.
#[derive(Debug, Clone, PartialEq)]
pub enum Cost {
    /// Nothing to pay.
    Free,
    /// Free because the city already has the named building.
    Chain(&'static str),
    /// Each way the player can afford to pay.
    Pay(Vec<Pay>),
    Cannot(&'static str),
}

impl Cost {
    pub fn allows(&self, pay: &Pay) -> bool {
        match self {
            Cost::Free | Cost::Chain(_) => *pay == Pay::default(),
            Cost::Pay(ways) => ways.contains(pay),
            Cost::Cannot(_) => false,
        }
    }

    pub fn possible(&self) -> bool {
        !matches!(self, Cost::Cannot(_))
    }
}

fn cost_of(cities: &[City], me: usize, cost: &[Res], bank: u8) -> Cost {
    let coins = cities[me].coins;
    if cost.is_empty() && bank == 0 {
        return Cost::Free;
    }
    if bank as i32 > coins {
        return Cost::Cannot("Not enough coins.");
    }
    let opts = trade_options(cities, me, cost);
    if opts.is_empty() {
        return Cost::Cannot("Nobody nearby makes what it needs.");
    }
    let ways: Vec<Pay> = opts
        .into_iter()
        .map(|(left, right)| Pay { left, right, bank })
        .filter(|p| p.total() <= coins)
        .collect();
    if ways.is_empty() {
        return Cost::Cannot("Not enough coins to buy what it needs.");
    }
    Cost::Pay(ways)
}

/// What building card `id` would take.
pub fn build_cost(cities: &[City], me: usize, id: u8) -> Cost {
    let d = def(id);
    let city = &cities[me];
    if city.owns(d.name) {
        return Cost::Cannot("You already have one.");
    }
    if let Some(name) = d.free_with.iter().find(|n| city.owns(n)) {
        return Cost::Chain(name);
    }
    cost_of(cities, me, d.cost, d.coins)
}

/// What the next stage of the monument would take; `None` once finished.
pub fn stage_cost(cities: &[City], me: usize) -> Option<Cost> {
    let city = &cities[me];
    let next = city.stage_defs().get(city.stages as usize)?;
    Some(cost_of(cities, me, next.cost, 0))
}

/// Whether the once-per-age free build can build card `id`.
pub fn can_use_power(city: &City, id: u8) -> bool {
    city.has_gift(Gift::FreeBuild) && !city.power_used && !city.owns(def(id).name)
}

/// Whether `choice` is a legal move for player `me` right now. The card
/// must already be known to be in their hand.
pub fn check(cities: &[City], me: usize, choice: &Choice) -> Result<(), &'static str> {
    match choice.act {
        Act::Sell => Ok(()),
        Act::Power => {
            if can_use_power(&cities[me], choice.card) && choice.pay == Pay::default() {
                Ok(())
            } else {
                Err("You can't build that for free now.")
            }
        }
        Act::Build => match build_cost(cities, me, choice.card) {
            Cost::Cannot(why) => Err(why),
            c if c.allows(&choice.pay) => Ok(()),
            _ => Err("That payment doesn't work."),
        },
        Act::Stage => match stage_cost(cities, me) {
            None => Err("Your monument is finished."),
            Some(Cost::Cannot(why)) => Err(why),
            Some(c) if c.allows(&choice.pay) => Ok(()),
            _ => Err("That payment doesn't work."),
        },
    }
}

/// Coins a card pays out when built, counting what's in the cities now.
pub fn coins_when_built(cities: &[City], me: usize, d: &CardDef) -> i32 {
    let n = cities.len();
    let city = &cities[me];
    match d.effect {
        Effect::Coins(c) => c as i32,
        CoinsAround(color, k) => {
            let around = city.count(color)
                + cities[left_of(n, me)].count(color)
                + cities[right_of(n, me)].count(color);
            k as i32 * around
        }
        Bounty(Per::Cards(color), k, _) => k as i32 * city.count(color),
        Bounty(Per::Stages, k, _) => k as i32 * city.stages as i32,
        _ => 0,
    }
}

// ── Scoring ───────────────────────────────────────────────────────────────────

/// Points for science: each field squared, plus 7 per full set of three.
pub fn science_points(counts: [i32; 3], wild: i32) -> i32 {
    fn value(c: [i32; 3]) -> i32 {
        c.iter().map(|x| x * x).sum::<i32>() + 7 * c.iter().min().copied().unwrap_or(0)
    }
    if wild <= 0 {
        return value(counts);
    }
    Sci::ALL
        .iter()
        .map(|&s| {
            let mut c = counts;
            c[s as usize] += 1;
            science_points(c, wild - 1)
        })
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Breakdown {
    pub player: u8,
    pub military: i32,
    pub treasury: i32,
    pub monument: i32,
    pub civic: i32,
    pub science: i32,
    pub commerce: i32,
    pub leagues: i32,
    pub total: i32,
}

fn league_points(cities: &[City], me: usize, league: League) -> i32 {
    let n = cities.len();
    let (l, r) = (&cities[left_of(n, me)], &cities[right_of(n, me)]);
    let city = &cities[me];
    match league {
        League::Neighbors(color, k) => k as i32 * (l.count(color) + r.count(color)),
        League::Defeats => l.defeats() + r.defeats(),
        League::Shipwrights => city.count(Brown) + city.count(Gray) + city.count(Purple),
        League::Scholars => 0,
        League::Masons => (l.stages + city.stages + r.stages) as i32,
    }
}

/// Player `me`'s score if the game ended now. Wild science symbols and a
/// copied league are chosen to score the most.
pub fn score(cities: &[City], me: usize) -> Breakdown {
    let n = cities.len();
    let city = &cities[me];
    let mut b = Breakdown {
        player: me as u8,
        military: city.tokens.iter().map(|&t| t as i32).sum(),
        treasury: city.coins / 3,
        ..Breakdown::default()
    };
    for g in city.gifts() {
        if let Gift::Points(p) = g {
            b.monument += p as i32;
        }
    }
    let mut leagues = Vec::new();
    for c in city.cards() {
        match c.effect {
            Points(p) => b.civic += p as i32,
            Bounty(Per::Cards(color), _, p) => b.commerce += p as i32 * city.count(color),
            Bounty(Per::Stages, _, p) => b.commerce += p as i32 * city.stages as i32,
            Effect::League(league) => leagues.push(league),
            _ => {}
        }
    }
    let own_leagues: i32 = leagues
        .iter()
        .map(|&lg| league_points(cities, me, lg))
        .sum();
    let (counts, wild) = city.science();

    // A copied league from a neighbor, if the monument allows one.
    let mut copies: Vec<Option<League>> = vec![None];
    if city.has_gift(Gift::CopyLeague) {
        for nb in [left_of(n, me), right_of(n, me)] {
            for c in cities[nb].cards() {
                if let Effect::League(lg) = c.effect {
                    if !city.owns(c.name) {
                        copies.push(Some(lg));
                    }
                }
            }
        }
    }
    let (science, extra) = copies
        .into_iter()
        .map(|copy| match copy {
            None => (science_points(counts, wild), 0),
            Some(League::Scholars) => (science_points(counts, wild + 1), 0),
            Some(lg) => (science_points(counts, wild), league_points(cities, me, lg)),
        })
        .max_by_key(|(s, e)| s + e)
        .unwrap_or_default();
    b.science = science;
    b.leagues = own_leagues + extra;
    b.total = b.military + b.treasury + b.monument + b.civic + b.science + b.commerce + b.leagues;
    b
}

/// Tokens each player gets from the war at the end of `age`, by comparing
/// shields with both neighbors.
pub fn war(cities: &[City], age: u8) -> Vec<Vec<i8>> {
    let n = cities.len();
    let win = match age {
        1 => 1,
        2 => 3,
        _ => 5,
    };
    (0..n)
        .map(|p| {
            let mine = cities[p].shields();
            [left_of(n, p), right_of(n, p)]
                .into_iter()
                .filter_map(|nb| {
                    let theirs = cities[nb].shields();
                    if mine > theirs {
                        Some(win)
                    } else if mine < theirs {
                        Some(-1)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect()
}

/// Hands pass left in Ages I and III, right in Age II.
pub fn pass_dir(age: u8) -> Pass {
    if age == 2 {
        Pass::Right
    } else {
        Pass::Left
    }
}

pub fn age_name(age: u8) -> &'static str {
    match age {
        1 => "I",
        2 => "II",
        _ => "III",
    }
}

// ── Wire ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Sides {
    #[default]
    A,
    B,
    Random,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Options {
    #[serde(default)]
    pub sides: Sides,
}

impl Options {
    pub fn describe(&self) -> String {
        match self.sides {
            Sides::A => "A-side monuments".into(),
            Sides::B => "B-side monuments".into(),
            Sides::Random => "Random monument sides".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    /// Build the card.
    Build,
    /// Tuck the card under the monument to build its next stage.
    Stage,
    /// Discard it for 3 coins.
    Sell,
    /// Build it with the once-per-age free build.
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub card: u8,
    pub act: Act,
    pub pay: Pay,
}

/// What a player did on the latest turn, for everyone to see. A card used
/// for the monument or sold stays hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Done {
    Built(u8),
    /// Built with the free-build power.
    Free(u8),
    /// The stage number, 1-based.
    Stage(u8),
    Sold,
    /// Built from the discards.
    Salvaged(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reveal {
    pub done: Done,
    pub pay: Pay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    pub city: City,
    /// They've picked this turn's card.
    pub ready: bool,
    pub last: Option<Reveal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stage {
    Choosing,
    /// `player` plays the last card of the age too.
    Seventh {
        player: u8,
    },
    /// `player` builds a card from the discards for free.
    Salvage {
        player: u8,
    },
    Over {
        scores: Vec<Breakdown>,
        standings: Vec<Standing>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MoveKind {
    /// Someone locked in or took back a pick.
    Ready,
    /// The turn played out. `played` is the viewer's own card; `passed` the
    /// cards that left their hand, for `to` (or the discards with `None`);
    /// the new hand came from `from`, or from the deck with `dealt`.
    Turn {
        played: Option<(u8, Act)>,
        passed: Vec<u8>,
        to: Option<u8>,
        from: Option<u8>,
        dealt: bool,
    },
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Move {
    pub seq: u32,
    pub kind: MoveKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    pub age: u8,
    /// 1-based turn within the age.
    pub turn: u8,
    pub options: Options,
    pub players: Vec<PlayerView>,
    pub hand: Vec<u8>,
    /// Your pick this turn, until the turn plays out.
    pub choice: Option<Choice>,
    pub stage: Stage,
    /// The discard pile, only while you're salvaging from it.
    pub discard: Vec<u8>,
    pub log: Vec<String>,
    pub last: Option<Move>,
}

impl GameView {
    pub fn cities(&self) -> Vec<City> {
        self.players.iter().map(|p| p.city.clone()).collect()
    }

    pub fn name(&self, id: u8) -> String {
        self.players
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.nickname.clone())
            .unwrap_or_default()
    }

    pub fn left(&self) -> u8 {
        left_of(self.players.len().max(1), self.you as usize) as u8
    }

    pub fn right(&self) -> u8 {
        right_of(self.players.len().max(1), self.you as usize) as u8
    }
}

impl TableView for GameView {
    fn you(&self) -> u8 {
        self.you
    }

    fn last_move(&self) -> Option<(u32, bool)> {
        self.last.as_ref().map(|m| {
            let dealt = matches!(m.kind, MoveKind::Turn { dealt: true, .. });
            (m.seq, dealt)
        })
    }

    fn placeholder() -> Self {
        GameView {
            you: 0,
            host: 0,
            age: 1,
            turn: 1,
            options: Options::default(),
            players: Vec::new(),
            hand: Vec::new(),
            choice: None,
            stage: Stage::Choosing,
            discard: Vec::new(),
            log: Vec::new(),
            last: None,
        }
    }

    /// The hand you passed flies to your neighbor and the one you get flies
    /// in from the other side; a card you built slides into your city.
    fn motion(&self) -> Motion {
        let Some(Move {
            kind:
                MoveKind::Turn {
                    played,
                    passed,
                    to,
                    from,
                    dealt,
                },
            ..
        }) = &self.last
        else {
            return Motion::default();
        };
        let mut leave: Vec<(Spot, Option<u8>)> =
            passed.iter().map(|&id| (Spot::Card(id), *to)).collect();
        if let Some((id, act)) = played {
            if matches!(act, Act::Stage | Act::Sell) {
                leave.push((Spot::Card(*id), None));
            }
        }
        Motion {
            arrive_from: if *dealt {
                Some(Spot::Pile("deck"))
            } else {
                from.map(Spot::Seat)
            },
            deal: *dealt,
            leave,
        }
    }
}

pub type ServerMsg = crate::games::cards::ServerMsg<GameView>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    SetNickname {
        name: String,
    },
    StartGame,
    Choose {
        choice: Choice,
    },
    /// Take back this turn's pick.
    Unchoose,
    /// Build a card from the discards, or `None` to pass.
    Salvage {
        card: Option<u8>,
    },
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_of(name: &str) -> u8 {
        (0..deck().len() as u8)
            .find(|&id| def(id).name == name)
            .unwrap()
    }

    #[test]
    fn deck_sizes() {
        for players in MIN_PLAYERS..=MAX_PLAYERS {
            assert_eq!(
                age_cards(1, players).len(),
                players * HAND,
                "age I, {players}"
            );
            assert_eq!(
                age_cards(2, players).len(),
                players * HAND,
                "age II, {players}"
            );
            // Age III leaves room for players + 2 leagues.
            assert_eq!(age_cards(3, players).len() + players + 2, players * HAND);
        }
        assert_eq!(league_cards().len(), 10);
        assert!(deck().len() < 256);
    }

    #[test]
    fn chains_name_real_buildings() {
        for c in CARDS {
            for n in c.free_with {
                let from = def_named(n).unwrap_or_else(|| panic!("{} chains from {n}", c.name));
                let age = |d: &CardDef| d.copies.iter().map(|x| x.0).min().unwrap();
                assert!(age(from) < age(c), "{} chains from a later card", c.name);
            }
        }
        let mut names: Vec<_> = CARDS.iter().map(|c| c.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), CARDS.len());
    }

    #[test]
    fn science() {
        assert_eq!(science_points([3, 2, 1], 0), 9 + 4 + 1 + 7);
        assert_eq!(science_points([3, 2, 2], 0), 9 + 4 + 4 + 14);
        // A wild symbol finishes the second set.
        assert_eq!(science_points([3, 2, 1], 1), 31);
        assert_eq!(science_points([0, 0, 0], 2), 4);
    }

    fn table(n: usize) -> Vec<City> {
        (0..n).map(|i| City::new(i as u8, Side::A)).collect()
    }

    #[test]
    fn trading() {
        // Player 0 is Mycenae (ore); left is Persepolis (stone), right Tyre (glass).
        let mut cities = table(7);
        let me = 0;
        assert_eq!(cities[left_of(7, me)].board().makes, Res::Stone);
        assert_eq!(cities[right_of(7, me)].board().makes, Res::Glass);
        assert_eq!(trade_options(&cities, me, &[O]), vec![(0, 0)]);
        assert_eq!(trade_options(&cities, me, &[S]), vec![(2, 0)]);
        assert_eq!(trade_options(&cities, me, &[S, G, O]), vec![(2, 2)]);
        assert!(trade_options(&cities, me, &[S, S]).is_empty());
        // Both neighbors have wood: either one will do.
        cities[6].built.push(id_of("Woodcutter's Camp"));
        cities[1].built.push(id_of("Timber Mill"));
        assert_eq!(trade_options(&cities, me, &[W]), vec![(0, 2), (2, 0)]);
        assert_eq!(trade_options(&cities, me, &[W, W, W]), vec![(2, 4)]);
        // The Western Road makes the left neighbor's raw materials cheap.
        // Buying from the right is still offered: it pays a different player.
        cities[0].built.push(id_of("Western Road"));
        assert_eq!(trade_options(&cities, me, &[W]), vec![(1, 0), (0, 2)]);
        // Own choice cards are free and not for sale.
        cities[0].built.push(id_of("Caravan Yard"));
        assert_eq!(trade_options(&cities, me, &[W]), vec![(0, 0)]);
        assert!(cities[0].producers(true).len() < cities[0].producers(false).len());
    }

    #[test]
    fn costs() {
        let mut cities = table(3);
        let shrine = id_of("Shrine");
        let sanctuary = id_of("Sanctuary");
        assert_eq!(build_cost(&cities, 0, shrine), Cost::Free);
        cities[0].built.push(shrine);
        assert!(!build_cost(&cities, 0, shrine).possible());
        assert_eq!(build_cost(&cities, 0, sanctuary), Cost::Chain("Shrine"));
        // Orchard costs a coin; with none it can't be built.
        let orchard = id_of("Orchard");
        assert_eq!(
            build_cost(&cities, 1, orchard),
            Cost::Pay(vec![Pay {
                left: 0,
                right: 0,
                bank: 1
            }])
        );
        cities[1].coins = 0;
        assert!(!build_cost(&cities, 1, orchard).possible());
    }

    #[test]
    fn scoring() {
        let mut cities = table(3);
        // Knossos B copies the best league next door.
        cities[0] = City::new(4, Side::B);
        cities[0].stages = 3;
        cities[1].built.push(id_of("Scholars' League"));
        cities[1].built.push(id_of("Masons' League"));
        cities[0].built.push(id_of("Stargazer"));
        cities[0].built.push(id_of("Tinkerer"));
        let b = score(&cities, 0);
        // Copying the scholars completes a set: 1+1+1+7.
        assert_eq!(b.science, 10);
        assert_eq!(b.leagues, 0);
        assert_eq!(b.treasury, 1);
        assert_eq!(b.monument, 5);
    }

    #[test]
    fn war_tokens() {
        let mut cities = table(3);
        cities[0].built.push(id_of("Palisade"));
        let t = war(&cities, 2);
        assert_eq!(t[0], vec![3, 3]);
        assert_eq!(t[1], vec![-1]);
    }
}

// Jackson Coxson
// The numbered deck shared by Stack Attack and 9 Holes of Golf: twelve each
// of 1 to 12 plus eighteen wild cards, and how those cards look on the table.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::games::cards::HandCard;

/// Cards of each number in the deck.
pub const PER_RANK: usize = 12;
pub const WILDS: usize = 18;
/// 12 × 12 numbers plus 18 wilds.
pub const DECK_SIZE: usize = 12 * PER_RANK + WILDS;

/// One card. `rank` is 1 to 12, or 0 for a wild card. The `id` tells
/// identical cards apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub rank: u8,
}

impl Card {
    pub fn is_wild(&self) -> bool {
        self.rank == 0
    }

    /// "7", or "a Wild".
    pub fn label(&self) -> String {
        if self.is_wild() {
            "a Wild".into()
        } else {
            self.rank.to_string()
        }
    }
}

impl HandCard for Card {
    fn id(&self) -> u8 {
        self.id
    }
}

/// The whole deck in order, ids 0 to 161.
pub fn full_deck() -> Vec<Card> {
    let mut cards = Vec::with_capacity(DECK_SIZE);
    for rank in 1..=12 {
        for _ in 0..PER_RANK {
            cards.push(Card {
                id: cards.len() as u8,
                rank,
            });
        }
    }
    for _ in 0..WILDS {
        cards.push(Card {
            id: cards.len() as u8,
            rank: 0,
        });
    }
    cards
}

// ── Looks ─────────────────────────────────────────────────────────────────────

/// Blue for 1–4, green for 5–8, red for 9–12, amber for wilds.
fn colors(card: Card) -> &'static str {
    match card.rank {
        0 => "bg-gradient-to-br from-amber-300 to-orange-500 text-white ring-1 ring-orange-600",
        1..=4 => "bg-white text-blue-600 ring-1 ring-stone-300",
        5..=8 => "bg-white text-green-600 ring-1 ring-stone-300",
        _ => "bg-white text-red-600 ring-1 ring-stone-300",
    }
}

/// A face-up card. Sized by the surrounding `--cw`; add `extra` classes for
/// rings and the like.
pub fn card_face(card: Card, extra: &'static str) -> impl IntoView {
    let corner = if card.is_wild() {
        "★".to_string()
    } else {
        card.rank.to_string()
    };
    let middle = corner.clone();
    view! {
        <div class=format!(
            "card-face num-card {} {extra} relative select-none overflow-hidden rounded-lg font-bold shadow-md",
            colors(card),
        )>
            <span class="num-card-corner absolute left-1 top-0.5 leading-none">{corner}</span>
            <span class="absolute inset-0 flex items-center justify-center leading-none">
                <span class="num-card-middle">{middle}</span>
            </span>
            {card
                .is_wild()
                .then(|| {
                    view! {
                        <span class="absolute inset-x-0 bottom-0 bg-orange-600 text-center text-[0.5rem] font-bold leading-tight text-white">
                            "WILD"
                        </span>
                    }
                })}
        </div>
    }
}

/// The back of a card, with an optional caption such as "12 left".
pub fn card_back(label: String) -> impl IntoView {
    view! {
        <div class="card-face flex select-none flex-col items-center justify-center rounded-lg bg-gradient-to-br from-sky-700 to-indigo-900 text-amber-300 shadow-md ring-1 ring-sky-400">
            <span class="num-card-middle leading-none">"✦"</span>
            {(!label.is_empty())
                .then(|| {
                    view! {
                        <span class="mt-0.5 text-[0.6rem] font-semibold text-sky-100">{label}</span>
                    }
                })}
        </div>
    }
}

/// An empty spot where a card could go.
pub fn card_slot(label: &'static str) -> impl IntoView {
    view! {
        <div class="card-face flex select-none items-center justify-center rounded-lg border-2 border-dashed border-green-700/80 text-center text-[0.6rem] text-green-200/70">
            {label}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_has_162_cards() {
        let deck = full_deck();
        assert_eq!(deck.len(), 162);
        assert_eq!(deck.iter().filter(|c| c.is_wild()).count(), 18);
        for rank in 1..=12 {
            assert_eq!(deck.iter().filter(|c| c.rank == rank).count(), 12);
        }
        assert!(deck.iter().enumerate().all(|(i, c)| c.id as usize == i));
    }
}

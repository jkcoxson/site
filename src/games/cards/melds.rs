// Jackson Coxson
// Sets and runs with wild cards, shared by the rummy-style games (Five
// Crowns, Ten Stages). Games map their cards to a `Natural` first, so the
// rules here don't care what a card looks like or which cards are wild.

use std::ops::RangeInclusive;

/// A card as melds see it: its rank and suit (or color), or `None` for a
/// wild card that can stand in for anything.
pub type Natural = Option<(u8, u8)>;

/// The rank shared by every natural card, if they all share one. All wilds
/// give `Some(None)`.
pub fn set_rank(cards: &[Natural]) -> Option<Option<u8>> {
    let mut rank = None;
    for &(r, _) in cards.iter().flatten() {
        match rank {
            None => rank = Some(r),
            Some(seen) if seen != r => return None,
            _ => {}
        }
    }
    Some(rank)
}

/// The suit shared by every natural card, if they all share one. All wilds
/// give `Some(None)`.
pub fn suit_of(cards: &[Natural]) -> Option<Option<u8>> {
    let mut suit = None;
    for &(_, s) in cards.iter().flatten() {
        match suit {
            None => suit = Some(s),
            Some(seen) if seen != s => return None,
            _ => {}
        }
    }
    Some(suit)
}

/// Where the run `cards` starts if they make one: every card a step up from
/// the last, inside `ranks`, with wilds filling gaps. Wilds left over go
/// above the naturals while there's room, then below. With `same_suit` the
/// naturals must share a suit.
pub fn run_start(cards: &[Natural], ranks: RangeInclusive<u8>, same_suit: bool) -> Option<u8> {
    let len = cards.len();
    let (lowest, highest) = (*ranks.start(), *ranks.end());
    if len == 0 || len > (highest - lowest) as usize + 1 {
        return None;
    }
    if same_suit && suit_of(cards).is_none() {
        return None;
    }
    let mut naturals: Vec<u8> = cards.iter().flatten().map(|&(r, _)| r).collect();
    naturals.sort_unstable();
    if naturals.windows(2).any(|w| w[0] == w[1]) {
        return None;
    }
    let (Some(&min), Some(&max)) = (naturals.first(), naturals.last()) else {
        return Some(lowest);
    };
    if min < lowest || max > highest || (max - min) as usize + 1 > len {
        return None;
    }
    Some(min.min(highest + 1 - len as u8))
}

/// Every subset of `n` items holding exactly `k` of them, as bit masks.
pub fn masks_of_size(n: usize, k: usize) -> impl Iterator<Item = u32> {
    let full: u32 = if n >= 32 { u32::MAX } else { (1 << n) - 1 };
    (0..=full).filter(move |m| m.count_ones() as usize == k)
}

/// The items of `all` picked out by `mask`.
pub fn pick<T: Copy>(all: &[T], mask: u32) -> Vec<T> {
    all.iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, &c)| c)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(rank: u8, suit: u8) -> Natural {
        Some((rank, suit))
    }

    #[test]
    fn sets() {
        assert_eq!(set_rank(&[n(7, 0), n(7, 1), None]), Some(Some(7)));
        assert_eq!(set_rank(&[None, None]), Some(None));
        assert_eq!(set_rank(&[n(7, 0), n(8, 0)]), None);
    }

    #[test]
    fn runs_place_wilds_above_then_below() {
        assert_eq!(run_start(&[n(3, 0), n(4, 1), None], 1..=12, false), Some(3));
        assert_eq!(
            run_start(&[n(11, 0), n(12, 1), None], 1..=12, false),
            Some(10)
        );
        assert_eq!(run_start(&[n(3, 0), None, n(5, 1)], 1..=12, false), Some(3));
        assert_eq!(run_start(&[n(3, 0), n(6, 1), None], 1..=12, false), None);
        assert_eq!(run_start(&[n(3, 0), n(3, 1), None], 1..=12, false), None);
        assert_eq!(run_start(&[n(3, 0), n(4, 1), n(5, 0)], 1..=12, true), None);
        assert_eq!(run_start(&[None; 13], 1..=12, false), None);
        assert_eq!(run_start(&[None; 3], 1..=12, false), Some(1));
    }

    #[test]
    fn subsets() {
        assert_eq!(masks_of_size(5, 2).count(), 10);
        assert_eq!(pick(&[1, 2, 3], 0b101), vec![1, 3]);
    }
}

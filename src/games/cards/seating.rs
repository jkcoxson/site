// Jackson Coxson
// Who sits next to whom around a card table, and passing hands around it.
// Shared by the server and the browser, so both agree on who "left" is.
//
// Players sit in id order around the table. Your left neighbor is the
// player before you and your right neighbor the one after, wrapping around,
// so on a row of seats drawn in id order your neighbors are on the matching
// sides (except at the ends).

use serde::{Deserialize, Serialize};

/// The player on `player`'s left at a table of `players`.
pub fn left_of(players: usize, player: usize) -> usize {
    (player + players - 1) % players
}

/// The player on `player`'s right at a table of `players`.
pub fn right_of(players: usize, player: usize) -> usize {
    (player + 1) % players
}

/// A direction to pass cards in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pass {
    Left,
    Right,
}

impl Pass {
    /// Who `player` passes to.
    pub fn to(self, players: usize, player: usize) -> usize {
        match self {
            Pass::Left => left_of(players, player),
            Pass::Right => right_of(players, player),
        }
    }

    /// Who passes to `player`.
    pub fn from(self, players: usize, player: usize) -> usize {
        match self {
            Pass::Left => right_of(players, player),
            Pass::Right => left_of(players, player),
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            Pass::Left => "◂",
            Pass::Right => "▸",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Pass::Left => "left",
            Pass::Right => "right",
        }
    }
}

/// Everyone hands their whole hand to the neighbor in `dir` (a draft).
pub fn pass_hands<T>(hands: &mut Vec<Vec<T>>, dir: Pass) {
    let n = hands.len();
    if n < 2 {
        return;
    }
    let mut old: Vec<Option<Vec<T>>> = hands.drain(..).map(Some).collect();
    let mut new: Vec<Vec<T>> = (0..n).map(|_| Vec::new()).collect();
    for p in 0..n {
        new[dir.to(n, p)] = old[p].take().unwrap_or_default();
    }
    *hands = new;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_wrap() {
        assert_eq!(left_of(4, 0), 3);
        assert_eq!(right_of(4, 3), 0);
        assert_eq!(left_of(4, 2), 1);
        for n in 3..8 {
            for p in 0..n {
                assert_eq!(Pass::Left.from(n, Pass::Left.to(n, p)), p);
                assert_eq!(Pass::Right.from(n, Pass::Right.to(n, p)), p);
            }
        }
    }

    #[test]
    fn hands_go_to_the_neighbor() {
        let mut hands = vec![vec![0], vec![1], vec![2]];
        pass_hands(&mut hands, Pass::Left);
        // Player 1 passed left to player 0.
        assert_eq!(hands, vec![vec![1], vec![2], vec![0]]);
        pass_hands(&mut hands, Pass::Right);
        assert_eq!(hands, vec![vec![0], vec![1], vec![2]]);
    }
}

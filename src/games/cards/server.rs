// Jackson Coxson
// Server helpers shared by the card games.

use serde::Serialize;

use super::{HandCard, LobbyInfo, SeatInfo, ServerMsg, Standing};
use crate::games::rooms::Seats;

/// A fresh six-character room code.
pub fn room_code() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let chars: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    (0..6)
        .map(|_| chars[rng.random_range(0..chars.len())] as char)
        .collect()
}

pub fn shuffle<T>(items: &mut [T]) {
    use rand::RngExt;
    let mut rng = rand::rng();
    for i in (1..items.len()).rev() {
        let j = rng.random_range(0..=i);
        items.swap(i, j);
    }
}

/// The top card of the draw pile. When it runs out, everything but the top
/// discard is shuffled back in first.
pub fn draw_from_deck<C>(deck: &mut Vec<C>, discard: &mut Vec<C>) -> Option<C> {
    if deck.is_empty() {
        let top = discard.pop();
        deck.append(discard);
        shuffle(deck);
        discard.extend(top);
    }
    deck.pop()
}

/// Put `hand` in the order of `order`, which must list every card id in the
/// hand exactly once. Returns whether it did.
pub fn arrange_hand<C: HandCard>(hand: &mut [C], order: &[u8]) -> bool {
    let mut wanted = order.to_vec();
    wanted.sort_unstable();
    let mut ids: Vec<u8> = hand.iter().map(|c| c.id()).collect();
    ids.sort_unstable();
    if wanted != ids {
        return false;
    }
    hand.sort_by_key(|c| order.iter().position(|&id| id == c.id()));
    true
}

pub fn name(seats: &Seats, player: usize) -> String {
    seats
        .get(player)
        .map(|s| s.nickname.clone())
        .unwrap_or_default()
}

/// Send everyone the waiting room. The first seat is the host.
pub fn broadcast_lobby<V: Serialize>(seats: &Seats, about: &str) {
    let players: Vec<SeatInfo> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| SeatInfo {
            id: i as u8,
            nickname: s.nickname.clone(),
            connected: s.connected,
        })
        .collect();
    for p in 0..seats.len() {
        seats.send(
            p,
            &ServerMsg::<V>::Lobby(LobbyInfo {
                players: players.clone(),
                host: 0,
                you: p as u8,
                about: about.to_string(),
            }),
        );
    }
}

pub fn error(seats: &Seats, player: usize, msg: &str) {
    seats.send(player, &ServerMsg::<()>::Error(msg.into()));
}

/// Places for final totals; ties share a place. With `low_wins` the smallest
/// total comes first.
pub fn standings(totals: &[i32], low_wins: bool) -> Vec<Standing> {
    totals
        .iter()
        .enumerate()
        .map(|(i, &total)| Standing {
            player: i as u8,
            total,
            place: 1 + totals
                .iter()
                .filter(|&&t| if low_wins { t < total } else { t > total })
                .count() as u8,
        })
        .collect()
}

/// A short recent-events log for table talk.
#[derive(Default)]
pub struct Log {
    lines: Vec<String>,
}

impl Log {
    const LEN: usize = 8;

    pub fn note(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > Self::LEN {
            self.lines.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn lines(&self) -> Vec<String> {
        self.lines.clone()
    }
}

/// Places for final totals with a tiebreak: each entry is (total, tiebreak),
/// higher is better for both, and players tied on both share a place.
pub fn standings_with_tiebreak(scores: &[(i32, i32)]) -> Vec<Standing> {
    scores
        .iter()
        .enumerate()
        .map(|(i, &mine)| Standing {
            player: i as u8,
            total: mine.0,
            place: 1 + scores.iter().filter(|&&s| s > mine).count() as u8,
        })
        .collect()
}

/// Choices everyone makes at once and keeps hidden until all are in (a
/// draft pick, a bid). A player may change or take back their pick until
/// the last one arrives.
pub struct Picks<T> {
    picks: Vec<Option<T>>,
}

impl<T> Picks<T> {
    pub fn new(players: usize) -> Self {
        Self {
            picks: (0..players).map(|_| None).collect(),
        }
    }

    pub fn set(&mut self, player: usize, pick: T) {
        if let Some(slot) = self.picks.get_mut(player) {
            *slot = Some(pick);
        }
    }

    pub fn clear(&mut self, player: usize) {
        if let Some(slot) = self.picks.get_mut(player) {
            *slot = None;
        }
    }

    pub fn get(&self, player: usize) -> Option<&T> {
        self.picks.get(player).and_then(|p| p.as_ref())
    }

    pub fn is_in(&self, player: usize) -> bool {
        self.get(player).is_some()
    }

    pub fn all_in(&self) -> bool {
        self.picks.iter().all(|p| p.is_some())
    }

    /// Everyone's pick in player order, leaving the slots empty for the
    /// next round. `None` unless everyone is in.
    pub fn take_all(&mut self) -> Option<Vec<T>> {
        if !self.all_in() {
            return None;
        }
        Some(self.picks.iter_mut().filter_map(|p| p.take()).collect())
    }

    pub fn reset(&mut self) {
        for p in &mut self.picks {
            *p = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiebreak_places() {
        let s = standings_with_tiebreak(&[(50, 3), (50, 7), (40, 9), (50, 7)]);
        let places: Vec<u8> = s.iter().map(|s| s.place).collect();
        assert_eq!(places, vec![3, 1, 4, 1]);
    }

    #[test]
    fn picks_wait_for_everyone() {
        let mut p = Picks::new(2);
        p.set(0, 'a');
        assert!(p.take_all().is_none());
        p.set(1, 'b');
        p.set(0, 'c');
        assert_eq!(p.take_all(), Some(vec!['c', 'b']));
        assert!(!p.is_in(0));
    }
}

// Jackson Coxson
// Server helpers shared by the card games.

use serde::Serialize;

use super::{LobbyInfo, SeatInfo, ServerMsg, Standing};
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

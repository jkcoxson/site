// Jackson Coxson

use std::collections::HashSet;

use crate::games::light_beams::logic::{ArrowDir, Board, Color, Piece, SlashDir};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn tag(self) -> u8 {
        match self {
            Dir::Up => 0,
            Dir::Down => 1,
            Dir::Left => 2,
            Dir::Right => 3,
        }
    }
}

fn opposite(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::Down,
        Dir::Down => Dir::Up,
        Dir::Left => Dir::Right,
        Dir::Right => Dir::Left,
    }
}

#[derive(Default)]
struct Light {
    red: bool,
    yellow: bool,
    blue: bool,
    absorb: bool,
}

impl Light {
    fn hit(&mut self, piece: Piece) {
        match piece {
            Piece::Red => self.red = true,
            Piece::Yellow => self.yellow = true,
            Piece::Blue => self.blue = true,
            Piece::Black => self.absorb = true,
            _ => {}
        }
    }

    fn color(self) -> Color {
        if self.absorb {
            return Color::Black;
        }
        match (self.red, self.yellow, self.blue) {
            (false, false, false) => Color::White,
            (true, false, false) => Color::Red,
            (false, true, false) => Color::Yellow,
            (false, false, true) => Color::Blue,
            (true, true, false) => Color::Orange,
            (true, false, true) => Color::Purple,
            (false, true, true) => Color::Green,
            (true, true, true) => Color::Brown,
        }
    }
}

fn port_entry(size: i32, port: i32) -> (i32, i32, Dir) {
    let n = size;
    if (1..=n).contains(&port) {
        // Top edge, entering downward.
        (0, port - 1, Dir::Down)
    } else if (n + 1..=2 * n).contains(&port) {
        // Right edge, entering leftward.
        (port - n - 1, n - 1, Dir::Left)
    } else if (2 * n + 1..=3 * n).contains(&port) {
        // Bottom edge, entering upward.
        (n - 1, 3 * n - port, Dir::Up)
    } else {
        // Left edge, entering rightward.
        (4 * n - port, 0, Dir::Right)
    }
}

fn exit_port(size: i32, row: i32, col: i32, dir: Dir) -> u8 {
    let n = size;
    let p = match dir {
        Dir::Up => col + 1,
        Dir::Right => n + 1 + row,
        Dir::Down => 3 * n - col,
        Dir::Left => 4 * n - row,
    };
    p as u8
}

pub fn trace_beam(board: &Board, port: u8) -> (u8, Color) {
    let size = board.size as i32;
    let (mut row, mut col, mut dir) = port_entry(size, port as i32);
    let mut light = Light::default();

    // Any path longer than the number of (cell, direction) states must revisit
    // one, so we detect a loop the moment a state repeats.
    let mut visited: HashSet<(i32, i32, u8)> = HashSet::new();

    loop {
        if !(0..size).contains(&row) || !(0..size).contains(&col) {
            return (exit_port(size, row, col, dir), light.color());
        }

        if !visited.insert((row, col, dir.tag())) {
            // Already been here heading the same way, the beam is trapped.
            return (0, light.color());
        }

        if let Some(piece) = board.get(row as usize, col as usize) {
            match piece {
                Piece::Slash(slash_dir) => {
                    dir = match slash_dir {
                        SlashDir::Forward => match dir {
                            Dir::Right => Dir::Up,
                            Dir::Up => Dir::Right,
                            Dir::Left => Dir::Down,
                            Dir::Down => Dir::Left,
                        },
                        SlashDir::Backward => match dir {
                            Dir::Right => Dir::Down,
                            Dir::Down => Dir::Right,
                            Dir::Left => Dir::Up,
                            Dir::Up => Dir::Left,
                        },
                    };
                }
                Piece::Arrow(arrow_dir) => {
                    let reflects = match arrow_dir {
                        ArrowDir::UpRight => dir == Dir::Down || dir == Dir::Left,
                        ArrowDir::DownRight => dir == Dir::Up || dir == Dir::Left,
                        ArrowDir::DownLeft => dir == Dir::Up || dir == Dir::Right,
                        ArrowDir::UpLeft => dir == Dir::Down || dir == Dir::Right,
                    };
                    if reflects {
                        dir = opposite(dir);
                    }
                }
                Piece::FourArrow => {
                    dir = opposite(dir);
                }
                other => light.hit(other),
            }
        }

        match dir {
            Dir::Up => row -= 1,
            Dir::Down => row += 1,
            Dir::Left => col -= 1,
            Dir::Right => col += 1,
        }
    }
}

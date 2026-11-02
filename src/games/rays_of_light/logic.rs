// Jackson Coxson

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// A corner piece made of two arrows meeting at a right angle. Each arrow
/// points toward one of the two named directions.
pub enum ArrowDir {
    UpRight,
    DownRight,
    DownLeft,
    UpLeft,
}

impl ArrowDir {
    pub fn symbol(self) -> &'static str {
        match self {
            ArrowDir::UpRight => "↑→",
            ArrowDir::DownRight => "↓→",
            ArrowDir::DownLeft => "↓←",
            ArrowDir::UpLeft => "↑←",
        }
    }
}

/// Orientation of a slash mirror.
/// Forward = `/`  (right→up, up→right, left→down, down→left)
/// Backward = `\` (right→down, down→right, left→up, up→left)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashDir {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Piece {
    Red,
    Yellow,
    Blue,
    Black,
    Slash(SlashDir),
    Arrow(ArrowDir),
}

impl Piece {
    pub fn symbol(self) -> &'static str {
        match self {
            Piece::Red => "R",
            Piece::Yellow => "Y",
            Piece::Blue => "B",
            Piece::Black => "█",
            Piece::Slash(SlashDir::Forward) => "/",
            Piece::Slash(SlashDir::Backward) => "\\",
            Piece::Arrow(d) => d.symbol(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Piece::Red => "Red",
            Piece::Yellow => "Yellow",
            Piece::Blue => "Blue",
            Piece::Black => "Black",
            Piece::Slash(SlashDir::Forward) => "Mirror /",
            Piece::Slash(SlashDir::Backward) => "Mirror \\",
            Piece::Arrow(ArrowDir::UpRight) => "Arrow ↑→",
            Piece::Arrow(ArrowDir::DownRight) => "Arrow ↓→",
            Piece::Arrow(ArrowDir::DownLeft) => "Arrow ↓←",
            Piece::Arrow(ArrowDir::UpLeft) => "Arrow ↑←",
        }
    }

    pub fn css_color(self) -> &'static str {
        match self {
            Piece::Red => "bg-red-500 text-white",
            Piece::Yellow => "bg-yellow-400 text-black",
            Piece::Blue => "bg-blue-500 text-white",
            Piece::Black => "bg-stone-900 text-white",
            Piece::Slash(_) => "bg-slate-400 dark:bg-slate-600 text-black dark:text-white",
            Piece::Arrow(_) => "bg-slate-400 dark:bg-slate-600 text-black dark:text-white",
        }
    }
}

/// Returns true when `a` and `b` are the same piece variant (ignoring inner direction).
pub fn same_piece_type(a: Piece, b: Piece) -> bool {
    match (a, b) {
        (Piece::Arrow(_), Piece::Arrow(_)) => true,
        (Piece::Slash(_), Piece::Slash(_)) => true,
        _ => a == b,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    White,
    Red,
    Yellow,
    Blue,
    Orange,
    Purple,
    Green,
    Brown,
    Black,
}

impl Color {
    pub fn name(self) -> &'static str {
        match self {
            Color::White => "White",
            Color::Red => "Red",
            Color::Yellow => "Yellow",
            Color::Blue => "Blue",
            Color::Orange => "Orange",
            Color::Purple => "Purple",
            Color::Green => "Green",
            Color::Brown => "Brown",
            Color::Black => "Black",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Color::White => "bg-white border border-gray-400 text-gray-700",
            Color::Red => "bg-red-500 text-white",
            Color::Yellow => "bg-yellow-400 text-black",
            Color::Blue => "bg-blue-500 text-white",
            Color::Orange => "bg-orange-500 text-white",
            Color::Purple => "bg-purple-500 text-white",
            Color::Green => "bg-green-500 text-white",
            Color::Brown => "bg-amber-800 text-white",
            Color::Black => "bg-gray-900 text-white",
        }
    }

    /// A concrete CSS color for this ray, used to tint the glow of a breathing
    /// port. Black is lightened to a slate so the glow stays visible against the
    /// dark board.
    pub fn glow(self) -> &'static str {
        match self {
            Color::White => "#e5e7eb",
            Color::Red => "#ef4444",
            Color::Yellow => "#facc15",
            Color::Blue => "#3b82f6",
            Color::Orange => "#f97316",
            Color::Purple => "#a855f7",
            Color::Green => "#22c55e",
            Color::Brown => "#92400e",
            Color::Black => "#64748b",
        }
    }
}

pub type Board = [[Option<Piece>; 4]; 4];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

fn opposite(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::Down,
        Dir::Down => Dir::Up,
        Dir::Left => Dir::Right,
        Dir::Right => Dir::Left,
    }
}

struct Light {
    red: bool,
    yellow: bool,
    blue: bool,
    absorb: bool,
}

impl Default for Light {
    fn default() -> Self {
        Self { red: false, yellow: false, blue: false, absorb: false }
    }
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
        // Only the black absorber piece yields true black; mixing all three
        // colored glasses makes brown instead.
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

fn port_entry(port: u8) -> (i32, i32, Dir) {
    match port {
        1 => (0, 0, Dir::Down),
        2 => (0, 1, Dir::Down),
        3 => (0, 2, Dir::Down),
        4 => (0, 3, Dir::Down),
        5 => (0, 3, Dir::Left),
        6 => (1, 3, Dir::Left),
        7 => (2, 3, Dir::Left),
        8 => (3, 3, Dir::Left),
        9 => (3, 3, Dir::Up),
        10 => (3, 2, Dir::Up),
        11 => (3, 1, Dir::Up),
        12 => (3, 0, Dir::Up),
        13 => (3, 0, Dir::Right),
        14 => (2, 0, Dir::Right),
        15 => (1, 0, Dir::Right),
        16 => (0, 0, Dir::Right),
        _ => (0, 0, Dir::Down),
    }
}

fn exit_port(row: i32, col: i32, dir: Dir) -> u8 {
    match dir {
        Dir::Down => [12u8, 11, 10, 9].get(col as usize).copied().unwrap_or(0),
        Dir::Up => [1u8, 2, 3, 4].get(col as usize).copied().unwrap_or(0),
        Dir::Right => [5u8, 6, 7, 8].get(row as usize).copied().unwrap_or(0),
        Dir::Left => [16u8, 15, 14, 13].get(row as usize).copied().unwrap_or(0),
    }
}

pub fn trace_ray(board: &Board, port: u8) -> (u8, Color) {
    let (mut row, mut col, mut dir) = port_entry(port);
    let mut light = Light::default();

    for _ in 0..128 {
        if !(0..4).contains(&row) || !(0..4).contains(&col) {
            return (exit_port(row, col, dir), light.color());
        }

        if let Some(piece) = board[row as usize][col as usize] {
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
                    // Two arrows meeting at a right angle. A ray that hits an
                    // arrow head-on (travelling opposite to where it points)
                    // bounces straight back the way it came; rays arriving
                    // from the other two directions pass through.
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

    (0, Color::White)
}

/// Generate a random, valid board: one of each colored glass, the black piece,
/// an arrow mirror (random orientation) and two slash mirrors (random
/// orientations), scattered across seven distinct cells. Used for single player
/// mode, where the player deduces this layout by shooting rays.
pub fn random_board() -> Board {
    use rand::RngExt;
    let mut rng = rand::rng();

    let slash = |rng: &mut _| {
        if rand::RngExt::random_range(rng, 0..2) == 0 {
            Piece::Slash(SlashDir::Forward)
        } else {
            Piece::Slash(SlashDir::Backward)
        }
    };
    let arrow = match rng.random_range(0..4) {
        0 => Piece::Arrow(ArrowDir::UpRight),
        1 => Piece::Arrow(ArrowDir::DownRight),
        2 => Piece::Arrow(ArrowDir::DownLeft),
        _ => Piece::Arrow(ArrowDir::UpLeft),
    };
    let pieces = [
        Piece::Red,
        Piece::Yellow,
        Piece::Blue,
        Piece::Black,
        slash(&mut rng),
        slash(&mut rng),
        arrow,
    ];

    // Fisher–Yates shuffle of the 16 cells, then take the first seven.
    let mut cells: Vec<(usize, usize)> =
        (0..4).flat_map(|r| (0..4).map(move |c| (r, c))).collect();
    for i in (1..cells.len()).rev() {
        let j = rng.random_range(0..(i + 1));
        cells.swap(i, j);
    }

    let mut board: Board = Default::default();
    for (piece, &(r, c)) in pieces.iter().zip(cells.iter()) {
        board[r][c] = Some(*piece);
    }
    board
}

pub fn validate_board(board: &Board) -> bool {
    let mut counts = [0u8; 6];
    for row in board.iter() {
        for cell in row.iter() {
            match cell {
                Some(Piece::Red) => counts[0] += 1,
                Some(Piece::Yellow) => counts[1] += 1,
                Some(Piece::Blue) => counts[2] += 1,
                Some(Piece::Black) => counts[3] += 1,
                Some(Piece::Slash(_)) => counts[4] += 1,
                Some(Piece::Arrow(_)) => counts[5] += 1,
                None => {}
            }
        }
    }
    // One of each piece, except the slash mirror, which the player gets two of.
    let [red, yellow, blue, black, slash, arrow] = counts;
    red == 1 && yellow == 1 && blue == 1 && black == 1 && slash == 2 && arrow == 1
}

// ── Reconnect resync types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResyncShot {
    pub mine: bool,
    pub call: u8,
    pub exit: u8,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ResyncPhase {
    WaitingForOpponent,
    Setup { your_board: Option<Board>, you_locked: bool, opponent_locked: bool },
    Playing {
        your_board: Board,
        shots: Vec<ResyncShot>,
        your_turn: bool,
        /// Whether the player has already used their one flip this turn.
        you_rotated: bool,
    },
    Over {
        you_win: bool,
        your_board: Board,
        /// The opponent's actual board — the answer the player was guessing.
        opponent_board: Board,
        shots: Vec<ResyncShot>,
    },
}

/// Only the slash mirror may be rotated during play. The arrow mirror's
/// direction is fixed by its owner during board setup.
pub fn is_slash(piece: Piece) -> bool {
    matches!(piece, Piece::Slash(_))
}

pub fn rotate_piece(piece: Piece) -> Piece {
    match piece {
        Piece::Arrow(ArrowDir::UpRight) => Piece::Arrow(ArrowDir::DownRight),
        Piece::Arrow(ArrowDir::DownRight) => Piece::Arrow(ArrowDir::DownLeft),
        Piece::Arrow(ArrowDir::DownLeft) => Piece::Arrow(ArrowDir::UpLeft),
        Piece::Arrow(ArrowDir::UpLeft) => Piece::Arrow(ArrowDir::UpRight),
        Piece::Slash(SlashDir::Forward) => Piece::Slash(SlashDir::Backward),
        Piece::Slash(SlashDir::Backward) => Piece::Slash(SlashDir::Forward),
        other => other,
    }
}

pub fn boards_match(a: &Board, b: &Board) -> bool {
    for r in 0..4 {
        for c in 0..4 {
            if a[r][c] != b[r][c] {
                return false;
            }
        }
    }
    true
}

// ── Share codes ─────────────────────────────────────────────────────────────
//
// A board is encoded into a short, opaque string for shareable single player URLs.
// Each of the 16 cells holds one of 11 values (empty or one of ten pieces), so
// it fits in a nibble; the 16 nibbles pack into 8 bytes, which are XOR-scrambled
// with a fixed key and base64url-encoded. The scramble is *obfuscation, not
// security* — it just stops the answer from being legible at a glance in the URL.

fn cell_code(cell: Option<Piece>) -> u8 {
    match cell {
        None => 0,
        Some(Piece::Red) => 1,
        Some(Piece::Yellow) => 2,
        Some(Piece::Blue) => 3,
        Some(Piece::Black) => 4,
        Some(Piece::Slash(SlashDir::Forward)) => 5,
        Some(Piece::Slash(SlashDir::Backward)) => 6,
        Some(Piece::Arrow(ArrowDir::UpRight)) => 7,
        Some(Piece::Arrow(ArrowDir::DownRight)) => 8,
        Some(Piece::Arrow(ArrowDir::DownLeft)) => 9,
        Some(Piece::Arrow(ArrowDir::UpLeft)) => 10,
    }
}

fn code_cell(code: u8) -> Option<Piece> {
    match code {
        1 => Some(Piece::Red),
        2 => Some(Piece::Yellow),
        3 => Some(Piece::Blue),
        4 => Some(Piece::Black),
        5 => Some(Piece::Slash(SlashDir::Forward)),
        6 => Some(Piece::Slash(SlashDir::Backward)),
        7 => Some(Piece::Arrow(ArrowDir::UpRight)),
        8 => Some(Piece::Arrow(ArrowDir::DownRight)),
        9 => Some(Piece::Arrow(ArrowDir::DownLeft)),
        10 => Some(Piece::Arrow(ArrowDir::UpLeft)),
        _ => None,
    }
}

const SHARE_KEY: [u8; 8] = [0x5a, 0x3c, 0x9e, 0x71, 0xb2, 0x4d, 0xe8, 0x16];
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | (*chunk.get(2).unwrap_or(&0) as u32);
        let idx = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        // bytes-in -> chars-out: 1->2, 2->3, 3->4 (drop padding chars).
        for &i in idx.iter().take(chunk.len() + 1) {
            out.push(B64[i as usize] as char);
        }
    }
    out
}

fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    for chunk in s.as_bytes().chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let mut vals = [0u32; 4];
        for (k, &c) in chunk.iter().enumerate() {
            vals[k] = val(c)? as u32;
        }
        let combined = vals[0] << 18 | vals[1] << 12 | vals[2] << 6 | vals[3];
        out.push((combined >> 16) as u8);
        if chunk.len() >= 3 {
            out.push((combined >> 8) as u8);
        }
        if chunk.len() >= 4 {
            out.push(combined as u8);
        }
    }
    Some(out)
}

/// Encode a board into a short, opaque share code (11 chars).
pub fn encode_board(board: &Board) -> String {
    let mut bytes = [0u8; 8];
    for i in 0..16 {
        let nib = cell_code(board[i / 4][i % 4]) & 0x0f;
        if i % 2 == 0 {
            bytes[i / 2] |= nib << 4;
        } else {
            bytes[i / 2] |= nib;
        }
    }
    for (b, k) in bytes.iter_mut().zip(SHARE_KEY.iter()) {
        *b ^= k;
    }
    base64url_encode(&bytes)
}

/// Decode a share code back into a board. Returns `None` if the code is
/// malformed or doesn't describe a legal board (so a tampered link is ignored).
pub fn decode_board(code: &str) -> Option<Board> {
    let decoded = base64url_decode(code)?;
    let mut bytes: [u8; 8] = decoded.try_into().ok()?;
    for (b, k) in bytes.iter_mut().zip(SHARE_KEY.iter()) {
        *b ^= k;
    }
    let mut board: Board = Default::default();
    for i in 0..16 {
        let byte = bytes[i / 2];
        let nib = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
        board[i / 4][i % 4] = code_cell(nib);
    }
    validate_board(&board).then_some(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_code_roundtrips() {
        let board = random_board();
        let code = encode_board(&board);
        assert_eq!(decode_board(&code), Some(board));
    }

    #[test]
    fn rejects_garbage_codes() {
        assert!(decode_board("!!!!").is_none());
        assert!(decode_board("AAAAAAAAAAA").is_none()); // decodes, but not a legal board
    }
}

// Jackson Coxson

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    FourArrow,
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
            Piece::FourArrow => "✛",
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
            Piece::FourArrow => "4-way mirror",
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
            Piece::FourArrow => "bg-slate-400 dark:bg-slate-600 text-black dark:text-white",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceType {
    Red,
    Yellow,
    Blue,
    Black,
    Slash,
    Arrow,
    FourArrow,
}

impl PieceType {
    pub const ALL: [PieceType; 7] = [
        PieceType::Red,
        PieceType::Yellow,
        PieceType::Blue,
        PieceType::Black,
        PieceType::Slash,
        PieceType::Arrow,
        PieceType::FourArrow,
    ];

    pub fn sample(self) -> Piece {
        match self {
            PieceType::Red => Piece::Red,
            PieceType::Yellow => Piece::Yellow,
            PieceType::Blue => Piece::Blue,
            PieceType::Black => Piece::Black,
            PieceType::Slash => Piece::Slash(SlashDir::Forward),
            PieceType::Arrow => Piece::Arrow(ArrowDir::UpRight),
            PieceType::FourArrow => Piece::FourArrow,
        }
    }

    pub fn of(piece: Piece) -> PieceType {
        match piece {
            Piece::Red => PieceType::Red,
            Piece::Yellow => PieceType::Yellow,
            Piece::Blue => PieceType::Blue,
            Piece::Black => PieceType::Black,
            Piece::Slash(_) => PieceType::Slash,
            Piece::Arrow(_) => PieceType::Arrow,
            Piece::FourArrow => PieceType::FourArrow,
        }
    }
}

pub fn same_piece_type(a: Piece, b: Piece) -> bool {
    PieceType::of(a) == PieceType::of(b)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub size: u8,
    pub cells: Vec<Option<Piece>>,
}

impl Default for Board {
    fn default() -> Self {
        Board::new(4)
    }
}

impl Board {
    pub fn new(size: u8) -> Self {
        Self {
            size,
            cells: vec![None; size as usize * size as usize],
        }
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size as usize
    }

    #[inline]
    fn idx(&self, row: usize, col: usize) -> usize {
        row * self.size() + col
    }

    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Option<Piece> {
        if row < self.size() && col < self.size() {
            self.cells[self.idx(row, col)]
        } else {
            None
        }
    }

    #[inline]
    pub fn set(&mut self, row: usize, col: usize, piece: Option<Piece>) {
        if row < self.size() && col < self.size() {
            let i = self.idx(row, col);
            self.cells[i] = piece;
        }
    }

    #[inline]
    pub fn toggle(&mut self, row: usize, col: usize, piece: Option<Piece>) {
        self.set(
            row,
            col,
            if self.get(row, col) == piece {
                None
            } else {
                piece
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PieceCounts {
    pub red: u8,
    pub yellow: u8,
    pub blue: u8,
    pub black: u8,
    pub slash: u8,
    pub arrow: u8,
    pub four_arrow: u8,
}

impl PieceCounts {
    pub fn classic() -> Self {
        Self {
            red: 1,
            yellow: 1,
            blue: 1,
            black: 1,
            slash: 2,
            arrow: 1,
            four_arrow: 0,
        }
    }

    pub fn total(&self) -> u32 {
        self.red as u32
            + self.yellow as u32
            + self.blue as u32
            + self.black as u32
            + self.slash as u32
            + self.arrow as u32
            + self.four_arrow as u32
    }

    pub fn get(&self, ty: PieceType) -> u8 {
        match ty {
            PieceType::Red => self.red,
            PieceType::Yellow => self.yellow,
            PieceType::Blue => self.blue,
            PieceType::Black => self.black,
            PieceType::Slash => self.slash,
            PieceType::Arrow => self.arrow,
            PieceType::FourArrow => self.four_arrow,
        }
    }

    pub fn set(&mut self, ty: PieceType, n: u8) {
        match ty {
            PieceType::Red => self.red = n,
            PieceType::Yellow => self.yellow = n,
            PieceType::Blue => self.blue = n,
            PieceType::Black => self.black = n,
            PieceType::Slash => self.slash = n,
            PieceType::Arrow => self.arrow = n,
            PieceType::FourArrow => self.four_arrow = n,
        }
    }
}

/// Whether each player designs a board for the other, or the computer generates
/// one hidden board that both players guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardSource {
    Versus,
    Computer,
}

/// How a two-player game resolves: Live is a real-time race (first correct
/// call wins). Async lets each player solve independently and compares scores
/// once both finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Live,
    Async,
}

/// The full configuration of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    pub size: u8,
    pub counts: PieceCounts,
    pub source: BoardSource,
    pub resolution: Resolution,
    pub timed: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules::classic_a()
    }
}

pub const MIN_SIZE: u8 = 4;
pub const MAX_SIZE: u8 = 10;

impl Rules {
    pub fn classic_a() -> Self {
        Self {
            size: 4,
            counts: PieceCounts::classic(),
            source: BoardSource::Versus,
            resolution: Resolution::Live,
            timed: false,
        }
    }

    pub fn ruleset_b() -> Self {
        let mut counts = PieceCounts::classic();
        counts.four_arrow = 1;
        Self {
            size: 5,
            counts,
            ..Rules::classic_a()
        }
    }

    pub fn turn_based(&self) -> bool {
        matches!(self.source, BoardSource::Versus) && matches!(self.resolution, Resolution::Live)
    }

    pub fn is_valid(&self) -> bool {
        let total = self.counts.total();
        (MIN_SIZE..=MAX_SIZE).contains(&self.size)
            && total >= 1
            && total <= self.size as u32 * self.size as u32
    }

    pub fn max_players(&self) -> usize {
        match self.source {
            BoardSource::Versus => 2,
            BoardSource::Computer => 8,
        }
    }
}

pub fn random_board(rules: &Rules) -> Board {
    use rand::RngExt;
    let mut rng = rand::rng();

    let slash = |rng: &mut rand::rngs::ThreadRng| {
        if rng.random_range(0..2) == 0 {
            Piece::Slash(SlashDir::Forward)
        } else {
            Piece::Slash(SlashDir::Backward)
        }
    };
    let arrow = |rng: &mut rand::rngs::ThreadRng| match rng.random_range(0..4) {
        0 => Piece::Arrow(ArrowDir::UpRight),
        1 => Piece::Arrow(ArrowDir::DownRight),
        2 => Piece::Arrow(ArrowDir::DownLeft),
        _ => Piece::Arrow(ArrowDir::UpLeft),
    };

    let c = &rules.counts;
    let mut pieces: Vec<Piece> = Vec::new();
    for _ in 0..c.red {
        pieces.push(Piece::Red);
    }
    for _ in 0..c.yellow {
        pieces.push(Piece::Yellow);
    }
    for _ in 0..c.blue {
        pieces.push(Piece::Blue);
    }
    for _ in 0..c.black {
        pieces.push(Piece::Black);
    }
    for _ in 0..c.slash {
        pieces.push(slash(&mut rng));
    }
    for _ in 0..c.arrow {
        pieces.push(arrow(&mut rng));
    }
    for _ in 0..c.four_arrow {
        pieces.push(Piece::FourArrow);
    }

    let n = rules.size as usize;
    let mut cells: Vec<(usize, usize)> = (0..n).flat_map(|r| (0..n).map(move |c| (r, c))).collect();
    for i in (1..cells.len()).rev() {
        let j = rng.random_range(0..(i + 1));
        cells.swap(i, j);
    }

    let mut board = Board::new(rules.size);
    for (piece, &(r, c)) in pieces.iter().zip(cells.iter()) {
        board.set(r, c, Some(*piece));
    }
    board
}

/// Tally how many of each piece type a board holds.
pub fn board_counts(board: &Board) -> PieceCounts {
    let mut counts = PieceCounts::default();
    for cell in board.cells.iter().flatten() {
        let ty = PieceType::of(*cell);
        counts.set(ty, counts.get(ty).saturating_add(1));
    }
    counts
}

/// A board is valid for a rule set when its size matches and it holds exactly
/// the configured number of each piece type.
pub fn validate_board(board: &Board, rules: &Rules) -> bool {
    board.size == rules.size && board_counts(board) == rules.counts
}

pub fn boards_match(a: &Board, b: &Board) -> bool {
    a == b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResyncShot {
    pub mine: bool,
    pub call: u8,
    pub exit: u8,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LobbyPlayer {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerResult {
    pub id: u8,
    pub nickname: String,
    pub score: Option<u32>,
    pub place: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ResyncPhase {
    Lobby,
    Setup {
        your_board: Option<Board>,
        you_locked: bool,
        opponent_locked: bool,
    },
    Playing {
        your_board: Board,
        shots: Vec<ResyncShot>,
        your_turn: bool,
        /// Whether the player has already used their one flip this turn.
        you_rotated: bool,
    },
    /// State for the independent engine (async / computer-board modes): the
    /// player shoots their own target freely, with a running score. `done` is
    /// the finished-flag for every player, so a rejoiner can show progress.
    Independent {
        your_board: Board,
        shots: Vec<ResyncShot>,
        score: u32,
        you_done: bool,
        done: Vec<bool>,
    },
    Over {
        you_win: bool,
        your_board: Board,
        /// The actual board the player was guessing, the answer.
        opponent_board: Board,
        shots: Vec<ResyncShot>,
        /// Final standings for an independent game (empty for the classic loop).
        results: Vec<PlayerResult>,
        /// The winning player id, if any.
        winner: Option<u8>,
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

// A board is encoded into a short, opaque string for shareable single player URLs.
// The first byte holds the board size; each of the following cells holds one of
// twelve values (empty or one of eleven pieces), packed two-per-byte. The bytes
// are XOR-scrambled with a fixed (cyclically repeated) key and base64url-encoded.
// The scramble is obfuscation, not security. It just stops the answer from
// being legible at a glance in the URL.

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
        Some(Piece::FourArrow) => 11,
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
        11 => Some(Piece::FourArrow),
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

/// Encode a board into a short, opaque share code.
pub fn encode_board(board: &Board) -> String {
    let cell_count = board.cells.len();
    let nib_bytes = cell_count.div_ceil(2);
    let mut bytes = vec![0u8; 1 + nib_bytes];
    bytes[0] = board.size;
    for (i, cell) in board.cells.iter().enumerate() {
        let nib = cell_code(*cell) & 0x0f;
        let bi = 1 + i / 2;
        if i % 2 == 0 {
            bytes[bi] |= nib << 4;
        } else {
            bytes[bi] |= nib;
        }
    }
    for (j, b) in bytes.iter_mut().enumerate() {
        *b ^= SHARE_KEY[j % SHARE_KEY.len()];
    }
    base64url_encode(&bytes)
}

pub fn decode_board(code: &str) -> Option<Board> {
    let mut bytes = base64url_decode(code)?;
    if bytes.is_empty() {
        return None;
    }
    for (j, b) in bytes.iter_mut().enumerate() {
        *b ^= SHARE_KEY[j % SHARE_KEY.len()];
    }
    let size = bytes[0];
    if !(MIN_SIZE..=MAX_SIZE).contains(&size) {
        return None;
    }
    let cell_count = size as usize * size as usize;
    let nib_bytes = cell_count.div_ceil(2);
    if bytes.len() != 1 + nib_bytes {
        return None;
    }
    let mut board = Board::new(size);
    for i in 0..cell_count {
        let byte = bytes[1 + i / 2];
        let nib = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
        board.cells[i] = code_cell(nib);
    }
    Some(board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::light_beams::beam_tracing::trace_beam;

    #[test]
    fn share_code_roundtrips_at_various_sizes() {
        for size in [4u8, 5, 7, 10] {
            let rules = Rules {
                size,
                counts: PieceCounts {
                    red: 1,
                    yellow: 1,
                    blue: 1,
                    black: 1,
                    slash: 2,
                    arrow: 1,
                    four_arrow: 1,
                },
                ..Rules::classic_a()
            };
            let board = random_board(&rules);
            let code = encode_board(&board);
            assert_eq!(decode_board(&code), Some(board), "size {size}");
        }
    }

    #[test]
    fn rejects_garbage_codes() {
        assert!(decode_board("!!!!").is_none());
        assert!(decode_board("").is_none());
    }

    #[test]
    fn classic_port_numbering_unchanged() {
        // The empty 4×4 board sends every beam straight across to the opposite
        // port, matching the original hardcoded layout.
        let board = Board::new(4);
        for (port, expected) in [(1u8, 12u8), (4, 9), (5, 16), (8, 13), (16, 5), (13, 8)] {
            let (exit, color) = trace_beam(&board, port);
            assert_eq!(exit, expected, "port {port}");
            assert_eq!(color, Color::White);
        }
    }

    #[test]
    fn four_way_mirror_returns_entry_port() {
        let mut board = Board::new(5);
        // Put a 4-way mirror in the path of port 1 (enters going down col 0).
        board.set(2, 0, Some(Piece::FourArrow));
        let (exit, _) = trace_beam(&board, 1);
        assert_eq!(exit, 1);
    }

    #[test]
    fn detects_infinite_loop() {
        let mut board = Board::new(4);
        board.set(1, 0, Some(Piece::Arrow(ArrowDir::UpRight))); // reflects Left→Right
        board.set(1, 2, Some(Piece::Arrow(ArrowDir::DownLeft))); // reflects Right→Left
                                                                 // Port 15 enters (1,0) going right; it passes the first arrow, reflects
                                                                 // off the second, reflects back off the first, and revisits a state.
        let (exit, _) = trace_beam(&board, 15);
        assert_eq!(exit, 0, "expected a trapped loop");
    }

    #[test]
    fn validate_checks_counts_and_size() {
        let rules = Rules::ruleset_b();
        let board = random_board(&rules);
        assert!(validate_board(&board, &rules));
        // Wrong size fails.
        let mut wrong = board.clone();
        wrong.size = 4;
        assert!(!validate_board(&wrong, &rules));
    }
}

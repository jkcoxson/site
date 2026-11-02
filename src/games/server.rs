// Jackson Coxson
// Game server: WebSocket room manager with lobby, passcodes, reconnect support.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, Path, Query,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::games::light_beams::{
    beam_tracing::trace_beam,
    logic::{
        boards_match, is_slash, random_board, rotate_piece, validate_board, Board, BoardSource,
        Color, LobbyPlayer, PlayerResult, Resolution, ResyncPhase, ResyncShot, Rules,
    },
};

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMsg {
    /// Sent immediately on connect. `token` is used to reclaim this slot on reconnect.
    Welcome { player: u8, token: String },
    /// The waiting room roster. `you` is the receiving player's id.
    Lobby {
        players: Vec<LobbyPlayer>,
        host: u8,
        you: u8,
        rules: Rules,
    },
    GameStart { rules: Rules },
    YourTurn,
    /// Play has begun but it's the opponent's move first.
    OpponentTurn,
    /// Every board is locked and the independent engine has started.
    BeginPlay,
    BeamResult { call: u8, exit: u8, color: Color },
    OpponentShot { call: u8, exit: u8, color: Color },
    OpponentLocked,
    GuessIncorrect,
    OpponentCalledGame,
    RotateResult { row: u8, col: u8, accepted: bool },
    /// The opponent attempted to flip one of your cells. `accepted` is true when
    /// they correctly guessed a mirror's location (and it was rotated).
    YourMirrorRotated { row: u8, col: u8, accepted: bool },
    /// You solved your board in an independent game.
    YouFinished { score: u32 },
    PlayerFinished { player: u8 },
    GameOver {
        you_win: bool,
        your_board: Board,
        answer_board: Board,
        results: Vec<PlayerResult>,
        winner: Option<u8>,
    },
    /// Full state snapshot sent after a successful reconnect.
    GameResync {
        player: u8,
        rules: Rules,
        players: Vec<LobbyPlayer>,
        host: u8,
        phase: ResyncPhase,
    },
    PlayerConnected { player: u8 },
    PlayerDisconnected { player: u8 },
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    SetNickname { name: String },
    StartGame,
    LockBoard { board: Board },
    ShootBeam { port: u8 },
    CallGame { guess: Board },
    RequestRotate { row: u8, col: u8 },
    PlayAgain,
}

// ── Match state machine ───────────────────────────────────────────────────────

enum MatchPhase {
    Lobby,
    Setup {
        locked: [bool; 2],
        boards: [Option<Board>; 2],
    },
    Playing {
        boards: [Board; 2],
        current: usize,
        rotated: bool,
    },
    /// Async and computer-board games: every player shoots their own target
    /// freely. `wrong` and `wasted` count bad calls and flips that missed a
    /// mirror; both cost a point like a beam does.
    Independent {
        own: Vec<Board>,
        targets: Vec<Board>,
        wrong: Vec<u32>,
        wasted: Vec<u32>,
        done: Vec<bool>,
        score: Vec<Option<u32>>,
        start: Instant,
    },
    Over {
        own: Vec<Board>,
        targets: Vec<Board>,
        results: Vec<PlayerResult>,
        winner: Option<usize>,
        turn_based: bool,
    },
}

struct ShotRecord {
    player: usize,
    call: u8,
    exit: u8,
    color: Color,
}

struct Player {
    nickname: String,
    /// Rejoin token, generated when the slot is first claimed.
    token: String,
    sender: Option<mpsc::UnboundedSender<String>>,
    connected: bool,
}

/// Disconnected lobby slots are released after this long so others can join.
const LOBBY_GRACE: Duration = Duration::from_secs(30);
const MAX_NICKNAME: usize = 20;

struct Match {
    phase: MatchPhase,
    rules: Rules,
    players: Vec<Player>,
    shot_history: Vec<ShotRecord>,
    /// When the room last became empty (no connected players). `None` while at
    /// least one player is connected. Used by the reaper to clear abandoned rooms.
    empty_since: Option<Instant>,
}

impl Match {
    fn new(rules: Rules) -> Self {
        Self {
            phase: MatchPhase::Lobby,
            rules,
            players: Vec::new(),
            shot_history: Vec::new(),
            empty_since: Some(Instant::now()),
        }
    }

    /// Refresh the empty-room timer based on the current connection state:
    /// clears it while anyone is connected, and starts it the moment the room
    /// becomes empty.
    fn touch_empty(&mut self) {
        if self.players.iter().any(|p| p.connected) {
            self.empty_since = None;
        } else if self.empty_since.is_none() {
            self.empty_since = Some(Instant::now());
        }
    }

    fn index_of(&self, token: &str) -> Option<usize> {
        self.players.iter().position(|p| p.token == token)
    }

    fn send(&self, player: usize, msg: &ServerMsg) {
        if let Some(Some(tx)) = self.players.get(player).map(|p| &p.sender) {
            let _ = tx.send(serde_json::to_string(msg).unwrap_or_default());
        }
    }

    fn send_others(&self, player: usize, msg: &ServerMsg) {
        for p in 0..self.players.len() {
            if p != player {
                self.send(p, msg);
            }
        }
    }

    fn broadcast(&self, msg: &ServerMsg) {
        let json = serde_json::to_string(msg).unwrap_or_default();
        for p in &self.players {
            if let Some(tx) = &p.sender {
                let _ = tx.send(json.clone());
            }
        }
    }

    fn roster(&self) -> Vec<LobbyPlayer> {
        self.players
            .iter()
            .enumerate()
            .map(|(i, p)| LobbyPlayer {
                id: i as u8,
                nickname: p.nickname.clone(),
                connected: p.connected,
            })
            .collect()
    }

    /// Send every player the roster, each with their own `you`.
    fn broadcast_lobby(&self) {
        let players = self.roster();
        for p in 0..self.players.len() {
            self.send(
                p,
                &ServerMsg::Lobby {
                    players: players.clone(),
                    host: 0,
                    you: p as u8,
                    rules: self.rules,
                },
            );
        }
    }

    fn capacity(&self) -> usize {
        self.rules.max_players()
    }

    fn shots_for(&self, player: usize, all: bool) -> Vec<ResyncShot> {
        self.shot_history
            .iter()
            .filter(|s| all || s.player == player)
            .map(|s| ResyncShot {
                mine: s.player == player,
                call: s.call,
                exit: s.exit,
                color: s.color,
            })
            .collect()
    }

    /// Beams fired plus penalties: what an untimed independent game scores.
    fn running_score(&self, player: usize) -> u32 {
        let shots = self.shot_history.iter().filter(|s| s.player == player).count() as u32;
        match &self.phase {
            MatchPhase::Independent { wrong, wasted, .. } => shots + wrong[player] + wasted[player],
            _ => shots,
        }
    }

    fn resync(&self, player: usize) -> ResyncPhase {
        let other = 1 - player.min(1);
        match &self.phase {
            MatchPhase::Lobby => ResyncPhase::Lobby,
            MatchPhase::Setup { locked, boards } => ResyncPhase::Setup {
                your_board: boards[player].clone(),
                you_locked: locked[player],
                opponent_locked: locked[other],
            },
            MatchPhase::Playing {
                boards,
                current,
                rotated,
            } => ResyncPhase::Playing {
                your_board: boards[player].clone(),
                shots: self.shots_for(player, true),
                your_turn: *current == player,
                you_rotated: *current == player && *rotated,
            },
            MatchPhase::Independent { own, done, .. } => ResyncPhase::Independent {
                your_board: own[player].clone(),
                shots: self.shots_for(player, false),
                score: self.running_score(player),
                you_done: done[player],
                done: done.clone(),
            },
            MatchPhase::Over {
                own,
                targets,
                results,
                winner,
                turn_based,
            } => ResyncPhase::Over {
                you_win: *winner == Some(player),
                your_board: own[player].clone(),
                opponent_board: targets[player].clone(),
                shots: self.shots_for(player, *turn_based),
                results: results.clone(),
                winner: winner.map(|w| w as u8),
            },
        }
    }

    fn game_resync(&self, player: usize) -> ServerMsg {
        ServerMsg::GameResync {
            player: player as u8,
            rules: self.rules,
            players: self.roster(),
            host: 0,
            phase: self.resync(player),
        }
    }

    /// Final standings. Finished players rank by score (lower is better, ties
    /// share a place); everyone who didn't finish ranks after them.
    fn standings(&self) -> Vec<PlayerResult> {
        let scores: Vec<Option<u32>> = match &self.phase {
            MatchPhase::Independent { score, .. } => score.clone(),
            _ => vec![None; self.players.len()],
        };
        let finished = scores.iter().filter(|s| s.is_some()).count() as u8;
        self.players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let place = match scores[i] {
                    Some(s) => 1 + scores.iter().flatten().filter(|&&o| o < s).count() as u8,
                    None => finished + 1,
                };
                PlayerResult {
                    id: i as u8,
                    nickname: p.nickname.clone(),
                    score: scores[i],
                    place,
                }
            })
            .collect()
    }

    /// An async game ends once every connected player has finished.
    fn async_ready(&self) -> bool {
        match &self.phase {
            MatchPhase::Independent { done, .. } => {
                matches!(self.rules.resolution, Resolution::Async)
                    && self.players.iter().zip(done).all(|(p, &d)| d || !p.connected)
            }
            _ => false,
        }
    }

    /// The single lowest score among finished players; a tie has no winner.
    fn async_winner(&self) -> Option<usize> {
        let MatchPhase::Independent { score, .. } = &self.phase else {
            return None;
        };
        let best = score.iter().flatten().min()?;
        let mut at_best = score.iter().enumerate().filter(|(_, s)| *s == &Some(*best));
        let (winner, _) = at_best.next()?;
        at_best.next().is_none().then_some(winner)
    }

    /// End an independent game. `winner` is the racer who called it first; an
    /// async game picks the best score instead.
    fn finish_independent(&mut self, winner: Option<usize>) {
        let winner = winner.or_else(|| self.async_winner());
        let results = self.standings();
        let (own, targets) = match &self.phase {
            MatchPhase::Independent { own, targets, .. } => (own.clone(), targets.clone()),
            _ => return,
        };
        for p in 0..self.players.len() {
            self.send(
                p,
                &ServerMsg::GameOver {
                    you_win: winner == Some(p),
                    your_board: own[p].clone(),
                    answer_board: targets[p].clone(),
                    results: results.clone(),
                    winner: winner.map(|w| w as u8),
                },
            );
        }
        self.phase = MatchPhase::Over {
            own,
            targets,
            results,
            winner,
            turn_based: false,
        };
    }

    fn handle(&mut self, player: usize, msg: ClientMsg) {
        match msg {
            ClientMsg::SetNickname { name } => self.on_set_nickname(player, name),
            ClientMsg::StartGame => self.on_start(player),
            ClientMsg::LockBoard { board } => self.on_lock_board(player, board),
            ClientMsg::ShootBeam { port } => self.on_shoot(player, port),
            ClientMsg::CallGame { guess } => self.on_call_game(player, guess),
            ClientMsg::RequestRotate { row, col } => self.on_request_rotate(player, row, col),
            ClientMsg::PlayAgain => self.on_play_again(),
        }
    }

    fn on_set_nickname(&mut self, player: usize, name: String) {
        if !matches!(self.phase, MatchPhase::Lobby) {
            return;
        }
        let name: String = name.trim().chars().take(MAX_NICKNAME).collect();
        if name.is_empty() {
            return;
        }
        self.players[player].nickname = name;
        self.broadcast_lobby();
    }

    fn on_start(&mut self, player: usize) {
        if player != 0 || !matches!(self.phase, MatchPhase::Lobby) {
            return;
        }
        if self.players.len() < 2 {
            self.send(
                player,
                &ServerMsg::Error("Need at least two players to start.".into()),
            );
            return;
        }
        if self.players.len() > self.capacity() {
            self.send(player, &ServerMsg::Error("Too many players for this mode.".into()));
            return;
        }
        self.shot_history.clear();
        self.broadcast(&ServerMsg::GameStart { rules: self.rules });
        self.phase = match self.rules.source {
            BoardSource::Versus => MatchPhase::Setup {
                locked: [false; 2],
                boards: [None, None],
            },
            BoardSource::Computer => {
                let n = self.players.len();
                let answer = random_board(&self.rules);
                MatchPhase::Independent {
                    own: vec![Board::new(self.rules.size); n],
                    targets: vec![answer; n],
                    wrong: vec![0; n],
                    wasted: vec![0; n],
                    done: vec![false; n],
                    score: vec![None; n],
                    start: Instant::now(),
                }
            }
        };
    }

    fn on_lock_board(&mut self, player: usize, board: Board) {
        if !matches!(self.phase, MatchPhase::Setup { .. }) {
            return;
        }
        if !validate_board(&board, &self.rules) {
            self.send(player, &ServerMsg::Error("Invalid board.".into()));
            return;
        }
        enum Outcome {
            Locked { other: usize },
            BothLocked { boards: [Board; 2] },
        }
        let MatchPhase::Setup { locked, boards } = &mut self.phase else {
            return;
        };
        boards[player] = Some(board);
        locked[player] = true;
        let outcome = if locked[0] && locked[1] {
            Outcome::BothLocked {
                boards: [boards[0].clone().unwrap(), boards[1].clone().unwrap()],
            }
        } else {
            Outcome::Locked { other: 1 - player }
        };
        match outcome {
            Outcome::Locked { other } => self.send(other, &ServerMsg::OpponentLocked),
            Outcome::BothLocked { boards } => {
                self.broadcast(&ServerMsg::OpponentLocked);
                if self.rules.turn_based() {
                    self.phase = MatchPhase::Playing {
                        boards,
                        current: 0,
                        rotated: false,
                    };
                    self.send(0, &ServerMsg::YourTurn);
                    self.send(1, &ServerMsg::OpponentTurn);
                } else {
                    // Each player solves the board their opponent designed.
                    let [a, b] = boards;
                    self.phase = MatchPhase::Independent {
                        targets: vec![b.clone(), a.clone()],
                        own: vec![a, b],
                        wrong: vec![0; 2],
                        wasted: vec![0; 2],
                        done: vec![false; 2],
                        score: vec![None; 2],
                        start: Instant::now(),
                    };
                    self.broadcast(&ServerMsg::BeginPlay);
                }
            }
        }
    }

    fn valid_port(&self, port: u8) -> bool {
        (1..=4 * self.rules.size as u32).contains(&(port as u32))
    }

    fn on_shoot(&mut self, player: usize, port: u8) {
        let valid_port = self.valid_port(port);
        enum Outcome {
            Ignore,
            NotYourTurn,
            InvalidPort,
            Shot { other: usize, exit: u8, color: Color },
            Solo { exit: u8, color: Color },
        }
        let outcome = match &mut self.phase {
            MatchPhase::Playing {
                boards,
                current,
                rotated,
            } => {
                if *current != player {
                    Outcome::NotYourTurn
                } else if !valid_port {
                    Outcome::InvalidPort
                } else {
                    let other = 1 - player;
                    let (exit, color) = trace_beam(&boards[other], port);
                    *current = other;
                    // Fresh turn for the opponent, they get a new flip.
                    *rotated = false;
                    Outcome::Shot { other, exit, color }
                }
            }
            MatchPhase::Independent { targets, done, .. } => {
                if done[player] {
                    Outcome::Ignore
                } else if !valid_port {
                    Outcome::InvalidPort
                } else {
                    let (exit, color) = trace_beam(&targets[player], port);
                    Outcome::Solo { exit, color }
                }
            }
            _ => Outcome::Ignore,
        };
        match outcome {
            Outcome::Ignore => {}
            Outcome::NotYourTurn => self.send(player, &ServerMsg::Error("Not your turn.".into())),
            Outcome::InvalidPort => self.send(player, &ServerMsg::Error("Invalid port.".into())),
            Outcome::Shot { other, exit, color } => {
                self.shot_history.push(ShotRecord {
                    player,
                    call: port,
                    exit,
                    color,
                });
                self.send(player, &ServerMsg::BeamResult { call: port, exit, color });
                self.send(other, &ServerMsg::OpponentShot { call: port, exit, color });
                self.send(other, &ServerMsg::YourTurn);
            }
            Outcome::Solo { exit, color } => {
                self.shot_history.push(ShotRecord {
                    player,
                    call: port,
                    exit,
                    color,
                });
                self.send(player, &ServerMsg::BeamResult { call: port, exit, color });
            }
        }
    }

    fn on_call_game(&mut self, player: usize, guess: Board) {
        let timed = self.rules.timed;
        let race = matches!(self.rules.resolution, Resolution::Live);
        let beams = self.shot_history.iter().filter(|s| s.player == player).count() as u32;
        enum Outcome {
            Ignore,
            NotYourTurn,
            Correct { boards: [Board; 2] },
            Wrong,
            Solved { score: u32 },
            Missed,
        }
        let outcome = match &mut self.phase {
            MatchPhase::Playing {
                boards,
                current,
                rotated,
            } => {
                if *current != player {
                    Outcome::NotYourTurn
                } else if boards_match(&guess, &boards[1 - player]) {
                    Outcome::Correct {
                        boards: [boards[0].clone(), boards[1].clone()],
                    }
                } else {
                    // A wrong guess ends your turn, just like shooting does.
                    *current = 1 - player;
                    *rotated = false;
                    Outcome::Wrong
                }
            }
            MatchPhase::Independent {
                targets,
                wrong,
                wasted,
                done,
                score,
                start,
                ..
            } => {
                if done[player] {
                    Outcome::Ignore
                } else if boards_match(&guess, &targets[player]) {
                    let s = if timed {
                        start.elapsed().as_secs() as u32
                    } else {
                        beams + wrong[player] + wasted[player]
                    };
                    done[player] = true;
                    score[player] = Some(s);
                    Outcome::Solved { score: s }
                } else {
                    wrong[player] += 1;
                    Outcome::Missed
                }
            }
            _ => Outcome::Ignore,
        };
        let other = 1 - player.min(1);
        match outcome {
            Outcome::Ignore => {}
            Outcome::NotYourTurn => self.send(
                player,
                &ServerMsg::Error("You can only call the game on your turn.".into()),
            ),
            Outcome::Correct { boards } => {
                let results = self.standings();
                let own = vec![boards[0].clone(), boards[1].clone()];
                let targets = vec![boards[1].clone(), boards[0].clone()];
                for p in 0..2 {
                    // Each player sees the opponent's actual board, the answer.
                    self.send(
                        p,
                        &ServerMsg::GameOver {
                            you_win: p == player,
                            your_board: own[p].clone(),
                            answer_board: targets[p].clone(),
                            results: results.clone(),
                            winner: Some(player as u8),
                        },
                    );
                }
                self.phase = MatchPhase::Over {
                    own,
                    targets,
                    results,
                    winner: Some(player),
                    turn_based: true,
                };
            }
            Outcome::Wrong => {
                self.send(player, &ServerMsg::GuessIncorrect);
                self.send(other, &ServerMsg::OpponentCalledGame);
                self.send(other, &ServerMsg::YourTurn);
            }
            Outcome::Solved { score } => {
                if race {
                    self.finish_independent(Some(player));
                } else if self.async_ready() {
                    // The last solver goes straight to the results.
                    self.finish_independent(None);
                } else {
                    self.send(player, &ServerMsg::YouFinished { score });
                    self.send_others(
                        player,
                        &ServerMsg::PlayerFinished {
                            player: player as u8,
                        },
                    );
                }
            }
            Outcome::Missed => self.send(player, &ServerMsg::GuessIncorrect),
        }
    }

    fn on_request_rotate(&mut self, player: usize, row: u8, col: u8) {
        if row >= self.rules.size || col >= self.rules.size {
            self.send(player, &ServerMsg::Error("Invalid coordinates.".into()));
            return;
        }
        let (r, c) = (row as usize, col as usize);
        let other = 1 - player.min(1);
        enum Outcome {
            Ignore,
            NotYourTurn,
            AlreadyFlipped,
            Done { accepted: bool },
            IndependentDone { accepted: bool },
        }
        let outcome = match &mut self.phase {
            MatchPhase::Playing {
                boards,
                current,
                rotated,
            } => {
                if *current != player {
                    Outcome::NotYourTurn
                } else if *rotated {
                    Outcome::AlreadyFlipped
                } else {
                    // The attempt is spent whether or not it lands on a mirror.
                    *rotated = true;
                    let accepted = match boards[other].get(r, c) {
                        Some(p) if is_slash(p) => {
                            boards[other].set(r, c, Some(rotate_piece(p)));
                            true
                        }
                        _ => false,
                    };
                    // A correct flip keeps your turn (you still shoot); flipping
                    // a cell with no mirror loses your turn.
                    if !accepted {
                        *current = other;
                        *rotated = false;
                    }
                    Outcome::Done { accepted }
                }
            }
            MatchPhase::Independent {
                targets,
                wasted,
                done,
                ..
            } => {
                if done[player] {
                    Outcome::Ignore
                } else {
                    let accepted = match targets[player].get(r, c) {
                        Some(p) if is_slash(p) => {
                            targets[player].set(r, c, Some(rotate_piece(p)));
                            true
                        }
                        _ => {
                            wasted[player] += 1;
                            false
                        }
                    };
                    Outcome::IndependentDone { accepted }
                }
            }
            _ => Outcome::Ignore,
        };
        match outcome {
            Outcome::Ignore => {}
            Outcome::NotYourTurn => self.send(
                player,
                &ServerMsg::Error("You can only flip a mirror on your turn.".into()),
            ),
            Outcome::AlreadyFlipped => self.send(
                player,
                &ServerMsg::Error("You can only flip once per turn.".into()),
            ),
            Outcome::Done { accepted } => {
                self.send(player, &ServerMsg::RotateResult { row, col, accepted });
                if accepted {
                    self.send(other, &ServerMsg::YourMirrorRotated { row, col, accepted });
                } else {
                    // Turn passed to the opponent. Send YourTurn first, its
                    // client handler clears the last-mirror notice, then the
                    // flip-attempt notice so the opponent still sees it.
                    self.send(other, &ServerMsg::YourTurn);
                    self.send(other, &ServerMsg::YourMirrorRotated { row, col, accepted });
                }
            }
            Outcome::IndependentDone { accepted } => {
                self.send(player, &ServerMsg::RotateResult { row, col, accepted })
            }
        }
    }

    fn on_play_again(&mut self) {
        if !matches!(self.phase, MatchPhase::Over { .. }) {
            return;
        }
        // Whoever left during the game gives up their seat.
        self.players.retain(|p| p.connected);
        self.shot_history.clear();
        self.phase = MatchPhase::Lobby;
        self.broadcast_lobby();
    }

    /// Mark a player gone. Returns true when the slot should be pruned later.
    fn on_disconnect(&mut self, player: usize) -> bool {
        self.players[player].connected = false;
        self.players[player].sender = None;
        self.touch_empty();
        if matches!(self.phase, MatchPhase::Lobby) {
            self.broadcast_lobby();
            return true;
        }
        self.send_others(
            player,
            &ServerMsg::PlayerDisconnected {
                player: player as u8,
            },
        );
        if self.async_ready() {
            self.finish_independent(None);
        }
        false
    }
}

// ── Lobby ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub players: u8,
    pub capacity: u8,
    pub in_progress: bool,
    pub mode: String,
}

struct LobbyRoom {
    name: String,
    passcode: String,
    game: Arc<Mutex<Match>>,
}

/// Short description of a room's rules for the room list, e.g. `5×5 · async`.
pub fn mode_label(rules: &Rules) -> String {
    let mut label = format!("{0}×{0}", rules.size);
    if matches!(rules.source, BoardSource::Computer) {
        label.push_str(" · computer");
    }
    if matches!(rules.resolution, Resolution::Async) {
        label.push_str(" · async");
    }
    if rules.timed {
        label.push_str(" · timed");
    }
    label
}

// ── Global state ──────────────────────────────────────────────────────────────

pub struct GameState {
    rooms: RwLock<HashMap<String, LobbyRoom>>,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_room(&self, id: String, name: String, passcode: String, rules: Rules) {
        let mut w = self.rooms.write().await;
        w.insert(
            id,
            LobbyRoom {
                name,
                passcode,
                game: Arc::new(Mutex::new(Match::new(rules))),
            },
        );
    }

    pub async fn list_rooms(&self) -> Vec<RoomInfo> {
        let r = self.rooms.read().await;
        let mut infos = Vec::new();
        for (id, room) in r.iter() {
            if let Ok(game) = room.game.try_lock() {
                infos.push(RoomInfo {
                    id: id.clone(),
                    name: room.name.clone(),
                    players: game.players.iter().filter(|p| p.connected).count() as u8,
                    capacity: game.capacity() as u8,
                    in_progress: !matches!(game.phase, MatchPhase::Lobby),
                    mode: mode_label(&game.rules),
                });
            }
        }
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }

    pub async fn room_exists(&self, id: &str) -> bool {
        self.rooms.read().await.contains_key(id)
    }

    async fn get_room(&self, id: &str) -> Option<(Arc<Mutex<Match>>, String)> {
        let r = self.rooms.read().await;
        r.get(id)
            .map(|room| (room.game.clone(), room.passcode.clone()))
    }

    async fn remove_if_over(&self, id: &str) {
        let should_remove = {
            let r = self.rooms.read().await;
            r.get(id)
                .and_then(|room| room.game.try_lock().ok())
                .map(|game| {
                    matches!(game.phase, MatchPhase::Over { .. })
                        && game.players.iter().all(|p| !p.connected)
                })
                .unwrap_or(false)
        };
        if should_remove {
            self.rooms.write().await.remove(id);
        }
    }

    /// Drop a lobby slot whose player never came back, so the seat opens up.
    async fn prune_lobby_slot(&self, id: &str, token: &str) {
        let Some((game, _)) = self.get_room(id).await else {
            return;
        };
        let mut r = game.lock().await;
        if !matches!(r.phase, MatchPhase::Lobby) {
            return;
        }
        if let Some(idx) = r.index_of(token) {
            if !r.players[idx].connected {
                r.players.remove(idx);
                r.broadcast_lobby();
            }
        }
    }

    /// Remove rooms that have had no connected players for at least `idle_ttl`.
    /// Covers rooms abandoned before the game finishes (created-but-never-joined,
    /// quit mid-match) which `remove_if_over` never reaps.
    async fn reap_idle_rooms(&self, idle_ttl: Duration) {
        let stale: Vec<String> = {
            let r = self.rooms.read().await;
            r.iter()
                .filter_map(|(id, room)| {
                    let game = room.game.try_lock().ok()?;
                    match game.empty_since {
                        Some(t) if t.elapsed() >= idle_ttl => Some(id.clone()),
                        _ => None,
                    }
                })
                .collect()
        };
        if stale.is_empty() {
            return;
        }
        let mut w = self.rooms.write().await;
        for id in stale {
            // Re-check under the write lock: a player may have reconnected
            // between the read scan and acquiring the write lock.
            let still_idle = w
                .get(&id)
                .and_then(|room| room.game.try_lock().ok())
                .map(|game| game.empty_since.is_some_and(|t| t.elapsed() >= idle_ttl))
                .unwrap_or(false);
            if still_idle {
                w.remove(&id);
            }
        }
    }

    /// Spawn a background task that periodically reaps idle rooms. Rooms empty
    /// for more than 5 minutes are removed.
    pub fn spawn_reaper(self: Arc<Self>) {
        const IDLE_TTL: Duration = Duration::from_secs(5 * 60);
        const SCAN_INTERVAL: Duration = Duration::from_secs(60);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SCAN_INTERVAL);
            loop {
                ticker.tick().await;
                self.reap_idle_rooms(IDLE_TTL).await;
            }
        });
    }
}

// ── Slot assignment ───────────────────────────────────────────────────────────

enum SlotResult {
    New { player_id: usize, token: String },
    Reconnected { player_id: usize, token: String },
    InProgress,
    Full,
}

fn assign_new_slot(r: &mut Match, tx: mpsc::UnboundedSender<String>) -> SlotResult {
    if !matches!(r.phase, MatchPhase::Lobby) {
        return SlotResult::InProgress;
    }
    if r.players.len() >= r.capacity() {
        return SlotResult::Full;
    }
    let token = gen_token();
    let nickname = gen_nickname(r);
    r.players.push(Player {
        nickname,
        token: token.clone(),
        sender: Some(tx),
        connected: true,
    });
    r.touch_empty();
    SlotResult::New {
        player_id: r.players.len() - 1,
        token,
    }
}

// ── Token generation ──────────────────────────────────────────────────────────

fn gen_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..8)
        .map(|_| format!("{:02x}", rng.random_range(0u8..=255)))
        .collect()
}

const ADJECTIVES: [&str; 16] = [
    "Bright", "Clever", "Swift", "Brave", "Calm", "Lucky", "Mighty", "Gentle", "Bold", "Sunny",
    "Quiet", "Witty", "Eager", "Jolly", "Keen", "Nimble",
];
const ANIMALS: [&str; 16] = [
    "Otter", "Falcon", "Badger", "Lynx", "Heron", "Fox", "Panda", "Wolf", "Raven", "Tiger",
    "Moose", "Hawk", "Bison", "Crane", "Seal", "Owl",
];

/// A friendly default nickname that nobody else in the room is using.
fn gen_nickname(r: &Match) -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    loop {
        let name = format!(
            "{} {}",
            ADJECTIVES[rng.random_range(0..ADJECTIVES.len())],
            ANIMALS[rng.random_range(0..ANIMALS.len())]
        );
        if r.players.iter().all(|p| p.nickname != name) {
            return name;
        }
    }
}

// ── WebSocket URL query params ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

// ── Axum WebSocket handler ────────────────────────────────────────────────────

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path((room_id, passcode)): Path<(String, String)>,
    Query(query): Query<WsQuery>,
    Extension(state): Extension<Arc<GameState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, room_id, passcode, query.token, state))
}

async fn handle_socket(
    socket: WebSocket,
    room_id: String,
    passcode: String,
    reconnect_token: Option<String>,
    state: Arc<GameState>,
) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let reject = |msg: &str| serde_json::to_string(&ServerMsg::Error(msg.into())).unwrap();

    let Some((game, expected_passcode)) = state.get_room(&room_id).await else {
        let _ = sink.send(Message::Text(reject("Room not found.").into())).await;
        return;
    };

    if passcode != expected_passcode {
        let _ = sink.send(Message::Text(reject("Wrong passcode.").into())).await;
        return;
    }

    // ── Assign/reclaim player slot ─────────────────────────────────────────
    let slot_result = {
        let mut r = game.lock().await;
        // Try to reclaim a slot by token; an unknown token is just a new player.
        match reconnect_token.as_deref().and_then(|tok| r.index_of(tok)) {
            Some(idx) => {
                r.players[idx].sender = Some(tx.clone());
                r.players[idx].connected = true;
                r.touch_empty();
                SlotResult::Reconnected {
                    player_id: idx,
                    token: r.players[idx].token.clone(),
                }
            }
            None => assign_new_slot(&mut r, tx.clone()),
        }
    };

    // Player ids shift when a lobby slot is pruned, so the socket remembers
    // its token and looks its index up fresh each time.
    let token = match slot_result {
        SlotResult::Full => {
            let _ = sink.send(Message::Text(reject("Room is full.").into())).await;
            return;
        }
        SlotResult::InProgress => {
            let _ = sink
                .send(Message::Text(reject("Game already in progress.").into()))
                .await;
            return;
        }
        SlotResult::New { player_id, token } => {
            let r = game.lock().await;
            r.send(
                player_id,
                &ServerMsg::Welcome {
                    player: player_id as u8,
                    token: token.clone(),
                },
            );
            r.broadcast_lobby();
            token
        }
        SlotResult::Reconnected { player_id, token } => {
            let r = game.lock().await;
            r.send(
                player_id,
                &ServerMsg::Welcome {
                    player: player_id as u8,
                    token: token.clone(),
                },
            );
            r.send(player_id, &r.game_resync(player_id));
            if matches!(r.phase, MatchPhase::Lobby) {
                r.broadcast_lobby();
            } else {
                r.send_others(
                    player_id,
                    &ServerMsg::PlayerConnected {
                        player: player_id as u8,
                    },
                );
            }
            token
        }
    };

    // ── Forward channel messages to WebSocket sink ─────────────────────────
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        if let Ok(client_msg) = serde_json::from_str::<ClientMsg>(&text) {
            let mut r = game.lock().await;
            if let Some(idx) = r.index_of(&token) {
                r.handle(idx, client_msg);
            }
        }
    }

    send_task.abort();

    // Mark slot as disconnected (keep game state alive)
    let prune = {
        let mut r = game.lock().await;
        match r.index_of(&token) {
            // A newer socket may have reclaimed this slot already.
            Some(idx) if r.players[idx].sender.as_ref().is_some_and(|s| s.same_channel(&tx)) => {
                r.on_disconnect(idx)
            }
            _ => false,
        }
    };
    if prune {
        let state = state.clone();
        let room_id = room_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(LOBBY_GRACE).await;
            state.prune_lobby_slot(&room_id, &token).await;
        });
    }

    // Only remove the room if the game is over AND everyone disconnected
    state.remove_if_over(&room_id).await;
}

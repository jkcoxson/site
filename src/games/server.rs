// Jackson Coxson
// Light Beams game server, plus the websocket and room-list entry points for
// every game. Rooms, seats and reconnects are handled by `rooms`.

use std::{sync::Arc, time::Instant};

use axum::{
    extract::{ws::WebSocketUpgrade, Extension, Path, Query},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::games::{
    five_crowns::server::FiveCrowns,
    light_beams::{
        beam_tracing::trace_beam,
        logic::{
            boards_match, is_slash, random_board, rotate_piece, validate_board, Board,
            BoardSource, Color, LobbyPlayer, PlayerResult, Resolution, ResyncPhase, ResyncShot,
            Rules,
        },
    },
    rooms::{self, RoomGame, Rooms, Seats},
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

pub struct LightBeams {
    phase: MatchPhase,
    rules: Rules,
    shot_history: Vec<ShotRecord>,
}

impl LightBeams {
    pub fn new(rules: Rules) -> Self {
        Self {
            phase: MatchPhase::Lobby,
            rules,
            shot_history: Vec::new(),
        }
    }

    fn roster(seats: &Seats) -> Vec<LobbyPlayer> {
        seats
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
    fn broadcast_lobby(&self, seats: &Seats) {
        let players = Self::roster(seats);
        for p in 0..seats.len() {
            seats.send(
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

    fn game_resync(&self, seats: &Seats, player: usize) -> ServerMsg {
        ServerMsg::GameResync {
            player: player as u8,
            rules: self.rules,
            players: Self::roster(seats),
            host: 0,
            phase: self.resync(player),
        }
    }

    /// Final standings. Finished players rank by score (lower is better, ties
    /// share a place); everyone who didn't finish ranks after them.
    fn standings(&self, seats: &Seats) -> Vec<PlayerResult> {
        let scores: Vec<Option<u32>> = match &self.phase {
            MatchPhase::Independent { score, .. } => score.clone(),
            _ => vec![None; seats.len()],
        };
        let finished = scores.iter().filter(|s| s.is_some()).count() as u8;
        seats
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
    fn async_ready(&self, seats: &Seats) -> bool {
        match &self.phase {
            MatchPhase::Independent { done, .. } => {
                matches!(self.rules.resolution, Resolution::Async)
                    && seats.iter().zip(done).all(|(p, &d)| d || !p.connected)
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
    fn finish_independent(&mut self, seats: &Seats, winner: Option<usize>) {
        let winner = winner.or_else(|| self.async_winner());
        let results = self.standings(seats);
        let (own, targets) = match &self.phase {
            MatchPhase::Independent { own, targets, .. } => (own.clone(), targets.clone()),
            _ => return,
        };
        for p in 0..seats.len() {
            seats.send(
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

    fn dispatch(&mut self, seats: &mut Seats, player: usize, msg: ClientMsg) {
        match msg {
            ClientMsg::SetNickname { name } => self.on_set_nickname(seats, player, name),
            ClientMsg::StartGame => self.on_start(seats, player),
            ClientMsg::LockBoard { board } => self.on_lock_board(seats, player, board),
            ClientMsg::ShootBeam { port } => self.on_shoot(seats, player, port),
            ClientMsg::CallGame { guess } => self.on_call_game(seats, player, guess),
            ClientMsg::RequestRotate { row, col } => self.on_request_rotate(seats, player, row, col),
            ClientMsg::PlayAgain => self.on_play_again(seats),
        }
    }

    fn on_set_nickname(&mut self, seats: &mut Seats, player: usize, name: String) {
        if !matches!(self.phase, MatchPhase::Lobby) {
            return;
        }
        if seats.set_nickname(player, &name) {
            self.broadcast_lobby(seats);
        }
    }

    fn on_start(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, MatchPhase::Lobby) {
            return;
        }
        if seats.len() < 2 {
            seats.send(
                player,
                &ServerMsg::Error("Need at least two players to start.".into()),
            );
            return;
        }
        if seats.len() > self.capacity() {
            seats.send(player, &ServerMsg::Error("Too many players for this mode.".into()));
            return;
        }
        self.shot_history.clear();
        seats.broadcast(&ServerMsg::GameStart { rules: self.rules });
        self.phase = match self.rules.source {
            BoardSource::Versus => MatchPhase::Setup {
                locked: [false; 2],
                boards: [None, None],
            },
            BoardSource::Computer => {
                let n = seats.len();
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

    fn on_lock_board(&mut self, seats: &Seats, player: usize, board: Board) {
        if !matches!(self.phase, MatchPhase::Setup { .. }) {
            return;
        }
        if !validate_board(&board, &self.rules) {
            seats.send(player, &ServerMsg::Error("Invalid board.".into()));
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
            Outcome::Locked { other } => seats.send(other, &ServerMsg::OpponentLocked),
            Outcome::BothLocked { boards } => {
                seats.broadcast(&ServerMsg::OpponentLocked);
                if self.rules.turn_based() {
                    self.phase = MatchPhase::Playing {
                        boards,
                        current: 0,
                        rotated: false,
                    };
                    seats.send(0, &ServerMsg::YourTurn);
                    seats.send(1, &ServerMsg::OpponentTurn);
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
                    seats.broadcast(&ServerMsg::BeginPlay);
                }
            }
        }
    }

    fn valid_port(&self, port: u8) -> bool {
        (1..=4 * self.rules.size as u32).contains(&(port as u32))
    }

    fn on_shoot(&mut self, seats: &Seats, player: usize, port: u8) {
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
            Outcome::NotYourTurn => seats.send(player, &ServerMsg::Error("Not your turn.".into())),
            Outcome::InvalidPort => seats.send(player, &ServerMsg::Error("Invalid port.".into())),
            Outcome::Shot { other, exit, color } => {
                self.shot_history.push(ShotRecord {
                    player,
                    call: port,
                    exit,
                    color,
                });
                seats.send(player, &ServerMsg::BeamResult { call: port, exit, color });
                seats.send(other, &ServerMsg::OpponentShot { call: port, exit, color });
                seats.send(other, &ServerMsg::YourTurn);
            }
            Outcome::Solo { exit, color } => {
                self.shot_history.push(ShotRecord {
                    player,
                    call: port,
                    exit,
                    color,
                });
                seats.send(player, &ServerMsg::BeamResult { call: port, exit, color });
            }
        }
    }

    fn on_call_game(&mut self, seats: &Seats, player: usize, guess: Board) {
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
            Outcome::NotYourTurn => seats.send(
                player,
                &ServerMsg::Error("You can only call the game on your turn.".into()),
            ),
            Outcome::Correct { boards } => {
                let results = self.standings(seats);
                let own = vec![boards[0].clone(), boards[1].clone()];
                let targets = vec![boards[1].clone(), boards[0].clone()];
                for p in 0..2 {
                    // Each player sees the opponent's actual board, the answer.
                    seats.send(
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
                seats.send(player, &ServerMsg::GuessIncorrect);
                seats.send(other, &ServerMsg::OpponentCalledGame);
                seats.send(other, &ServerMsg::YourTurn);
            }
            Outcome::Solved { score } => {
                if race {
                    self.finish_independent(seats, Some(player));
                } else if self.async_ready(seats) {
                    // The last solver goes straight to the results.
                    self.finish_independent(seats, None);
                } else {
                    seats.send(player, &ServerMsg::YouFinished { score });
                    seats.send_others(
                        player,
                        &ServerMsg::PlayerFinished {
                            player: player as u8,
                        },
                    );
                }
            }
            Outcome::Missed => seats.send(player, &ServerMsg::GuessIncorrect),
        }
    }

    fn on_request_rotate(&mut self, seats: &Seats, player: usize, row: u8, col: u8) {
        if row >= self.rules.size || col >= self.rules.size {
            seats.send(player, &ServerMsg::Error("Invalid coordinates.".into()));
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
            Outcome::NotYourTurn => seats.send(
                player,
                &ServerMsg::Error("You can only flip a mirror on your turn.".into()),
            ),
            Outcome::AlreadyFlipped => seats.send(
                player,
                &ServerMsg::Error("You can only flip once per turn.".into()),
            ),
            Outcome::Done { accepted } => {
                seats.send(player, &ServerMsg::RotateResult { row, col, accepted });
                if accepted {
                    seats.send(other, &ServerMsg::YourMirrorRotated { row, col, accepted });
                } else {
                    // Turn passed to the opponent. Send YourTurn first, its
                    // client handler clears the last-mirror notice, then the
                    // flip-attempt notice so the opponent still sees it.
                    seats.send(other, &ServerMsg::YourTurn);
                    seats.send(other, &ServerMsg::YourMirrorRotated { row, col, accepted });
                }
            }
            Outcome::IndependentDone { accepted } => {
                seats.send(player, &ServerMsg::RotateResult { row, col, accepted })
            }
        }
    }

    fn on_play_again(&mut self, seats: &mut Seats) {
        if !matches!(self.phase, MatchPhase::Over { .. }) {
            return;
        }
        seats.drop_disconnected();
        self.shot_history.clear();
        self.phase = MatchPhase::Lobby;
        self.broadcast_lobby(seats);
    }
}

impl RoomGame for LightBeams {
    type ClientMsg = ClientMsg;

    fn capacity(&self) -> usize {
        self.rules.max_players()
    }

    fn in_lobby(&self) -> bool {
        matches!(self.phase, MatchPhase::Lobby)
    }

    fn is_over(&self) -> bool {
        matches!(self.phase, MatchPhase::Over { .. })
    }

    fn mode(&self) -> String {
        mode_label(&self.rules)
    }

    fn joined(&mut self, seats: &Seats, _player: usize) {
        self.broadcast_lobby(seats);
    }

    fn rejoined(&mut self, seats: &Seats, player: usize) {
        seats.send(player, &self.game_resync(seats, player));
        if self.in_lobby() {
            self.broadcast_lobby(seats);
        } else {
            seats.send_others(
                player,
                &ServerMsg::PlayerConnected {
                    player: player as u8,
                },
            );
        }
    }

    fn handle(&mut self, seats: &mut Seats, player: usize, msg: ClientMsg) {
        self.dispatch(seats, player, msg);
    }

    fn left(&mut self, seats: &Seats, player: usize) {
        if self.in_lobby() {
            self.broadcast_lobby(seats);
            return;
        }
        seats.send_others(
            player,
            &ServerMsg::PlayerDisconnected {
                player: player as u8,
            },
        );
        if self.async_ready(seats) {
            self.finish_independent(seats, None);
        }
    }

    fn pruned(&mut self, seats: &Seats) {
        self.broadcast_lobby(seats);
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

impl From<rooms::RoomSummary> for RoomInfo {
    fn from(r: rooms::RoomSummary) -> Self {
        Self {
            id: r.id,
            name: r.name,
            players: r.players,
            capacity: r.capacity,
            in_progress: r.in_progress,
            mode: r.mode,
        }
    }
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
    light_beams: Arc<Rooms<LightBeams>>,
    five_crowns: Arc<Rooms<FiveCrowns>>,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    pub fn new() -> Self {
        Self {
            light_beams: Arc::new(Rooms::new()),
            five_crowns: Arc::new(Rooms::new()),
        }
    }

    /// Spawn the background tasks that reap abandoned rooms.
    pub fn spawn_reaper(self: Arc<Self>) {
        self.light_beams.clone().spawn_reaper();
        self.five_crowns.clone().spawn_reaper();
    }

    pub async fn create_room(&self, id: String, name: String, passcode: String, rules: Rules) {
        self.light_beams
            .create(id, name, passcode, LightBeams::new(rules))
            .await;
    }

    pub async fn list_rooms(&self) -> Vec<RoomInfo> {
        let rooms = self.light_beams.list().await;
        rooms.into_iter().map(RoomInfo::from).collect()
    }

    pub async fn room_exists(&self, id: &str) -> bool {
        self.light_beams.exists(id).await
    }

    pub async fn create_five_crowns_room(&self, id: String, name: String, passcode: String) {
        self.five_crowns
            .create(id, name, passcode, FiveCrowns::new())
            .await;
    }

    pub async fn list_five_crowns_rooms(&self) -> Vec<RoomInfo> {
        let rooms = self.five_crowns.list().await;
        rooms.into_iter().map(RoomInfo::from).collect()
    }

    pub async fn five_crowns_room_exists(&self, id: &str) -> bool {
        self.five_crowns.exists(id).await
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
    let rooms = state.light_beams.clone();
    ws.on_upgrade(move |socket| rooms::serve(socket, rooms, room_id, passcode, query.token))
}

pub async fn five_crowns_ws_handler(
    ws: WebSocketUpgrade,
    Path((room_id, passcode)): Path<(String, String)>,
    Query(query): Query<WsQuery>,
    Extension(state): Extension<Arc<GameState>>,
) -> impl IntoResponse {
    let rooms = state.five_crowns.clone();
    ws.on_upgrade(move |socket| rooms::serve(socket, rooms, room_id, passcode, query.token))
}

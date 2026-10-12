//! Authoritative, rendering-independent game rules. Clients only receive `View`.
use rand::{seq::SliceRandom, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

pub type PlayerId = usize;
pub const MAX_PLAYERS: usize = 10;
pub const WINNING_SCORE: u32 = 500;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Color {
    Red,
    Yellow,
    Green,
    Blue,
}
impl Color {
    pub const ALL: [Self; 4] = [Self::Red, Self::Yellow, Self::Green, Self::Blue];
    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Face {
    Number(u8),
    Skip,
    Reverse,
    DrawTwo,
    Wild,
    WildFour,
    TargetTwo,
}
impl Face {
    pub fn label(self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::Skip => "SKIP".into(),
            Self::Reverse => "REV".into(),
            Self::DrawTwo => "+2".into(),
            Self::Wild => "WILD".into(),
            Self::WildFour => "+4".into(),
            Self::TargetTwo => "?".into(),
        }
    }
    pub fn wild(self) -> bool {
        matches!(self, Self::Wild | Self::WildFour | Self::TargetTwo)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub id: u16,
    pub color: Option<Color>,
    pub face: Face,
}
impl Card {
    pub fn points(self) -> u32 {
        match self.face {
            Face::Number(n) => n as u32,
            Face::Skip | Face::Reverse | Face::DrawTwo => 20,
            _ => 50,
        }
    }
    pub fn label(self) -> String {
        match self.color {
            Some(color) => format!("{} {}", color.name(), self.face.label()),
            None => self.face.label(),
        }
    }
}

/// 108 classic cards plus four wild, targeted +2 cards.
pub fn deck() -> Vec<Card> {
    let mut cards = Vec::with_capacity(112);
    let mut push = |color, face| {
        cards.push(Card {
            id: cards.len() as u16,
            color,
            face,
        });
    };
    for color in Color::ALL {
        push(Some(color), Face::Number(0));
        for _ in 0..2 {
            for n in 1..=9 {
                push(Some(color), Face::Number(n));
            }
            for face in [Face::Skip, Face::Reverse, Face::DrawTwo] {
                push(Some(color), face);
            }
        }
    }
    for _ in 0..4 {
        for face in [Face::Wild, Face::WildFour, Face::TargetTwo] {
            push(None, face);
        }
    }
    cards
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Start,
    Play {
        card: u16,
        color: Option<Color>,
        target: Option<PlayerId>,
        uno: bool,
    },
    Draw,
    Pass,
    AcceptFour,
    ChallengeFour,
    CatchUno {
        player: PlayerId,
    },
    DeclareUno,
    CheckUno {
        player: PlayerId,
    },
    ChooseOpeningColor {
        color: Color,
    },
    NextRound,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    Lobby,
    OpeningColor,
    Playing,
    RoundOver { winner: PlayerId, points: u32 },
    MatchOver { winner: PlayerId },
}

#[derive(Clone, Debug)]
pub struct Player {
    pub name: String,
    pub hand: Vec<Card>,
    pub score: u32,
    pub connected: bool,
}
#[derive(Clone, Debug)]
struct PendingFour {
    actor: PlayerId,
    victim: PlayerId,
    illegal: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerView {
    pub name: String,
    pub cards: usize,
    pub score: u32,
    pub connected: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FourView {
    pub actor: PlayerId,
    pub victim: PlayerId,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct View {
    pub you: PlayerId,
    pub host: PlayerId,
    pub players: Vec<PlayerView>,
    pub hand: Vec<Card>,
    pub phase: Phase,
    pub turn: PlayerId,
    pub turn_id: u64,
    pub direction: i8,
    pub top: Option<Card>,
    pub active_color: Color,
    pub deck_count: usize,
    pub playable: Vec<u16>,
    pub can_draw: bool,
    pub can_pass: bool,
    pub pending_four: Option<FourView>,
    pub pending_two: usize,
    pub vulnerable: Option<PlayerId>,
    /// Last declared UNO, available for the next player to verify.
    #[serde(default)]
    pub declared_uno: Option<PlayerId>,
    pub paused: bool,
    pub round: u32,
    pub log: Vec<String>,
}

pub struct Game {
    pub players: Vec<Player>,
    pub phase: Phase,
    pub host: PlayerId,
    turn: PlayerId,
    turn_id: u64,
    direction: i8,
    active_color: Color,
    stock: Vec<Card>,
    discard: Vec<Card>,
    pending_four: Option<PendingFour>,
    pending_two: usize,
    vulnerable: Option<PlayerId>,
    // Keep validity at declaration time, not after subsequent penalties.
    declared_uno: Option<(PlayerId, bool)>,
    // A +2 chain can empty several hands before its debt is resolved.
    finishers: Vec<PlayerId>,
    dealer: PlayerId,
    round: u32,
    log: Vec<String>,
    rng: rand::rngs::StdRng,
}
impl Default for Game {
    fn default() -> Self {
        Self::new(rand::thread_rng().gen())
    }
}
impl Game {
    pub fn new(seed: u64) -> Self {
        Self {
            players: Vec::new(),
            phase: Phase::Lobby,
            host: 0,
            turn: 0,
            turn_id: 0,
            direction: 1,
            active_color: Color::Red,
            stock: Vec::new(),
            discard: Vec::new(),
            pending_four: None,
            pending_two: 0,
            vulnerable: None,
            declared_uno: None,
            finishers: Vec::new(),
            dealer: 0,
            round: 0,
            log: Vec::new(),
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }
    pub fn add_player(&mut self, name: String) -> Result<PlayerId, String> {
        if self.phase != Phase::Lobby {
            return Err("The game has already started.".into());
        }
        if self.players.len() >= MAX_PLAYERS {
            return Err("The table is full (10 players).".into());
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 20 || name.chars().any(char::is_control) {
            return Err("Use a name of 1-20 printable characters.".into());
        }
        let id = self.players.len();
        self.players.push(Player {
            name: name.into(),
            hand: Vec::new(),
            score: 0,
            connected: true,
        });
        self.note(format!("{name} joined the table."));
        Ok(id)
    }
    pub fn set_connected(&mut self, player: PlayerId, connected: bool) {
        if let Some(p) = self.players.get_mut(player) {
            p.connected = connected;
            let name = p.name.clone();
            self.note(format!(
                "{name} {}.",
                if connected {
                    "reconnected"
                } else {
                    "disconnected; waiting for them"
                }
            ));
        }
    }
    fn note(&mut self, message: String) {
        self.log.push(message);
        if self.log.len() > 10 {
            self.log.remove(0);
        }
    }
    fn next(&self, player: PlayerId, steps: usize) -> PlayerId {
        (player as isize + self.direction as isize * steps as isize)
            .rem_euclid(self.players.len() as isize) as usize
    }
    fn advance(&mut self, steps: usize) {
        self.turn = self.next(self.turn, steps);
        self.turn_id += 1;
    }
    fn paused(&self) -> bool {
        self.players.iter().any(|p| !p.connected)
    }
    fn matches(&self, card: Card) -> bool {
        card.face.wild()
            || card.color == Some(self.active_color)
            || self.discard.last().is_some_and(|top| top.face == card.face)
    }
    fn take(&mut self) -> Option<Card> {
        if self.stock.is_empty() && self.discard.len() > 1 {
            let top = self.discard.pop().unwrap();
            self.stock.append(&mut self.discard);
            self.stock.shuffle(&mut self.rng);
            self.discard.push(top);
        }
        self.stock.pop()
    }
    fn give(&mut self, player: PlayerId, count: usize) -> usize {
        let mut actual = 0;
        for _ in 0..count {
            if let Some(card) = self.take() {
                self.players[player].hand.push(card);
                actual += 1;
            } else {
                break;
            }
        }
        if actual > 0 {
            self.finishers.retain(|p| *p != player);
        }
        if self.players[player].hand.len() != 1 && self.vulnerable == Some(player) {
            self.vulnerable = None;
        }
        actual
    }
    fn begin_round(&mut self) {
        self.round += 1;
        self.turn_id += 1;
        self.direction = 1;
        self.pending_four = None;
        self.pending_two = 0;
        self.vulnerable = None;
        self.declared_uno = None;
        self.finishers.clear();
        self.discard.clear();
        self.stock = deck();
        self.stock.shuffle(&mut self.rng);
        for p in &mut self.players {
            p.hand.clear();
        }
        for _ in 0..7 {
            for p in 0..self.players.len() {
                self.give(p, 1);
            }
        }
        // A Draw Four or custom ? cannot be the opening discard.
        let top = loop {
            let card = self.stock.pop().unwrap();
            if matches!(card.face, Face::WildFour | Face::TargetTwo) {
                self.stock.insert(0, card);
                self.stock.shuffle(&mut self.rng);
            } else {
                break card;
            }
        };
        self.discard.push(top);
        self.active_color = top.color.unwrap_or(Color::Red);
        self.turn = self.next(self.dealer, 1);
        self.phase = Phase::Playing;
        match top.face {
            Face::Skip => self.advance(1),
            Face::Reverse if self.players.len() > 2 => {
                self.direction = -1;
                self.turn = self.dealer;
            }
            Face::DrawTwo => self.pending_two = 2,
            Face::Wild => self.phase = Phase::OpeningColor,
            _ => {}
        }
        self.note(format!(
            "Round {}. Opening card: {}.",
            self.round,
            top.label()
        ));
    }
    fn finish_if_won(&mut self) {
        if self.pending_four.is_some()
            || self.pending_two > 0
            || self
                .declared_uno
                .is_some_and(|(actor, valid)| !valid && actor != self.turn)
        {
            return;
        }
        if let Some(&winner) = self.finishers.first() {
            let points = self
                .players
                .iter()
                .flat_map(|p| &p.hand)
                .map(|c| c.points())
                .sum();
            self.players[winner].score += points;
            self.vulnerable = None;
            self.declared_uno = None;
            self.note(format!(
                "{} wins the round (+{points} points).",
                self.players[winner].name
            ));
            self.phase = if self.players[winner].score >= WINNING_SCORE {
                Phase::MatchOver { winner }
            } else {
                Phase::RoundOver { winner, points }
            };
        }
    }
    /// Invalid requests never consume the next player's opportunity to check UNO.
    fn close_uno_window(&mut self) -> bool {
        self.vulnerable = None;
        self.declared_uno = None;
        self.finish_if_won();
        self.phase != Phase::Playing
    }
    pub fn act(&mut self, player: PlayerId, action: Action) -> Result<(), String> {
        if player >= self.players.len() {
            return Err("Unknown player.".into());
        }
        if self.paused() {
            return Err("Waiting for disconnected players to reconnect.".into());
        }
        if matches!(action, Action::Start) {
            if player != self.host || self.phase != Phase::Lobby || self.players.len() < 2 {
                return Err("Only the host can start, with at least two players.".into());
            }
            self.dealer = self.rng.gen_range(0..self.players.len());
            self.begin_round();
            return Ok(());
        }
        if matches!(action, Action::NextRound) {
            if player != self.host
                || !matches!(
                    self.phase,
                    Phase::RoundOver { .. } | Phase::MatchOver { .. }
                )
            {
                return Err("Only the host can begin the next round after a round ends.".into());
            }
            if matches!(self.phase, Phase::MatchOver { .. }) {
                for p in &mut self.players {
                    p.score = 0;
                }
                self.round = 0;
            }
            self.dealer = (self.dealer + 1) % self.players.len();
            self.begin_round();
            return Ok(());
        }
        if let Action::ChooseOpeningColor { color } = action {
            if self.phase != Phase::OpeningColor || player != self.turn {
                return Err("It is not your opening color choice.".into());
            }
            self.active_color = color;
            self.phase = Phase::Playing;
            self.note(format!(
                "{} chose {}.",
                self.players[player].name,
                color.name()
            ));
            return Ok(());
        }
        if self.phase != Phase::Playing {
            return Err("The round is not in play.".into());
        }
        if let Action::CheckUno { player: target } = action {
            let Some((actor, valid)) = self.declared_uno else {
                return Err("There is no UNO declaration to check.".into());
            };
            if player != self.turn || target != actor || player == actor {
                return Err("Only the next player can check that UNO.".into());
            }
            self.declared_uno = None;
            if valid {
                self.note(format!(
                    "{} checked {}: UNO was valid. No penalty.",
                    self.players[player].name, self.players[actor].name
                ));
            } else {
                let count = self.give(actor, 2);
                self.note(format!(
                    "{} checked {}: false UNO! (+{count})",
                    self.players[player].name, self.players[actor].name
                ));
            }
            self.finish_if_won();
            return Ok(());
        }
        if let Action::CatchUno { player: target } = action {
            if player != self.turn || target == player || self.vulnerable != Some(target) {
                return Err("That player cannot be caught now.".into());
            }
            let count = self.give(target, 2);
            self.vulnerable = None;
            self.note(format!(
                "{} caught {} not saying UNO! (+{count})",
                self.players[player].name, self.players[target].name
            ));
            return Ok(());
        }
        if matches!(action, Action::DeclareUno) {
            if self.vulnerable != Some(player) {
                return Err("There is no missed UNO to declare.".into());
            }
            self.vulnerable = None;
            self.declared_uno = Some((player, true));
            self.note(format!(
                "{}: UNO! (before being caught)",
                self.players[player].name
            ));
            return Ok(());
        }
        if player != self.turn {
            return Err("It is not your turn.".into());
        }
        if let Some(pending) = self.pending_four.clone() {
            if !matches!(action, Action::AcceptFour | Action::ChallengeFour) {
                return Err("Accept or challenge the Wild Draw Four first.".into());
            }
            self.close_uno_window();
            self.pending_four = None;
            match action {
                Action::ChallengeFour if pending.illegal => {
                    let count = self.give(pending.actor, 4);
                    self.note(format!("Challenge successful: {} had the active color and draws {count}. Your turn continues.", self.players[pending.actor].name));
                }
                Action::ChallengeFour => {
                    let count = self.give(player, 6);
                    self.note(format!(
                        "Challenge failed: {} draws {count} and loses their turn.",
                        self.players[player].name
                    ));
                    self.advance(1);
                }
                _ => {
                    let count = self.give(player, 4);
                    self.note(format!(
                        "{} accepts +{count} and loses their turn.",
                        self.players[player].name
                    ));
                    self.advance(1);
                }
            }
            self.finish_if_won();
            return Ok(());
        }
        match action {
            Action::Play {
                card,
                color,
                target,
                uno,
            } => {
                let index = self.players[player]
                    .hand
                    .iter()
                    .position(|c| c.id == card)
                    .ok_or("You do not hold that card.")?;
                let card = self.players[player].hand[index];
                let valid = if self.pending_two > 0 {
                    card.face == Face::DrawTwo
                } else {
                    self.matches(card)
                };
                if !valid {
                    return Err("That card cannot be played. Your turn continues.".into());
                }
                if card.face.wild() && color.is_none() {
                    return Err("Choose a color for this wild card.".into());
                }
                if card.face == Face::TargetTwo
                    && !target.is_some_and(|p| p < self.players.len() && p != player)
                {
                    return Err("Choose another player for the ? card.".into());
                }
                let illegal = card.face == Face::WildFour
                    && self.players[player]
                        .hand
                        .iter()
                        .any(|c| c.color == Some(self.active_color));
                if self.close_uno_window() {
                    return Ok(());
                }
                self.players[player].hand.remove(index);
                if self.players[player].hand.is_empty() {
                    self.finishers.push(player);
                }
                self.discard.push(card);
                self.active_color = card.color.or(color).unwrap();
                let name = self.players[player].name.clone();
                self.note(format!(
                    "{name} played {}{}.",
                    card.label(),
                    if card.face.wild() {
                        format!(" -> {}", self.active_color.name())
                    } else {
                        String::new()
                    }
                ));
                let real_uno = self.players[player].hand.len() == 1;
                if uno {
                    self.declared_uno = Some((player, real_uno));
                    self.note(format!("{name}: UNO!"));
                } else if real_uno {
                    self.vulnerable = Some(player);
                }
                match card.face {
                    Face::Skip => self.advance(2),
                    Face::Reverse => {
                        if self.players.len() > 2 {
                            self.direction *= -1;
                        }
                        self.advance(1);
                    }
                    Face::DrawTwo => {
                        self.pending_two += 2;
                        self.advance(1);
                        self.note(format!(
                            "{}: stack +2 or draw {} and pass.",
                            self.players[self.turn].name, self.pending_two
                        ));
                    }
                    Face::WildFour => {
                        let victim = self.next(player, 1);
                        self.pending_four = Some(PendingFour {
                            actor: player,
                            victim,
                            illegal,
                        });
                        self.advance(1);
                    }
                    Face::TargetTwo => {
                        let victim = target.unwrap();
                        let count = self.give(victim, 2);
                        self.note(format!(
                            "{} was chosen to draw {count} (not skipped).",
                            self.players[victim].name
                        ));
                        self.advance(1);
                    }
                    _ => self.advance(1),
                }
                self.finish_if_won();
            }
            Action::Draw => {
                if self.pending_two > 0 {
                    self.close_uno_window();
                    let penalty = std::mem::take(&mut self.pending_two);
                    let count = self.give(player, penalty);
                    self.note(format!(
                        "{} accepts +{count}; turn passed.",
                        self.players[player].name
                    ));
                    self.advance(1);
                    self.finish_if_won();
                    return Ok(());
                }
                if self.stock.is_empty() && self.discard.len() <= 1 {
                    return Err(
                        "The deck is empty. Play a matching card to finish your turn.".into(),
                    );
                }
                if self.close_uno_window() {
                    return Ok(());
                }
                self.give(player, 1);
                self.note(format!("{} drew one card.", self.players[player].name));
            }
            // Retained in the wire protocol only to reject older clients explicitly.
            Action::Pass => return Err("Passing is not allowed. Play a matching card.".into()),
            _ => return Err("That action is not available now.".into()),
        }
        Ok(())
    }
    pub fn view(&self, you: PlayerId) -> View {
        let active = !self.paused()
            && self.phase == Phase::Playing
            && self.turn == you
            && self.pending_four.is_none();
        let hand = self
            .players
            .get(you)
            .map(|p| p.hand.clone())
            .unwrap_or_default();
        View {
            you,
            host: self.host,
            players: self
                .players
                .iter()
                .map(|p| PlayerView {
                    name: p.name.clone(),
                    cards: p.hand.len(),
                    score: p.score,
                    connected: p.connected,
                })
                .collect(),
            playable: if active {
                hand.iter()
                    .filter(|c| {
                        if self.pending_two > 0 {
                            c.face == Face::DrawTwo
                        } else {
                            self.matches(**c)
                        }
                    })
                    .map(|c| c.id)
                    .collect()
            } else {
                Vec::new()
            },
            can_draw: active,
            can_pass: false,
            hand,
            phase: self.phase.clone(),
            turn: self.turn,
            turn_id: self.turn_id,
            direction: self.direction,
            top: self.discard.last().copied(),
            active_color: self.active_color,
            deck_count: self.stock.len(),
            pending_four: self.pending_four.as_ref().map(|p| FourView {
                actor: p.actor,
                victim: p.victim,
            }),
            pending_two: self.pending_two,
            vulnerable: self.vulnerable,
            declared_uno: self.declared_uno.map(|(actor, _)| actor),
            paused: self.paused(),
            round: self.round,
            log: self.log.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(id: u16, color: Option<Color>, face: Face) -> Card {
        Card { id, color, face }
    }
    fn red(id: u16, n: u8) -> Card {
        card(id, Some(Color::Red), Face::Number(n))
    }
    fn blue(id: u16, n: u8) -> Card {
        card(id, Some(Color::Blue), Face::Number(n))
    }
    fn setup(n: usize) -> Game {
        let mut g = Game::new(42);
        for i in 0..n {
            g.add_player(format!("P{i}")).unwrap();
        }
        g.phase = Phase::Playing;
        g.discard = vec![red(100, 5)];
        g.stock = (30..60).map(|id| blue(id, 9)).collect();
        for i in 0..n {
            g.players[i].hand = vec![red(i as u16 * 3, 3), blue(i as u16 * 3 + 1, 1)];
        }
        g
    }
    fn play(g: &mut Game, p: usize, id: u16) {
        g.act(
            p,
            Action::Play {
                card: id,
                color: Some(Color::Green),
                target: Some(1),
                uno: g.players[p].hand.len() == 2,
            },
        )
        .unwrap();
    }
    #[test]
    fn deck_composition() {
        let d = deck();
        assert_eq!(d.len(), 112);
        assert_eq!(d.iter().filter(|c| c.face == Face::TargetTwo).count(), 4);
        let ids: std::collections::HashSet<_> = d.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 112);
        for color in Color::ALL {
            assert_eq!(d.iter().filter(|c| c.color == Some(color)).count(), 25);
        }
    }
    #[test]
    fn draw_takes_exactly_one_and_never_passes() {
        let mut g = setup(2);
        g.players[0].hand = vec![blue(10, 2)];
        g.stock = vec![red(20, 7), blue(21, 4), blue(22, 6)];
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand, vec![blue(10, 2), blue(22, 6)]);
        assert!(g.view(0).playable.is_empty());
        assert!(g.view(0).can_draw);
        assert!(!g.view(0).can_pass);
        assert!(g.act(0, Action::Pass).is_err());
        assert_eq!(g.turn, 0);
    }
    #[test]
    fn voluntary_draw_takes_one_and_preserves_all_matching_cards() {
        let mut g = setup(2);
        assert!(g.act(1, Action::Draw).is_err());
        assert!(g.act(0, Action::Pass).is_err());
        g.stock = vec![red(90, 8), blue(91, 2), blue(92, 3)];
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 3);
        assert_eq!(g.turn, 0);
        assert_eq!(g.view(0).playable, vec![0]);
        g.stock.push(red(93, 9));
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 4);
        assert_eq!(g.view(0).playable, vec![0, 93]);
        play(&mut g, 0, 93);
        assert_eq!(g.turn, 1);
    }
    #[test]
    fn late_uno_declaration_before_catch_is_allowed() {
        let mut g = setup(3);
        g.act(
            0,
            Action::Play {
                card: 0,
                color: None,
                target: None,
                uno: false,
            },
        )
        .unwrap();
        assert!(g.act(2, Action::DeclareUno).is_err());
        g.act(0, Action::DeclareUno).unwrap();
        assert!(g.act(1, Action::CatchUno { player: 0 }).is_err());
        assert_eq!(g.players[0].hand.len(), 1);
    }
    #[test]
    fn target_two_chooses_opponent_without_skipping() {
        let mut g = setup(3);
        g.players[0].hand[0] = card(0, None, Face::TargetTwo);
        g.act(
            0,
            Action::Play {
                card: 0,
                color: Some(Color::Blue),
                target: Some(2),
                uno: true,
            },
        )
        .unwrap();
        assert_eq!(g.players[2].hand.len(), 4);
        assert_eq!(g.players[1].hand.len(), 2);
        assert_eq!(g.turn, 1);
        assert_eq!(g.active_color, Color::Blue);
    }
    #[test]
    fn invalid_target_and_missing_color_are_atomic() {
        let mut g = setup(2);
        g.players[0].hand[0] = card(0, None, Face::TargetTwo);
        for (color, target) in [
            (None, Some(1)),
            (Some(Color::Red), Some(0)),
            (Some(Color::Red), Some(99)),
        ] {
            assert!(g
                .act(
                    0,
                    Action::Play {
                        card: 0,
                        color,
                        target,
                        uno: false
                    }
                )
                .is_err());
            assert_eq!(g.players[0].hand.len(), 2);
            assert_eq!(g.discard.len(), 1);
        }
    }
    #[test]
    fn reverse_two_players_passes_without_changing_direction() {
        let mut g = setup(2);
        g.players[0].hand[0] = card(0, Some(Color::Red), Face::Reverse);
        play(&mut g, 0, 0);
        assert_eq!(g.turn, 1);
        assert_eq!(g.direction, 1);
    }
    #[test]
    fn turn_id_changes_on_two_player_skip_but_not_on_draw() {
        let mut g = setup(2);
        let before = g.view(0).turn_id;
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.view(0).turn_id, before);
        g.players[0].hand[0] = card(0, Some(Color::Red), Face::Skip);
        play(&mut g, 0, 0);
        assert_eq!(g.turn, 0);
        assert_ne!(g.view(0).turn_id, before);
    }
    #[test]
    fn reverse_three_players_and_skip() {
        let mut g = setup(3);
        g.players[0].hand[0] = card(0, Some(Color::Red), Face::Reverse);
        play(&mut g, 0, 0);
        assert_eq!(g.turn, 2);
        g.players[2].hand[0] = card(6, Some(Color::Red), Face::Skip);
        play(&mut g, 2, 6);
        assert_eq!(g.turn, 0);
    }
    #[test]
    fn draw_two_stacks_across_colors_and_can_be_accepted() {
        let mut g = setup(3);
        g.players[0].hand[0] = card(0, Some(Color::Red), Face::DrawTwo);
        g.players[1].hand[0] = card(3, Some(Color::Blue), Face::DrawTwo);
        g.players[2].hand[0] = card(6, Some(Color::Green), Face::DrawTwo);
        for (player, id, debt) in [(0, 0, 2), (1, 3, 4), (2, 6, 6)] {
            play(&mut g, player, id);
            assert_eq!(g.pending_two, debt);
            assert_eq!(g.turn, (player + 1) % 3);
        }
        assert_eq!(g.players[0].hand.len(), 1);
        assert!(g.view(0).playable.is_empty());
        assert!(!g.view(0).can_pass);
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 7);
        assert_eq!(g.pending_two, 0);
        assert_eq!(g.turn, 1);
    }
    #[test]
    fn invalid_card_is_atomic_and_does_not_end_turn() {
        for debt in [0, 4] {
            let mut g = setup(3);
            g.pending_two = debt;
            let before = serde_json::to_string(&g.view(0)).unwrap();
            for _ in 0..3 {
                assert!(g
                    .act(
                        0,
                        Action::Play {
                            card: 1,
                            color: None,
                            target: None,
                            uno: false
                        }
                    )
                    .is_err());
                assert_eq!(serde_json::to_string(&g.view(0)).unwrap(), before);
            }
        }
    }
    #[test]
    fn final_draw_two_waits_for_penalty_before_scoring() {
        let mut g = setup(2);
        g.players[0].hand = vec![card(0, Some(Color::Red), Face::DrawTwo)];
        play(&mut g, 0, 0);
        assert_eq!(g.phase, Phase::Playing);
        g.act(1, Action::Draw).unwrap();
        assert_eq!(g.players[1].hand.len(), 4);
        assert_eq!(
            g.phase,
            Phase::RoundOver {
                winner: 0,
                points: 22
            }
        );
    }
    #[test]
    fn stacked_final_cards_award_first_empty_hand_not_lowest_seat() {
        for direction in [-1, 1] {
            let mut g = setup(3);
            g.direction = direction;
            g.turn = 2;
            let second = g.next(2, 1);
            let victim = g.next(second, 1);
            for player in [2, second] {
                g.players[player].hand = vec![card(player as u16, Some(Color::Red), Face::DrawTwo)];
                play(&mut g, player, player as u16);
                assert_eq!(g.phase, Phase::Playing);
            }
            g.act(victim, Action::Draw).unwrap();
            assert!(matches!(g.phase, Phase::RoundOver { winner: 2, .. }));
            assert_eq!(g.players[second].score, 0);
            g.act(g.host, Action::NextRound).unwrap();
            assert!(g.finishers.is_empty());
        }
    }
    #[test]
    fn final_card_chain_can_return_penalty_to_first_finisher() {
        let mut g = setup(2);
        for player in 0..2 {
            g.players[player].hand = vec![card(player as u16, Some(Color::Red), Face::DrawTwo)];
            play(&mut g, player, player as u16);
        }
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 4);
        assert!(matches!(g.phase, Phase::RoundOver { winner: 1, .. }));
    }
    #[test]
    fn exhausted_final_chain_keeps_first_finisher() {
        let mut g = setup(2);
        for player in [1, 0] {
            g.turn = player;
            g.players[player].hand = vec![card(player as u16, Some(Color::Red), Face::DrawTwo)];
            play(&mut g, player, player as u16);
        }
        g.stock.clear();
        g.discard.drain(..g.discard.len() - 1);
        g.act(1, Action::Draw).unwrap();
        assert!(matches!(g.phase, Phase::RoundOver { winner: 1, .. }));
    }
    #[test]
    fn penalty_can_be_accepted_even_with_a_draw_two_in_hand() {
        let mut g = setup(2);
        g.players[0].hand[0] = card(0, Some(Color::Red), Face::DrawTwo);
        g.players[1].hand[0] = card(3, Some(Color::Blue), Face::DrawTwo);
        play(&mut g, 0, 0);
        assert_eq!(g.view(1).playable, vec![3]);
        assert!(g.view(1).can_draw);
        g.act(1, Action::Draw).unwrap();
        assert_eq!(g.players[1].hand.len(), 4);
        assert_eq!(g.pending_two, 0);
        assert_eq!(g.turn, 0);
    }
    #[test]
    fn matching_old_card_can_be_played_after_drawing() {
        let mut g = setup(2);
        g.stock.push(red(90, 8));
        g.act(0, Action::Draw).unwrap();
        play(&mut g, 0, 0);
        assert_eq!(g.players[0].hand.len(), 2);
        assert!(!g.players[0].hand.contains(&red(0, 3)));
        assert_eq!(g.discard, vec![red(100, 5), red(0, 3)]);
        assert_eq!(g.turn, 1);
    }
    #[test]
    fn opening_reverse_has_no_effect_with_two_players() {
        for seed in 0..500 {
            let mut g = Game::new(seed);
            g.add_player("A".into()).unwrap();
            g.add_player("B".into()).unwrap();
            g.dealer = 0;
            g.begin_round();
            if g.discard[0].face == Face::Reverse {
                assert_eq!(g.turn, 1);
                assert_eq!(g.direction, 1);
                return;
            }
        }
        panic!("No opening Reverse in seeded deals");
    }
    #[test]
    fn four_challenge_success_and_failure() {
        for illegal in [false, true] {
            let mut g = setup(3);
            g.players[0].hand = vec![
                card(0, None, Face::WildFour),
                if illegal { red(1, 2) } else { blue(1, 2) },
            ];
            play(&mut g, 0, 0);
            assert!(g.view(1).playable.is_empty());
            assert!(g.act(1, Action::Draw).is_err());
            g.act(1, Action::ChallengeFour).unwrap();
            assert_eq!(g.players[0].hand.len(), if illegal { 5 } else { 1 });
            assert_eq!(g.players[1].hand.len(), if illegal { 2 } else { 8 });
            assert_eq!(g.turn, if illegal { 1 } else { 2 });
        }
    }
    #[test]
    fn accept_four_and_final_card_penalty_counts_in_score() {
        let mut g = setup(2);
        g.players[0].hand = vec![card(0, None, Face::WildFour)];
        play(&mut g, 0, 0);
        assert_eq!(g.phase, Phase::Playing);
        g.act(1, Action::AcceptFour).unwrap();
        assert_eq!(g.players[1].hand.len(), 6);
        assert_eq!(
            g.phase,
            Phase::RoundOver {
                winner: 0,
                points: 40
            }
        );
    }
    #[test]
    fn uno_catch_window_and_declaration() {
        let mut g = setup(3);
        g.act(
            0,
            Action::Play {
                card: 0,
                color: None,
                target: None,
                uno: false,
            },
        )
        .unwrap();
        assert_eq!(g.vulnerable, Some(0));
        assert!(g.act(0, Action::CatchUno { player: 0 }).is_err());
        assert!(g.act(2, Action::CatchUno { player: 0 }).is_err());
        g.act(1, Action::CatchUno { player: 0 }).unwrap();
        assert_eq!(g.players[0].hand.len(), 3);
        assert!(g.act(1, Action::CatchUno { player: 0 }).is_err());
        play(&mut g, 1, 3);
        assert_eq!(g.vulnerable, None);
    }
    fn declare_with_play(g: &mut Game) {
        g.act(
            0,
            Action::Play {
                card: 0,
                color: Some(Color::Red),
                target: Some(1),
                uno: true,
            },
        )
        .unwrap();
    }
    #[test]
    fn checking_uno_penalizes_only_false_declarations_once() {
        for valid in [true, false] {
            let mut g = setup(3);
            if !valid {
                g.players[0].hand.push(blue(90, 2));
            }
            declare_with_play(&mut g);
            let before = g.players[0].hand.len();
            let turn_id = g.turn_id;
            assert_eq!(g.view(1).declared_uno, Some(0));
            assert_eq!(g.view(1).vulnerable, None);
            assert!(g.act(0, Action::CheckUno { player: 0 }).is_err());
            assert!(g.act(2, Action::CheckUno { player: 0 }).is_err());
            assert!(g.act(1, Action::CheckUno { player: usize::MAX }).is_err());
            assert!(g.act(1, Action::CatchUno { player: 0 }).is_err());
            g.act(1, Action::CheckUno { player: 0 }).unwrap();
            assert_eq!(g.players[0].hand.len(), before + if valid { 0 } else { 2 });
            assert_eq!(g.players[1].hand.len(), 2);
            assert_eq!(g.turn_id, turn_id);
            assert_eq!(g.turn, 1);
            assert!(g.view(1).declared_uno.is_none());
            assert!(g.act(1, Action::CheckUno { player: 0 }).is_err());
        }
    }
    #[test]
    fn uno_checks_follow_skip_and_reverse_not_seat_order() {
        for face in [Face::Skip, Face::Reverse] {
            let mut g = setup(4);
            g.players[0].hand[0].face = face;
            g.players[0].hand.push(blue(90, 2));
            declare_with_play(&mut g);
            assert!(g.act(1, Action::CheckUno { player: 0 }).is_err());
            let next = g.turn;
            g.act(next, Action::CheckUno { player: 0 }).unwrap();
            assert_eq!(g.players[0].hand.len(), 4);
            assert_eq!(g.turn, next);
        }
    }
    #[test]
    fn invalid_actions_and_pause_preserve_declared_uno() {
        let mut g = setup(3);
        declare_with_play(&mut g);
        for action in [
            Action::Pass,
            Action::Play {
                card: u16::MAX,
                color: None,
                target: None,
                uno: false,
            },
            Action::Play {
                card: 4,
                color: None,
                target: None,
                uno: false,
            },
            Action::AcceptFour,
        ] {
            assert!(g.act(1, action).is_err());
            assert_eq!(g.view(1).declared_uno, Some(0));
        }
        assert!(g.act(2, Action::Draw).is_err());
        g.stock.clear();
        g.discard = vec![red(100, 3)];
        assert!(g.act(1, Action::Draw).is_err());
        g.set_connected(2, false);
        assert!(g.act(1, Action::CheckUno { player: 0 }).is_err());
        g.set_connected(2, true);
        assert_eq!(g.view(1).declared_uno, Some(0));
        g.act(1, Action::CheckUno { player: 0 }).unwrap();
    }
    #[test]
    fn declared_uno_expires_on_valid_draw_play_and_four_decision() {
        for mode in 0..3 {
            let mut g = setup(3);
            if mode == 2 {
                g.players[0].hand[0] = card(0, None, Face::WildFour);
            }
            declare_with_play(&mut g);
            let action = match mode {
                0 => Action::Draw,
                1 => Action::Play {
                    card: 3,
                    color: None,
                    target: None,
                    uno: false,
                },
                _ => Action::AcceptFour,
            };
            g.act(1, action).unwrap();
            assert!(g.view(1).declared_uno.is_none());
            assert!(g.act(g.turn, Action::CheckUno { player: 0 }).is_err());
        }
    }
    #[test]
    fn false_uno_can_be_checked_before_a_four_decision() {
        let mut g = setup(3);
        g.players[0].hand[0] = card(0, None, Face::WildFour);
        g.players[0].hand.push(blue(90, 2));
        declare_with_play(&mut g);
        g.act(1, Action::CheckUno { player: 0 }).unwrap();
        assert_eq!(g.players[0].hand.len(), 4);
        assert!(g.pending_four.is_some());
        g.act(1, Action::AcceptFour).unwrap();
        assert_eq!(g.players[1].hand.len(), 6);
    }
    #[test]
    fn false_uno_on_last_card_waits_for_check_or_next_valid_action() {
        for check in [false, true] {
            let mut g = setup(3);
            g.players[0].hand.truncate(1);
            declare_with_play(&mut g);
            assert_eq!(g.phase, Phase::Playing);
            assert!(g.act(1, Action::Pass).is_err());
            g.act(
                1,
                if check {
                    Action::CheckUno { player: 0 }
                } else {
                    Action::Draw
                },
            )
            .unwrap();
            if check {
                assert_eq!(g.players[0].hand.len(), 2);
                assert_eq!(g.phase, Phase::Playing);
                assert!(g.finishers.is_empty());
            } else {
                assert!(matches!(g.phase, Phase::RoundOver { winner: 0, .. }));
                assert_eq!(g.players[1].hand.len(), 2);
            }
        }
    }
    #[test]
    fn late_declaration_is_checkable_and_serializes() {
        let mut g = setup(3);
        g.act(
            0,
            Action::Play {
                card: 0,
                color: None,
                target: None,
                uno: false,
            },
        )
        .unwrap();
        g.act(0, Action::DeclareUno).unwrap();
        let wire = serde_json::to_string(&g.view(1)).unwrap();
        let snapshot: View = serde_json::from_str(&wire).unwrap();
        assert_eq!(snapshot.declared_uno, Some(0));
        assert_eq!(snapshot.vulnerable, None);
        g.act(1, Action::CheckUno { player: 0 }).unwrap();
        assert_eq!(g.players[0].hand.len(), 1);
    }
    #[test]
    fn uno_window_closes_on_next_valid_action() {
        let mut g = setup(3);
        g.act(
            0,
            Action::Play {
                card: 0,
                color: None,
                target: None,
                uno: false,
            },
        )
        .unwrap();
        assert!(g.act(1, Action::Pass).is_err());
        assert_eq!(g.vulnerable, Some(0));
        play(&mut g, 1, 3);
        assert!(g.act(2, Action::CatchUno { player: 0 }).is_err());
    }
    #[test]
    fn malformed_requests_and_wrong_cards_preserve_uno_window() {
        let mut g = setup(3);
        g.act(
            0,
            Action::Play {
                card: 0,
                color: None,
                target: None,
                uno: false,
            },
        )
        .unwrap();
        for (player, card) in [(1, u16::MAX), (2, 6)] {
            assert!(g
                .act(
                    player,
                    Action::Play {
                        card,
                        color: None,
                        target: None,
                        uno: false
                    }
                )
                .is_err());
            assert_eq!(g.vulnerable, Some(0));
        }
        g.players[1].hand.push(card(90, None, Face::Wild));
        assert!(g
            .act(
                1,
                Action::Play {
                    card: 90,
                    color: None,
                    target: None,
                    uno: false
                }
            )
            .is_err());
        assert_eq!(g.vulnerable, Some(0));
        assert!(g
            .act(
                1,
                Action::Play {
                    card: 4,
                    color: None,
                    target: None,
                    uno: false
                }
            )
            .is_err());
        assert_eq!(g.vulnerable, Some(0));
        assert_eq!(g.turn, 1);
        assert_eq!(g.players[1].hand.len(), 3);
        g.act(1, Action::CatchUno { player: 0 }).unwrap();
    }
    #[test]
    fn draw_does_not_skip_a_challengeable_four() {
        let mut g = setup(2);
        g.stock = vec![red(90, 8), card(91, None, Face::WildFour)];
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 3);
        assert_eq!(g.view(0).playable, vec![0, 91]);
        assert!(!g.view(0).can_pass);
    }
    #[test]
    fn recycle_preserves_top_and_exhaustion_never_passes() {
        let mut g = setup(2);
        g.players[0].hand = vec![blue(10, 2)];
        g.stock.clear();
        g.discard = vec![blue(101, 6), red(100, 5)];
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.discard, vec![red(100, 5)]);
        assert_eq!(g.players[0].hand.len(), 2);
        assert_eq!(g.turn, 0);
        let before = serde_json::to_string(&g.view(0)).unwrap();
        assert!(g.act(0, Action::Draw).is_err());
        assert_eq!(serde_json::to_string(&g.view(0)).unwrap(), before);
    }
    #[test]
    fn private_view_and_disconnect_pause() {
        let mut g = setup(2);
        assert_eq!(g.view(0).hand, g.players[0].hand);
        let json = serde_json::to_string(&g.view(0)).unwrap();
        assert!(!json.contains("illegal"));
        assert!(!json.contains("stock"));
        g.set_connected(1, false);
        assert!(g.view(0).paused);
        assert!(g.act(0, Action::Draw).is_err());
        g.set_connected(1, true);
        play(&mut g, 0, 0);
    }
    #[test]
    fn deal_and_multiple_rounds_preserve_cards() {
        for n in 2..=10 {
            for seed in 0..50 {
                let mut g = Game::new(seed);
                for i in 0..n {
                    g.add_player(format!("{i}")).unwrap();
                }
                g.act(0, Action::Start).unwrap();
                let count = g.stock.len()
                    + g.discard.len()
                    + g.players.iter().map(|p| p.hand.len()).sum::<usize>();
                assert_eq!(count, 112);
                assert!(g.turn < n);
            }
        }
    }
    #[test]
    fn all_opening_action_cards_follow_table_rules() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..500 {
            let mut g = Game::new(seed);
            for name in ["A", "B", "C"] {
                g.add_player(name.into()).unwrap();
            }
            g.dealer = 0;
            g.begin_round();
            let face = g.discard[0].face;
            match face {
                Face::Wild => {
                    seen.insert("wild");
                    assert_eq!(g.phase, Phase::OpeningColor);
                    assert!(g
                        .act(0, Action::ChooseOpeningColor { color: Color::Blue })
                        .is_err());
                    g.act(1, Action::ChooseOpeningColor { color: Color::Blue })
                        .unwrap();
                    assert_eq!(g.turn, 1);
                    assert_eq!(g.active_color, Color::Blue);
                }
                Face::DrawTwo => {
                    seen.insert("draw2");
                    assert_eq!(g.players[1].hand.len(), 7);
                    assert_eq!(g.pending_two, 2);
                    assert_eq!(g.turn, 1);
                }
                Face::Reverse => {
                    seen.insert("reverse");
                    assert_eq!(g.turn, 0);
                    assert_eq!(g.direction, -1);
                }
                Face::Skip => {
                    seen.insert("skip");
                    assert_eq!(g.turn, 2);
                }
                Face::Number(_) => {
                    seen.insert("number");
                    assert_eq!(g.turn, 1);
                }
                _ => panic!("Draw Four and ? must not be opening cards"),
            }
        }
        assert_eq!(seen.len(), 5);
    }
    #[test]
    fn matching_number_does_not_make_four_illegal() {
        let mut g = setup(2);
        g.players[0].hand = vec![card(0, None, Face::WildFour), blue(1, 5)];
        play(&mut g, 0, 0);
        g.act(1, Action::ChallengeFour).unwrap();
        assert_eq!(g.players[1].hand.len(), 8);
        assert_eq!(g.players[0].hand.len(), 1);
    }
    #[test]
    fn repeated_single_draws_can_reach_a_wild_and_play_it() {
        let mut g = setup(2);
        g.players[0].hand = vec![blue(10, 2)];
        g.stock = vec![red(20, 7), card(21, None, Face::TargetTwo), blue(22, 6)];
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 2);
        assert!(g.view(0).playable.is_empty());
        g.act(0, Action::Draw).unwrap();
        assert_eq!(g.players[0].hand.len(), 3);
        assert_eq!(g.view(0).playable, vec![21]);
        play(&mut g, 0, 21);
        assert_eq!(g.turn, 1);
    }
    #[test]
    fn final_target_penalty_is_applied_before_scoring() {
        let mut g = setup(3);
        g.players[0].hand = vec![card(0, None, Face::TargetTwo)];
        play(&mut g, 0, 0);
        assert_eq!(g.players[1].hand.len(), 4);
        assert_eq!(
            g.phase,
            Phase::RoundOver {
                winner: 0,
                points: 26
            }
        );
    }
    #[test]
    fn scores_persist_between_rounds_and_reset_after_match() {
        let mut g = setup(2);
        g.players[0].score = 490;
        g.players[0].hand = vec![red(0, 2)];
        play(&mut g, 0, 0);
        assert_eq!(g.players[0].score, 494);
        assert!(g.act(1, Action::NextRound).is_err());
        g.act(0, Action::NextRound).unwrap();
        assert_eq!(g.players[0].score, 494);
        g.phase = Phase::MatchOver { winner: 0 };
        g.act(0, Action::NextRound).unwrap();
        assert_eq!(g.players[0].score, 0);
        assert_eq!(g.round, 1);
    }
    #[test]
    fn lobby_limits_and_host_authority() {
        let mut g = Game::new(0);
        assert!(g.add_player("\n".into()).is_err());
        assert!(g.add_player("A".repeat(21)).is_err());
        g.add_player("Host".into()).unwrap();
        assert!(g.act(0, Action::Start).is_err());
        for i in 1..10 {
            g.add_player(format!("P{i}")).unwrap();
        }
        assert!(g.add_player("Extra".into()).is_err());
        assert!(g.act(1, Action::Start).is_err());
        g.act(0, Action::Start).unwrap();
        assert!(g.add_player("Late joiner".into()).is_err());
        assert!(g.act(0, Action::Start).is_err());
    }
    #[test]
    fn simulated_matches_conserve_all_cards() {
        for seed in 0..30 {
            let mut g = Game::new(seed);
            for i in 0..4 {
                g.add_player(format!("{i}")).unwrap();
            }
            g.act(0, Action::Start).unwrap();
            for _ in 0..2000 {
                let p = g.turn;
                if g.phase == Phase::OpeningColor {
                    g.act(p, Action::ChooseOpeningColor { color: Color::Red })
                        .unwrap();
                } else if g.pending_four.is_some() {
                    g.act(p, Action::ChallengeFour).unwrap();
                } else if g.phase == Phase::Playing {
                    let v = g.view(p);
                    if let Some(id) = v.playable.first() {
                        let target = (p + 1) % 4;
                        g.act(
                            p,
                            Action::Play {
                                card: *id,
                                color: Some(Color::ALL[p]),
                                target: Some(target),
                                uno: true,
                            },
                        )
                        .unwrap();
                    } else {
                        g.act(p, Action::Draw).unwrap();
                    }
                } else {
                    break;
                }
                let all: Vec<_> = g
                    .stock
                    .iter()
                    .chain(&g.discard)
                    .chain(g.players.iter().flat_map(|p| &p.hand))
                    .map(|c| c.id)
                    .collect();
                let unique: std::collections::HashSet<_> = all.iter().collect();
                assert_eq!(all.len(), 112);
                assert_eq!(unique.len(), 112);
            }
            assert!(
                matches!(g.phase, Phase::RoundOver { .. } | Phase::MatchOver { .. }),
                "seed {seed} did not finish"
            );
        }
    }
}

//! Small bounded, newline-delimited JSON protocol over direct TCP.
//! TCP is not encrypted: use a trusted LAN or an encrypted mesh VPN.
use crate::game::{Action, Game, Phase, PlayerId, View};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const DEFAULT_PORT: u16 = 34197;
const PROTOCOL: u16 = 3;
const MAX_FRAME: usize = 32 * 1024;
const MAX_QUEUED: usize = 256 * 1024;
const TICK: Duration = Duration::from_millis(10);
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub struct Invite {
    pub address: String,
    pub token: String,
}
impl Invite {
    pub fn parse(input: &str) -> Result<Self, String> {
        let rest = input
            .trim()
            .strip_prefix("uno://")
            .ok_or("Invite must start with uno://")?;
        let (address, token) = rest
            .split_once('/')
            .ok_or("Invite must contain an address and room token.")?;
        if address.is_empty()
            || address.len() > 255
            || address
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '@' | '?' | '#' | '/'))
        {
            return Err("Invalid host address.".into());
        }
        let (host, port) = address
            .rsplit_once(':')
            .ok_or("Invite address needs a port.")?;
        let valid_host = if let Some(ip) = host.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
        {
            ip.parse::<std::net::Ipv6Addr>().is_ok()
        } else {
            !host.is_empty()
                && host
                    .strip_suffix('.')
                    .unwrap_or(host)
                    .split('.')
                    .all(|label| {
                        !label.is_empty()
                            && label.len() <= 63
                            && !label.starts_with('-')
                            && !label.ends_with('-')
                            && label
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    })
        };
        if !valid_host {
            return Err("Invalid host address. Use a hostname, IPv4, or bracketed IPv6.".into());
        }
        if !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
        {
            return Err("Invalid port.".into());
        }
        if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid room token.".into());
        }
        Ok(Self {
            address: address.into(),
            token: token.to_ascii_lowercase(),
        })
    }
    pub fn link(&self) -> String {
        format!("uno://{}/{}", self.address, self.token)
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum ClientMessage {
    Hello {
        version: u16,
        token: String,
        name: String,
        resume: Option<String>,
    },
    Action(Action),
    Ping,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome { player: PlayerId, resume: String },
    // Snapshots grow with public rule state; keep queued events small.
    State(Box<View>),
    Error(String),
    Pong,
}

fn secret() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Wire {
    stream: TcpStream,
    input: Vec<u8>,
    output: VecDeque<Vec<u8>>,
    written: usize,
    queued: usize,
    last_rx: Instant,
}
impl Wire {
    fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            input: Vec::new(),
            output: VecDeque::new(),
            written: 0,
            queued: 0,
            last_rx: Instant::now(),
        })
    }
    fn queue(&mut self, message: &impl Serialize) -> io::Result<()> {
        let mut data = serde_json::to_vec(message)?;
        data.push(b'\n');
        if data.len() > MAX_FRAME || self.queued + data.len() > MAX_QUEUED {
            return Err(io::Error::other("Slow peer or oversized message"));
        }
        self.queued += data.len();
        self.output.push_back(data);
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut budget = 64 * 1024;
        while let Some(data) = self.output.front() {
            if budget == 0 {
                break;
            }
            let end = data.len().min(self.written + budget);
            match self.stream.write(&data[self.written..end]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    self.written += n;
                    self.queued -= n;
                    budget -= n;
                    if self.written == data.len() {
                        self.output.pop_front();
                        self.written = 0;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    fn receive<T: serde::de::DeserializeOwned>(&mut self) -> io::Result<Vec<T>> {
        let mut buf = [0u8; 8192];
        // Bounded work per peer and tick, including malformed/unterminated messages.
        match self.stream.read(&mut buf) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                self.last_rx = Instant::now();
                self.input.extend_from_slice(&buf[..n]);
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
        let mut messages = Vec::new();
        while let Some(end) = self.input.iter().position(|b| *b == b'\n') {
            if end >= MAX_FRAME || messages.len() >= 32 {
                return Err(io::Error::other("Protocol flood"));
            }
            messages.push(serde_json::from_slice(&self.input[..end])?);
            self.input.drain(..=end);
        }
        if self.input.len() >= MAX_FRAME || self.last_rx.elapsed() > TIMEOUT {
            return Err(io::Error::other(
                "Peer timed out or sent an oversized frame",
            ));
        }
        Ok(messages)
    }
}
struct Peer {
    wire: Wire,
    player: Option<PlayerId>,
    dead: bool,
    closing: bool,
    joined: Instant,
    actions: u32,
    window: Instant,
}

pub struct ServerHandle {
    pub address: SocketAddr,
    pub token: String,
    pub owner_token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl ServerHandle {
    pub fn start(bind: &str) -> Result<Self, String> {
        let listener =
            TcpListener::bind(bind).map_err(|e| format!("Cannot host on {bind}: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let token = secret();
        let owner_token = secret();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let room = token.clone();
        let owner = owner_token.clone();
        let thread = thread::spawn(move || serve(listener, room, owner, thread_stop));
        Ok(Self {
            address,
            token,
            owner_token,
            stop,
            thread: Some(thread),
        })
    }
    pub fn owner_invite(&self) -> Invite {
        Invite {
            address: SocketAddr::new(
                if self.address.ip().is_unspecified() {
                    if self.address.is_ipv4() {
                        std::net::Ipv4Addr::LOCALHOST.into()
                    } else {
                        std::net::Ipv6Addr::LOCALHOST.into()
                    }
                } else {
                    self.address.ip()
                },
                self.address.port(),
            )
            .to_string(),
            token: self.owner_token.clone(),
        }
    }
    pub fn invite(&self, address: &str) -> Invite {
        Invite {
            address: address.into(),
            token: self.token.clone(),
        }
    }
}
impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Apply known socket failures before accepting any further game actions.
fn disconnect_dead(game: &mut Game, peers: &mut [Peer]) -> bool {
    let mut changed = false;
    for peer in peers.iter_mut().filter(|p| p.dead) {
        if let Some(player) = peer.player.take() {
            game.set_connected(player, false);
            changed = true;
        }
    }
    changed
}

fn serve(listener: TcpListener, token: String, owner: String, stop: Arc<AtomicBool>) {
    let mut game = Game::default();
    let mut peers: Vec<Peer> = Vec::new();
    let mut sessions: Vec<String> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        // Limit open sockets and accepts per tick to avoid unbounded resources.
        for _ in 0..8 {
            match listener.accept() {
                Ok((stream, _)) if peers.len() < 32 => {
                    if let Ok(wire) = Wire::new(stream) {
                        peers.push(Peer {
                            wire,
                            player: None,
                            dead: false,
                            closing: false,
                            joined: Instant::now(),
                            actions: 0,
                            window: Instant::now(),
                        });
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        let mut changed = disconnect_dead(&mut game, &mut peers);
        for i in 0..peers.len() {
            if peers[i].dead {
                continue;
            }
            if peers[i].closing {
                if peers[i].wire.flush().is_err() || peers[i].wire.queued == 0 {
                    peers[i].dead = true;
                }
                continue;
            }
            let messages = match peers[i].wire.receive::<ClientMessage>() {
                Ok(m) => m,
                Err(_) => {
                    peers[i].dead = true;
                    continue;
                }
            };
            if peers[i].player.is_none() && peers[i].joined.elapsed() > Duration::from_secs(5) {
                peers[i].dead = true;
                continue;
            }
            if peers[i].window.elapsed() >= Duration::from_secs(1) {
                peers[i].window = Instant::now();
                peers[i].actions = 0;
            }
            for message in messages {
                peers[i].actions += 1;
                if peers[i].actions > 30 {
                    peers[i].dead = true;
                    break;
                }
                let result: Result<(), String> = match message {
                    ClientMessage::Hello {
                        version,
                        token: supplied,
                        name,
                        resume,
                    } => {
                        if peers[i].player.is_some() {
                            Err("Already joined.".into())
                        } else if version != PROTOCOL {
                            Err("Game versions differ. Install the same release.".into())
                        } else if supplied != token && supplied != owner {
                            Err("Invalid invite token.".into())
                        } else {
                            let joined = if let Some(key) = resume {
                                sessions
                                    .iter()
                                    .position(|s| s == &key)
                                    .ok_or("Rejoin session not found.".into())
                            } else if supplied == owner && !game.players.is_empty() {
                                Err("Host already joined. Use the rejoin button in the original client.".into())
                            } else if supplied != owner && game.players.is_empty() {
                                Err("The host must join first. Try again in a moment.".into())
                            } else {
                                game.add_player(name).inspect(|_| sessions.push(secret()))
                            };
                            match joined {
                                Ok(player) => {
                                    // Transfer a resumed identity; old socket may still be timing out.
                                    for (j, peer) in peers.iter_mut().enumerate() {
                                        if j != i && peer.player == Some(player) {
                                            peer.player = None;
                                            peer.dead = true;
                                        }
                                    }
                                    peers[i].player = Some(player);
                                    if !game.players[player].connected {
                                        game.set_connected(player, true);
                                    }
                                    let welcome = ServerMessage::Welcome {
                                        player,
                                        resume: sessions[player].clone(),
                                    };
                                    if peers[i].wire.queue(&welcome).is_err() {
                                        peers[i].dead = true;
                                    }
                                    changed = true;
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                    }
                    ClientMessage::Action(action) => {
                        changed |= disconnect_dead(&mut game, &mut peers);
                        if let Some(player) = peers[i].player {
                            let result = game.act(player, action);
                            if result.is_ok() {
                                changed = true;
                            }
                            result
                        } else {
                            Err("Join the room first.".into())
                        }
                    }
                    ClientMessage::Ping => {
                        if peers[i].wire.queue(&ServerMessage::Pong).is_err() {
                            peers[i].dead = true;
                        }
                        Ok(())
                    }
                };
                if let Err(error) = result {
                    if peers[i].wire.queue(&ServerMessage::Error(error)).is_err() {
                        peers[i].dead = true;
                    }
                    if peers[i].player.is_none() {
                        peers[i].closing = true;
                        break;
                    }
                }
            }
        }
        // Mark disconnections before broadcasting. No seat/hand is silently discarded.
        changed |= disconnect_dead(&mut game, &mut peers);
        peers.retain(|p| !p.dead);
        for peer in &mut peers {
            if changed {
                if let Some(player) = peer.player {
                    if peer
                        .wire
                        .queue(&ServerMessage::State(Box::new(game.view(player))))
                        .is_err()
                    {
                        peer.dead = true;
                    }
                }
            }
            if peer.wire.flush().is_err() {
                peer.dead = true;
            }
        }
        thread::sleep(TICK);
    }
}

#[derive(Debug)]
pub enum ClientEvent {
    Message(ServerMessage),
    Disconnected(String),
}
pub struct Client {
    actions: mpsc::SyncSender<Action>,
    pub events: mpsc::Receiver<ClientEvent>,
    stop: Arc<AtomicBool>,
}
impl Client {
    pub fn connect(invite: Invite, name: String, resume: Option<String>) -> Self {
        let (actions, rx) = mpsc::sync_channel(32);
        let (tx, events) = mpsc::sync_channel(128);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        thread::spawn(move || {
            let result = client_loop(invite, name, resume, rx, &tx, thread_stop);
            let reason = result
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Disconnected.".into());
            // This terminal event must not be lost when snapshots filled the queue.
            // Dropping the client also drops the receiver, unblocking this send.
            let _ = tx.send(ClientEvent::Disconnected(reason));
        });
        Self {
            actions,
            events,
            stop,
        }
    }
    pub fn send(&self, action: Action) -> Result<(), String> {
        self.actions
            .try_send(action)
            .map_err(|_| "Connection busy or disconnected.".into())
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
/// JSON bounds alone do not make a snapshot safe for rendering/indexing.
fn validate_view(view: &View) -> io::Result<()> {
    let count = view.players.len();
    let cards = crate::game::deck();
    let valid = (1..=crate::game::MAX_PLAYERS).contains(&count)
        && (view.phase == Phase::Lobby || count >= 2)
        && view.you < count
        && view.host < count
        && view.turn < count
        && matches!(view.direction, -1 | 1)
        && view.deck_count <= cards.len()
        && view.players.iter().all(|p| {
            p.cards <= cards.len()
                && (1..=20).contains(&p.name.chars().count())
                && !p.name.chars().any(char::is_control)
        })
        && view.hand.len() == view.players[view.you].cards
        && view.players.iter().map(|p| p.cards).sum::<usize>()
            + view.deck_count
            + usize::from(view.top.is_some())
            <= cards.len()
        && view
            .hand
            .iter()
            .chain(view.top.iter())
            .all(|c| cards.get(c.id as usize) == Some(c))
        && view
            .hand
            .iter()
            .enumerate()
            .all(|(i, c)| !view.hand[..i].contains(c) && view.top != Some(*c))
        && view.playable.len() <= view.hand.len()
        && view
            .playable
            .iter()
            .all(|id| view.hand.iter().any(|c| c.id == *id))
        && view.vulnerable.is_none_or(|p| p < count)
        && view.declared_uno.is_none_or(|p| p < count)
        && view
            .pending_four
            .as_ref()
            .is_none_or(|p| p.actor < count && p.victim < count)
        && match view.phase {
            Phase::RoundOver { winner, .. } | Phase::MatchOver { winner } => winner < count,
            _ => true,
        }
        && view.log.len() <= 10
        && view.log.iter().all(|entry| entry.len() <= 1024);
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid game snapshot from host",
        ))
    }
}

fn client_loop(
    invite: Invite,
    name: String,
    resume: Option<String>,
    actions: mpsc::Receiver<Action>,
    events: &mpsc::SyncSender<ClientEvent>,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    let addresses: Vec<_> = invite.address.to_socket_addrs()?.take(8).collect();
    let mut stream = None;
    let mut last_error = io::Error::other("No address found for host");
    for address in addresses {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match TcpStream::connect_timeout(&address, Duration::from_secs(3)) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last_error = e,
        }
    }
    let mut wire = Wire::new(stream.ok_or(last_error)?)?;
    wire.queue(&ClientMessage::Hello {
        version: PROTOCOL,
        token: invite.token,
        name,
        resume,
    })?;
    let mut ping = Instant::now();
    let mut last_server = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        for action in actions.try_iter().take(16) {
            wire.queue(&ClientMessage::Action(action))?;
        }
        if ping.elapsed() > Duration::from_secs(2) {
            wire.queue(&ClientMessage::Ping)?;
            ping = Instant::now();
        }
        wire.flush()?;
        for message in wire.receive::<ServerMessage>()? {
            if let ServerMessage::State(view) = &message {
                validate_view(view)?;
            }
            last_server = Instant::now();
            if !matches!(message, ServerMessage::Pong) {
                events
                    .try_send(ClientEvent::Message(message))
                    .map_err(|_| io::Error::other("Client event queue is full"))?;
            }
        }
        if last_server.elapsed() > TIMEOUT {
            return Err(io::Error::other("Host stopped responding"));
        }
        thread::sleep(TICK);
    }
}

pub fn local_address(port: u16) -> String {
    // UDP connect selects an interface without transmitting a packet.
    let ip = UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.0.2.1:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into());
    format!("{ip}:{port}")
}

pub fn phase_name(phase: &Phase) -> &'static str {
    match phase {
        Phase::Lobby => "Lobby",
        Phase::OpeningColor => "Choose opening color",
        Phase::Playing => "In play",
        Phase::RoundOver { .. } => "Round complete",
        Phase::MatchOver { .. } => "Match complete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait(client: &Client, pred: impl Fn(&ServerMessage) -> bool) -> ServerMessage {
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            match client.events.recv_timeout(Duration::from_millis(100)) {
                Ok(ClientEvent::Message(m)) if pred(&m) => return m,
                Ok(ClientEvent::Disconnected(e)) => panic!("Disconnected: {e}"),
                _ => {}
            }
        }
        panic!("Timed out waiting for server");
    }
    #[test]
    fn invites_validate_and_roundtrip() {
        let invite = Invite {
            address: "[::1]:34197".into(),
            token: "0123456789abcdef0123456789abcdef".into(),
        };
        assert_eq!(Invite::parse(&invite.link()).unwrap().link(), invite.link());
        for bad in [
            "http://example.com",
            "uno://localhost/a",
            "uno://localhost:0/0123456789abcdef0123456789abcdef",
            "uno://localhost:12/not-a-token",
            "uno://:34197/0123456789abcdef0123456789abcdef",
            "uno://::1:34197/0123456789abcdef0123456789abcdef",
            "uno://[not-ipv6]:34197/0123456789abcdef0123456789abcdef",
            "uno://local\0host:34197/0123456789abcdef0123456789abcdef",
            "uno://host..local:34197/0123456789abcdef0123456789abcdef",
            "uno://host..:34197/0123456789abcdef0123456789abcdef",
            "uno://localhost:+12/0123456789abcdef0123456789abcdef",
        ] {
            assert!(Invite::parse(bad).is_err());
        }
    }
    fn wire_pair() -> (Wire, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let remote = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        (Wire::new(stream).unwrap(), remote)
    }

    fn receive<T: serde::de::DeserializeOwned>(wire: &mut Wire) -> io::Result<Vec<T>> {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            let messages = wire.receive()?;
            if !messages.is_empty() {
                return Ok(messages);
            }
            assert!(Instant::now() < until, "Timed out waiting for frame");
            thread::sleep(TICK);
        }
    }

    #[test]
    fn fragmented_frames_and_exact_size_limit() {
        let (mut wire, mut remote) = wire_pair();
        remote.write_all(b"\"Pi").unwrap();
        assert!(wire.receive::<ClientMessage>().unwrap().is_empty());
        remote.write_all(b"ng\"\n").unwrap();
        assert!(matches!(
            receive::<ClientMessage>(&mut wire).unwrap()[0],
            ClientMessage::Ping
        ));

        let value = "x".repeat(MAX_FRAME - 3); // Quotes + newline are part of the limit.
        wire.queue(&value).unwrap();
        wire.flush().unwrap();
        let mut remote = Wire::new(remote).unwrap();
        assert_eq!(receive::<String>(&mut remote).unwrap(), vec![value]);
        assert!(wire.queue(&"x".repeat(MAX_FRAME - 2)).is_err());
        // A raw peer cannot bypass the same boundary on receive.
        let mut oversized = serde_json::to_vec(&"x".repeat(MAX_FRAME - 2)).unwrap();
        oversized.push(b'\n');
        wire.stream.write_all(&oversized).unwrap();
        assert!(receive::<String>(&mut remote).is_err());
    }

    #[test]
    fn malformed_snapshots_are_rejected_before_reaching_the_ui() {
        let mut game = Game::new(23);
        game.add_player("Host".into()).unwrap();
        assert!(validate_view(&game.view(0)).is_ok());
        for n in 1..10 {
            game.add_player(format!("P{n}")).unwrap();
        }
        game.act(0, Action::Start).unwrap();
        let valid = game.view(0);
        assert!(validate_view(&valid).is_ok());
        let mutations: &[fn(&mut View)] = &[
            |v| v.players.clear(),
            |v| v.you = usize::MAX,
            |v| v.turn = v.players.len(),
            |v| v.host = usize::MAX,
            |v| v.players[0].cards = usize::MAX,
            |v| v.deck_count = usize::MAX,
            |v| v.hand[0].id = u16::MAX,
            |v| v.hand[1] = v.hand[0],
            |v| v.top = Some(v.hand[0]),
            |v| v.phase = Phase::MatchOver { winner: usize::MAX },
            |v| v.vulnerable = Some(usize::MAX),
            |v| v.declared_uno = Some(usize::MAX),
            |v| {
                v.pending_four = Some(crate::game::FourView {
                    actor: 0,
                    victim: usize::MAX,
                })
            },
            |v| v.log.push("x".repeat(1025)),
        ];
        for mutate in mutations {
            let mut invalid = valid.clone();
            mutate(&mut invalid);
            assert!(validate_view(&invalid).is_err());
        }
    }

    #[test]
    fn known_socket_failures_pause_before_further_actions() {
        let mut game = Game::new(1);
        game.add_player("Host".into()).unwrap();
        game.add_player("Guest".into()).unwrap();
        let (wire, _remote) = wire_pair();
        let mut peers = vec![Peer {
            wire,
            player: Some(1),
            dead: true,
            closing: false,
            joined: Instant::now(),
            actions: 0,
            window: Instant::now(),
        }];
        assert!(disconnect_dead(&mut game, &mut peers));
        assert!(game.view(0).paused);
        assert!(game.act(0, Action::Start).is_err());
        assert!(!disconnect_dead(&mut game, &mut peers));
    }

    #[test]
    fn terminal_disconnect_is_delivered_after_a_full_event_queue() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = Client::connect(
            Invite {
                address: listener.local_addr().unwrap().to_string(),
                token: "0".repeat(32),
            },
            "Guest".into(),
            None,
        );
        let fake_host = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut wire = Wire::new(stream).unwrap();
            receive::<ClientMessage>(&mut wire).unwrap();
            // Wait for the worker to close its socket after overflowing events.
            let until = Instant::now() + Duration::from_secs(6);
            while Instant::now() < until {
                if wire.queue(&ServerMessage::Error("test".into())).is_err()
                    || wire.flush().is_err()
                    || wire.receive::<ClientMessage>().is_err()
                {
                    return;
                }
                thread::sleep(TICK);
            }
            panic!("Worker did not disconnect from a full event queue");
        });
        fake_host.join().unwrap();
        for _ in 0..128 {
            assert!(matches!(
                client.events.recv_timeout(Duration::from_secs(2)).unwrap(),
                ClientEvent::Message(ServerMessage::Error(_))
            ));
        }
        assert!(
            matches!(client.events.recv_timeout(Duration::from_secs(2)).unwrap(),
            ClientEvent::Disconnected(reason) if reason.contains("queue is full"))
        );
    }

    #[test]
    fn resume_replaces_a_live_socket_without_pausing_or_losing_a_hand() {
        let server = ServerHandle::start("127.0.0.1:0").unwrap();
        let host = Client::connect(server.owner_invite(), "Host".into(), None);
        wait(&host, |m| matches!(m, ServerMessage::Welcome { .. }));
        let invite = server.invite(&server.address.to_string());
        let old = Client::connect(invite.clone(), "Guest".into(), None);
        let key = match wait(&old, |m| matches!(m, ServerMessage::Welcome { .. })) {
            ServerMessage::Welcome { resume, .. } => resume,
            _ => unreachable!(),
        };
        wait(
            &host,
            |m| matches!(m, ServerMessage::State(v) if v.players.len() == 2),
        );
        host.send(Action::Start).unwrap();
        let before = match wait(
            &old,
            |m| matches!(m, ServerMessage::State(v) if v.phase != Phase::Lobby),
        ) {
            ServerMessage::State(v) => v,
            _ => unreachable!(),
        };
        let resumed = Client::connect(invite, "Ignored name".into(), Some(key));
        let after = match wait(&resumed, |m| matches!(m, ServerMessage::State(_))) {
            ServerMessage::State(v) => v,
            _ => unreachable!(),
        };
        assert!(!after.paused);
        assert_eq!(after.you, before.you);
        assert_eq!(after.hand, before.hand);
        assert_eq!(after.players[after.you].name, "Guest");
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            if matches!(
                old.events.recv_timeout(TICK),
                Ok(ClientEvent::Disconnected(_))
            ) {
                break;
            }
            assert!(Instant::now() < until, "Old socket was not replaced");
        }
        // Dropping the replaced socket must not mark the resumed identity offline.
        drop(old);
        host.send(Action::Start).unwrap();
        wait(
            &host,
            |m| matches!(m, ServerMessage::Error(e) if !e.contains("disconnected")),
        );
    }

    #[test]
    fn real_tcp_lobby_start_private_hands_and_resume() {
        let server = ServerHandle::start("127.0.0.1:0").unwrap();
        let host = Client::connect(server.owner_invite(), "Host".into(), None);
        wait(&host, |m| matches!(m, ServerMessage::Welcome { .. }));
        let invite = server.invite(&server.address.to_string());
        let guest = Client::connect(invite.clone(), "Guest".into(), None);
        let resume = match wait(&guest, |m| matches!(m, ServerMessage::Welcome { .. })) {
            ServerMessage::Welcome { resume, .. } => resume,
            _ => unreachable!(),
        };
        wait(
            &host,
            |m| matches!(m, ServerMessage::State(v) if v.players.len() == 2),
        );
        guest.send(Action::Start).unwrap();
        wait(&guest, |m| matches!(m, ServerMessage::Error(_)));
        host.send(Action::Start).unwrap();
        let hv = match wait(
            &host,
            |m| matches!(m, ServerMessage::State(v) if v.phase != Phase::Lobby),
        ) {
            ServerMessage::State(v) => v,
            _ => unreachable!(),
        };
        let gv = match wait(
            &guest,
            |m| matches!(m, ServerMessage::State(v) if v.phase != Phase::Lobby),
        ) {
            ServerMessage::State(v) => v,
            _ => unreachable!(),
        };
        assert_eq!(gv.you, 1);
        assert!(!hv.hand.iter().any(|c| gv.hand.contains(c)));
        drop(guest);
        wait(&host, |m| matches!(m, ServerMessage::State(v) if v.paused));
        let rejoined = Client::connect(invite, "Guest".into(), Some(resume));
        let resumed = match wait(
            &rejoined,
            |m| matches!(m, ServerMessage::State(v) if !v.paused),
        ) {
            ServerMessage::State(v) => v,
            _ => unreachable!(),
        };
        assert_eq!(gv.hand, resumed.hand);
        wait(&host, |m| matches!(m, ServerMessage::State(v) if !v.paused));
    }
    #[test]
    fn oversized_peer_does_not_stop_the_host() {
        let server = ServerHandle::start("127.0.0.1:0").unwrap();
        let host = Client::connect(server.owner_invite(), "Host".into(), None);
        wait(&host, |m| matches!(m, ServerMessage::Welcome { .. }));
        let mut attacker = TcpStream::connect(server.address).unwrap();
        attacker
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let _ = attacker.write_all(&vec![b'x'; MAX_FRAME + 8192]);
        let guest = Client::connect(
            server.invite(&server.address.to_string()),
            "Guest".into(),
            None,
        );
        wait(&guest, |m| matches!(m, ServerMessage::Welcome { .. }));
        wait(
            &host,
            |m| matches!(m, ServerMessage::State(v) if v.players.len() == 2),
        );
        host.send(Action::Start).unwrap();
        wait(
            &host,
            |m| matches!(m, ServerMessage::State(v) if v.phase != Phase::Lobby),
        );
    }
    #[test]
    fn whole_round_over_real_tcp() {
        let server = ServerHandle::start("127.0.0.1:0").unwrap();
        let host = Client::connect(server.owner_invite(), "Host".into(), None);
        wait(&host, |m| matches!(m, ServerMessage::Welcome { .. }));
        let guest = Client::connect(
            server.invite(&server.address.to_string()),
            "Guest".into(),
            None,
        );
        wait(&guest, |m| matches!(m, ServerMessage::Welcome { .. }));
        wait(
            &host,
            |m| matches!(m, ServerMessage::State(v) if v.players.len() == 2),
        );
        let clients = [host, guest];
        clients[0].send(Action::Start).unwrap();
        let mut finished = false;
        for _ in 0..2000 {
            let views: Vec<_> = clients
                .iter()
                .map(|c| {
                    match wait(
                        c,
                        |m| matches!(m, ServerMessage::State(v) if v.phase != Phase::Lobby),
                    ) {
                        ServerMessage::State(v) => v,
                        _ => unreachable!(),
                    }
                })
                .collect();
            let p = views[0].turn;
            let v = &views[p];
            assert!(!views[0].hand.iter().any(|c| views[1].hand.contains(c)));
            if matches!(v.phase, Phase::RoundOver { .. } | Phase::MatchOver { .. }) {
                finished = true;
                break;
            }
            let action = if v.phase == Phase::OpeningColor {
                Action::ChooseOpeningColor {
                    color: crate::game::Color::Blue,
                }
            } else if v.pending_four.is_some() {
                Action::AcceptFour
            } else if let Some(card) = v.playable.first() {
                Action::Play {
                    card: *card,
                    color: Some(crate::game::Color::Blue),
                    target: Some(1 - p),
                    uno: true,
                }
            } else {
                Action::Draw
            };
            clients[p].send(action).unwrap();
            // Respect the production rate limit even on an all-local test.
            thread::sleep(Duration::from_millis(40));
        }
        assert!(finished, "Round did not finish");
    }
    #[test]
    fn wrong_invite_rejected() {
        let server = ServerHandle::start("127.0.0.1:0").unwrap();
        let bad = Client::connect(
            Invite {
                address: server.address.to_string(),
                token: "0".repeat(32),
            },
            "Nope".into(),
            None,
        );
        wait(
            &bad,
            |m| matches!(m, ServerMessage::Error(e) if e.contains("Invalid invite")),
        );
    }
}

mod card_motion;
mod clipboard;
mod self_play;
mod table_view;
mod ui;
use macroquad::prelude::*;
use ui::{ACCENT, ERROR, GOLD, INK, LINE, MUTED, PANEL, TEXT};
use uno::{
    game::{Action, Card, Color as Suit, Face, Phase, View},
    net::{local_address, Client, ClientEvent, Invite, ServerHandle, ServerMessage, DEFAULT_PORT},
};

fn config() -> Conf {
    Conf {
        window_title: "UNO - a little friendly competition".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    }
}

#[derive(Default)]
struct Choice {
    card: Option<Card>,
    color: Option<Suit>,
    target: Option<usize>,
}
#[derive(Clone, Copy, PartialEq)]
enum HomeTab {
    Host,
    Join,
    SelfPlay,
}

struct CardDrag {
    card: Card,
    home: table_view::CardPose,
    pose: table_view::CardPose,
    offset: Vec3,
}
struct CardReturn {
    drag: CardDrag,
    start: f64,
    owner: usize,
    round: u32,
}
impl CardReturn {
    // A penalty changes the hand immediately; only the visual return is delayed.
    fn refresh(&mut self, view: &View) -> bool {
        if view.you != self.owner || view.round != self.round || view.phase != Phase::Playing {
            return false;
        }
        let Some(index) = view.hand.iter().position(|c| *c == self.drag.card) else {
            return false;
        };
        self.drag.home = table_view::hand_card(
            view.you,
            view.you,
            view.players.len(),
            view.hand.len(),
            index,
        );
        true
    }
}

struct App {
    art: ui::Art,
    name: String,
    invite_text: String,
    address: String,
    port: String,
    focus: ui::InputFocus,
    home_tab: HomeTab,
    local_players: usize,
    self_play: Option<self_play::SelfPlay>,
    server: Option<ServerHandle>,
    client: Option<Client>,
    invite: Option<Invite>,
    resume: Option<String>,
    view: Option<View>,
    disconnected: bool,
    connecting: bool,
    error: String,
    notice: String,
    notice_until: f64,
    choice: Option<Choice>,
    uno: bool,
    rules: bool,
    table_talk: bool,
    leaving: bool,
    exit_requested: bool,
    camera: table_view::TableCamera,
    focused_seat: usize,
    camera_dragging: bool,
    card_drag: Option<CardDrag>,
    card_return: Option<CardReturn>,
    last_mouse: Vec2,
    motion: card_motion::CardMotion,
    pending_drag: Option<CardDrag>,
    camera_reveal: f32,
    player_menu: bool,
    show_roster: bool,
    fullscreen: bool,
}
impl App {
    fn new() -> Self {
        let mut args = std::env::args().skip(1);
        let mut invite_text = String::new();
        let mut name = "Player".to_string();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--join" => invite_text = args.next().unwrap_or_default(),
                "--name" => name = args.next().unwrap_or_else(|| "Player".into()),
                _ if arg.starts_with("uno://") => invite_text = arg,
                _ => {}
            }
        }
        Self {
            art: ui::Art::new(),
            home_tab: if invite_text.is_empty() {
                HomeTab::Host
            } else {
                HomeTab::Join
            },
            local_players: 2,
            self_play: None,
            name,
            invite_text,
            address: local_address(DEFAULT_PORT),
            port: DEFAULT_PORT.to_string(),
            focus: ui::InputFocus::default(),
            server: None,
            client: None,
            invite: None,
            resume: None,
            view: None,
            disconnected: false,
            connecting: false,
            error: String::new(),
            notice: String::new(),
            notice_until: 0.,
            choice: None,
            uno: false,
            rules: false,
            table_talk: false,
            leaving: false,
            exit_requested: false,
            camera: table_view::TableCamera::default(),
            focused_seat: 0,
            camera_dragging: false,
            card_drag: None,
            card_return: None,
            last_mouse: Vec2::ZERO,
            motion: card_motion::CardMotion::default(),
            pending_drag: None,
            camera_reveal: 0.,
            player_menu: false,
            show_roster: false,
            fullscreen: false,
        }
    }
    fn notify(&mut self, value: &str) {
        self.notice = value.into();
        self.notice_until = get_time() + 3.;
    }
    fn send(&mut self, action: Action) {
        if self.disconnected || self.connecting {
            return;
        }
        if let Some(local) = &mut self.self_play {
            if let Err(error) = local.act(action) {
                self.error = error;
                self.return_pending();
            }
        } else if let Some(client) = &self.client {
            if let Err(error) = client.send(action) {
                self.error = error;
                self.return_pending();
            }
        }
    }
    fn connect(&mut self, invite: Invite) {
        self.client = Some(Client::connect(
            invite.clone(),
            self.name.trim().into(),
            self.resume.clone(),
        ));
        self.invite = Some(invite);
        self.disconnected = false;
        self.connecting = true;
        self.error.clear();
    }
    fn host(&mut self) {
        if self.name.trim().is_empty() {
            self.error = "Choose a player name.".into();
            return;
        }
        let Some(port) = self.port.parse::<u16>().ok().filter(|p| *p > 0) else {
            self.error = "Choose a TCP port from 1 to 65535.".into();
            return;
        };
        // Validate advertised address before starting a room.
        if Invite::parse(&format!("uno://{}/{}", self.address, "0".repeat(32))).is_err() {
            self.error =
                "Invite address should be HOST:PORT, for example 192.168.1.20:34197.".into();
            return;
        }
        match ServerHandle::start(&format!("0.0.0.0:{port}")) {
            Ok(server) => {
                let invite = server.owner_invite();
                self.server = Some(server);
                self.connect(invite);
            }
            Err(error) => self.error = error,
        }
    }
    fn start_self_play(&mut self) {
        match self_play::SelfPlay::new(self.name.trim(), self.local_players) {
            Ok(local) => {
                self.leave();
                self.self_play = Some(local);
                self.focus = ui::InputFocus::default();
            }
            Err(error) => self.error = error,
        }
    }
    fn active(&self) -> bool {
        self.client.is_some() || self.self_play.is_some()
    }
    fn leave(&mut self) {
        self.self_play = None;
        self.client = None;
        self.server = None;
        self.view = None;
        self.invite = None;
        self.resume = None;
        self.disconnected = false;
        self.connecting = false;
        self.choice = None;
        self.leaving = false;
        self.rules = false;
        self.table_talk = false;
        self.error.clear();
        self.uno = false;
        self.camera = table_view::TableCamera::default();
        self.focused_seat = 0;
        self.camera_dragging = false;
        self.card_drag = None;
        self.card_return = None;
        self.motion = card_motion::CardMotion::default();
        self.pending_drag = None;
        self.camera_reveal = 0.;
        self.player_menu = false;
        self.notice.clear();
    }
    fn apply_view(&mut self, view: View) {
        let mut cancel_pending = view.paused;
        if let Some(old) = &self.view {
            self.motion.observe(
                old,
                &view,
                self.pending_drag.as_ref().map(|d| d.pose),
                get_time(),
            );
            if view.turn_id != old.turn_id || view.you != old.you || view.round != old.round {
                self.uno = false;
            }
            if self
                .pending_drag
                .as_ref()
                .is_some_and(|d| !view.hand.contains(&d.card))
                || view.you != old.you
                || view.round != old.round
            {
                self.pending_drag = None;
            }
            if view.turn != old.turn
                || view.hand != old.hand
                || view.you != old.you
                || view.phase != old.phase
                || view.round != old.round
            {
                self.choice = None;
                self.card_drag = None;
                cancel_pending = true;
            }
            if view.you != old.you || view.round != old.round {
                self.focused_seat = 0;
                self.camera_dragging = false;
                self.camera = table_view::TableCamera::default();
            }
        }
        if self.card_return.as_mut().is_some_and(|r| !r.refresh(&view)) {
            self.card_return = None;
        }
        if view.paused {
            self.choice = None;
            self.card_drag = None;
        }
        self.view = Some(view);
        if cancel_pending {
            self.return_pending();
        }
        self.connecting = false;
        self.error.clear();
    }
    fn poll(&mut self) {
        if let Some(view) = self.self_play.as_mut().and_then(|local| local.take_view()) {
            self.apply_view(view);
        }
        let events: Vec<_> = self
            .client
            .as_ref()
            .map(|c| c.events.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                ClientEvent::Message(ServerMessage::Welcome { resume, .. }) => {
                    self.resume = Some(resume)
                }
                ClientEvent::Message(ServerMessage::State(view)) => {
                    self.apply_view(*view);
                }
                ClientEvent::Message(ServerMessage::Error(error)) => {
                    self.error = error;
                    self.return_pending();
                }
                ClientEvent::Disconnected(reason) => {
                    self.disconnected = true;
                    self.connecting = false;
                    self.choice = None;
                    self.card_drag = None;
                    self.card_return = None;
                    self.return_pending();
                    if self.error.is_empty() {
                        self.error = format!("Connection lost: {reason}");
                    }
                }
                _ => {}
            }
        }
    }
    fn header(&mut self, active: bool) {
        ui::text("UNO", 28., 53., 36., TEXT);
        ui::text("TABLE", 111., 52., 24., ACCENT);
        ui::text("LOCAL HOST. REAL FRIENDS.", 223., 49., 13., MUTED);
        let enabled = !self.rules && !self.table_talk && !self.leaving && self.choice.is_none();
        if active {
            if let Some(local) = &mut self.self_play {
                ui::text("SELF-PLAY", 553., 49., 14., GOLD);
                if ui::button("Next hand", Rect::new(692., 23., 154., 38.), enabled, false) {
                    local.next_hand();
                }
                if ui::button("Turn hand", Rect::new(858., 23., 134., 38.), enabled, false) {
                    local.follow_turn();
                }
            }
            if ui::button(
                "Leave table",
                Rect::new(1110., 23., 150., 38.),
                enabled,
                false,
            ) {
                self.leaving = true;
            }
            if ui::button("Rules", Rect::new(1004., 23., 94., 38.), enabled, false) {
                self.rules = true;
            }
            if self.server.is_some()
                && ui::button(
                    "Copy invite",
                    Rect::new(858., 23., 134., 38.),
                    enabled,
                    false,
                )
            {
                self.copy_invite();
            }
        } else if ui::button(
            "How to play",
            Rect::new(1110., 23., 150., 38.),
            enabled,
            false,
        ) {
            self.rules = true;
        }
        draw_line(20., 76., 1260., 76., 1., LINE);
    }
    fn copy_invite(&mut self) {
        if let Some(server) = &self.server {
            match clipboard::set(&server.invite(&self.address).link()) {
                Ok(()) => self.notify("Invite copied. Send it privately to your friends."),
                Err(error) => self.error = error,
            }
        }
    }
    fn home(&mut self) {
        self.art.scene(
            None,
            Rect::new(520., 120., 740., 510.),
            table_view::TablePose {
                orbit: (get_time() * 0.06) as f32,
                elevation: 65.,
                distance: 23.,
                ..Default::default()
            },
            None,
            None,
            None,
        );
        self.header(false);
        ui::text("Your table.", 60., 164., 54., TEXT);
        ui::text("Your house rules.", 60., 223., 48., ACCENT);
        ui::text(
            "2-10 players / 3D / no public game server",
            63.,
            264.,
            18.,
            MUTED,
        );
        let enabled = !self.rules;
        for (i, (label, tab)) in [
            ("Host a table", HomeTab::Host),
            ("Join a friend", HomeTab::Join),
            ("Self-play", HomeTab::SelfPlay),
        ]
        .into_iter()
        .enumerate()
        {
            if ui::button(
                label,
                Rect::new(60. + i as f32 * 148., 302., 136., 43.),
                enabled,
                self.home_tab == tab,
            ) {
                self.home_tab = tab;
                self.focus = ui::InputFocus::default();
                self.error.clear();
            }
        }
        if enabled {
            ui::input(
                "YOUR NAME",
                Rect::new(60., 385., 432., 44.),
                &mut self.name,
                ui::Field::Name,
                &mut self.focus,
                20,
            )
            .unwrap_or_else(|error| self.error = error);
            if self.home_tab == HomeTab::Host {
                ui::input(
                    "INVITE ADDRESS (LAN / VPN / PUBLIC HOST:PORT)",
                    Rect::new(60., 468., 432., 44.),
                    &mut self.address,
                    ui::Field::Address,
                    &mut self.focus,
                    255,
                )
                .unwrap_or_else(|error| self.error = error);
                ui::input(
                    "LISTEN PORT",
                    Rect::new(60., 553., 142., 44.),
                    &mut self.port,
                    ui::Field::Port,
                    &mut self.focus,
                    5,
                )
                .unwrap_or_else(|error| self.error = error);
                if ui::button(
                    "Use LAN address",
                    Rect::new(215., 553., 277., 44.),
                    true,
                    false,
                ) {
                    self.address = local_address(self.port.parse().unwrap_or(DEFAULT_PORT));
                    self.notify(
                        "LAN address selected. For VPN play, replace it with your VPN IP:port.",
                    );
                }
                if ui::button("Open table  >", Rect::new(60., 623., 432., 50.), true, true) {
                    self.focus = ui::InputFocus::default();
                    self.host();
                }
            } else if self.home_tab == HomeTab::SelfPlay {
                ui::text("SEATS / YOU CONTROL EVERY PLAYER", 60., 458., 15., MUTED);
                if ui::button(
                    "-",
                    Rect::new(60., 468., 54., 44.),
                    self.local_players > 2,
                    false,
                ) {
                    self.local_players -= 1;
                }
                ui::center(
                    &self.local_players.to_string(),
                    Rect::new(126., 468., 74., 44.),
                    26.,
                    TEXT,
                );
                if ui::button(
                    "+",
                    Rect::new(212., 468., 54., 44.),
                    self.local_players < uno::game::MAX_PLAYERS,
                    false,
                ) {
                    self.local_players += 1;
                }
                ui::wrap("Hands follow the turn automatically. Use Next hand to inspect another seat, or Turn hand to return. No bots, network or second window.", 60., 549., 432., 18., MUTED);
                if ui::button(
                    "Start self-play  >",
                    Rect::new(60., 623., 432., 50.),
                    true,
                    true,
                ) {
                    self.start_self_play();
                }
            } else {
                ui::input(
                    "INVITE LINK",
                    Rect::new(60., 468., 432., 44.),
                    &mut self.invite_text,
                    ui::Field::Invite,
                    &mut self.focus,
                    512,
                )
                .unwrap_or_else(|error| self.error = error);
                if ui::button("Paste invite", Rect::new(60., 531., 160., 39.), true, false) {
                    match clipboard::get() {
                        Ok(value) => {
                            self.invite_text = clipboard::field_text(&value, 512);
                            self.focus = ui::InputFocus::default();
                        }
                        Err(error) => self.error = error,
                    }
                }
                if ui::button(
                    "Take a seat  >",
                    Rect::new(60., 623., 432., 50.),
                    true,
                    true,
                ) {
                    self.focus = ui::InputFocus::default();
                    match Invite::parse(&self.invite_text) {
                        Ok(invite) => self.connect(invite),
                        Err(error) => self.error = error,
                    }
                }
            }
        }
        ui::panel(Rect::new(612., 596., 590., 122.), PANEL);
        ui::text("A CLASSIC, WITH TWO TWISTS", 636., 626., 15., GOLD);
        ui::text(
            "No match? Keep drawing until you find one.",
            636.,
            658.,
            20.,
            TEXT,
        );
        ui::text("Play ? and choose who picks up two.", 636., 691., 20., TEXT);
        ui::wrap(
            if self.home_tab == HomeTab::SelfPlay {
                "Offline testing with the same rules as multiplayer. Your name is used for seat 1; the other seats are Player 2, Player 3, etc."
            } else {
                "Direct TCP on a trusted LAN or private VPN. Internet hosting requires a reachable IP and port forwarding. An invite is not a NAT bypass."
            },
            60.,
            712.,
            450.,
            15.,
            MUTED,
        );
        ui::text(
            "F11  fullscreen     |     No account. No assets to download.",
            660.,
            765.,
            14.,
            MUTED,
        );
    }
    fn connecting(&mut self) {
        self.header(true);
        self.art.scene(
            None,
            Rect::new(100., 110., 1080., 590.),
            table_view::TablePose {
                orbit: (get_time() * 0.1) as f32,
                elevation: 65.,
                distance: 23.,
                ..Default::default()
            },
            None,
            None,
            None,
        );
        ui::panel(Rect::new(380., 281., 520., 210.), PANEL);
        ui::center(
            if self.disconnected {
                "Could not reach the table"
            } else {
                "Connecting to your table..."
            },
            Rect::new(395., 299., 490., 54.),
            28.,
            TEXT,
        );
        ui::wrap(
            if self.disconnected {
                "Check the address, firewall and VPN. The host must keep the game open."
            } else {
                "Opening a direct connection. Your hand will be sent privately by the host."
            },
            413.,
            382.,
            450.,
            18.,
            MUTED,
        );
        if self.disconnected
            && ui::button(
                "Try again",
                Rect::new(510., 435., 260., 40.),
                !self.leaving && !self.rules,
                true,
            )
        {
            if let Some(invite) = self.invite.clone() {
                self.connect(invite);
            }
        }
    }
    fn lobby(&mut self, view: &View) {
        self.art.scene(
            None,
            Rect::new(475., 160., 790., 485.),
            table_view::TablePose {
                orbit: (get_time() * 0.045) as f32,
                elevation: 65.,
                distance: 23.,
                ..Default::default()
            },
            None,
            None,
            None,
        );
        self.header(true);
        ui::text("Make yourself at home.", 44., 135., 34., TEXT);
        ui::text(
            "The host starts when everyone has a seat.",
            46.,
            170.,
            18.,
            MUTED,
        );
        ui::panel(Rect::new(40., 201., 466., 497.), PANEL);
        ui::text(
            &format!("AT THE TABLE  /  {} OF 10", view.players.len()),
            62.,
            233.,
            16.,
            GOLD,
        );
        for (id, p) in view.players.iter().enumerate() {
            let y = 254. + id as f32 * 35.;
            draw_circle(71., y + 10., 4., if p.connected { ACCENT } else { ERROR });
            ui::text(&format!("{:02}", id + 1), 89., y + 16., 16., MUTED);
            ui::text(&ui::trim(&p.name, 225., 20), 124., y + 17., 20., TEXT);
            ui::text(
                if id == view.host {
                    "HOST"
                } else if id == view.you {
                    "YOU"
                } else if !p.connected {
                    "AWAY"
                } else {
                    "READY"
                },
                424.,
                y + 16.,
                13.,
                MUTED,
            );
        }
        let enabled = !self.rules && !self.leaving && !self.disconnected && !self.connecting;
        if view.you == view.host {
            if ui::button(
                "Deal the cards  >",
                Rect::new(62., 636., 422., 43.),
                enabled && view.players.len() >= 2 && !view.paused,
                true,
            ) {
                self.send(Action::Start);
            }
        } else {
            ui::center(
                "Waiting for the host to deal",
                Rect::new(62., 636., 422., 43.),
                18.,
                MUTED,
            );
        }
        if let Some(server) = &self.server {
            let link = server.invite(&self.address).link();
            ui::panel(Rect::new(550., 635., 677., 99.), PANEL);
            ui::text("YOUR PRIVATE INVITE", 570., 662., 14., GOLD);
            ui::text(&ui::trim(&link, 620., 16), 570., 691., 16., TEXT);
            ui::text(
                "Use Copy invite above. Friends paste it into Join a friend.",
                570.,
                716.,
                14.,
                MUTED,
            );
        } else {
            ui::text(
                "Seven cards each. First to 500 points wins the match.",
                608.,
                682.,
                19.,
                MUTED,
            );
        }
        ui::text("Keep this window open. Closing the host ends the table; disconnected players can rejoin from their open client.", 43., 771., 14., MUTED);
    }
    fn table(&mut self, view: &View) {
        let finished = matches!(
            view.phase,
            Phase::RoundOver { .. } | Phase::MatchOver { .. }
        );
        if finished {
            self.table_talk = false;
        }
        let returning = self.card_return.is_some();
        if self
            .card_return
            .as_ref()
            .is_some_and(|r| get_time() - r.start >= 0.28)
        {
            self.card_return = None;
        }
        let free = !self.rules
            && !self.table_talk
            && !self.leaving
            && self.choice.is_none()
            && !returning
            && self.pending_drag.is_none();
        let enabled = free && !self.disconnected && !self.connecting && !view.paused && !finished;
        if !enabled
            || view.turn != view.you
            || view.phase != Phase::Playing
            || view.pending_four.is_some()
        {
            self.card_drag = None;
        }
        let scene_rect = ui::screen_rect();
        // HUD regions must not click through, but resized-window margins remain usable.
        let m = ui::mouse();
        let over_table = scene_rect.contains(m)
            && !Rect::new(0., 0., 1280., 112.).contains(m)
            && !Rect::new(456., 704., 368., 96.).contains(m)
            && !Rect::new(scene_rect.right() - 76., 260., 76., 310.).contains(m)
            && !Rect::new(scene_rect.x + 12., 110., 152., 34.).contains(m)
            && !(self.player_menu && Rect::new(scene_rect.x + 12., 148., 250., 116.).contains(m))
            && !(self.show_roster
                && ui::player_badge_at(scene_rect, view.players.len(), m).is_some());
        let dragging =
            is_mouse_button_down(MouseButton::Right) || is_mouse_button_down(MouseButton::Middle);
        if !free || !dragging || self.card_drag.is_some() {
            self.camera_dragging = false;
        } else if (is_mouse_button_pressed(MouseButton::Right)
            || is_mouse_button_pressed(MouseButton::Middle))
            && over_table
        {
            self.camera_dragging = true;
            self.last_mouse = m;
        }
        if self.camera_dragging {
            if is_mouse_button_down(MouseButton::Middle)
                || is_key_down(KeyCode::LeftShift)
                || is_key_down(KeyCode::RightShift)
            {
                self.camera.pan(m - self.last_mouse);
            } else {
                self.camera.drag(m - self.last_mouse);
            }
        }
        if free && self.card_drag.is_none() {
            if over_table {
                self.camera.zoom(mouse_wheel().1);
            }
            let axis = |positive, negative| {
                (is_key_down(positive) as i32 - is_key_down(negative) as i32) as f32
            };
            let dt = get_frame_time().min(0.1);
            let orbit = vec2(
                axis(KeyCode::Left, KeyCode::Right),
                axis(KeyCode::Up, KeyCode::Down),
            );
            // Invert mouse Y only; retain the existing arrow/WASD directions.
            let pan = vec2(axis(KeyCode::D, KeyCode::A), axis(KeyCode::S, KeyCode::W));
            if orbit != Vec2::ZERO {
                self.camera.drag(orbit * dt * 180.);
            }
            if pan != Vec2::ZERO {
                self.camera.pan(pan * dt * 180.);
            }
            self.camera
                .zoom(axis(KeyCode::Equal, KeyCode::Minus) * dt * 8.);
        }
        self.last_mouse = m;
        self.camera.update(get_frame_time());
        let effects = self.motion.effects(view, get_time());
        let pointer = (enabled && over_table && !self.camera_dragging).then_some(m);
        let hit = pointer.and_then(|p| ui::table_hit(view, self.camera.pose, scene_rect, p)).filter(|hit| {
            !matches!(hit, ui::TableHit::Card(card) if view.hand.iter().position(|c| c == card).is_some_and(|i| effects.hidden.contains(&(view.you, i))))
        });
        if enabled && self.card_drag.is_none() && is_mouse_button_pressed(MouseButton::Left) {
            match hit {
                Some(ui::TableHit::Card(card))
                    if view.turn == view.you
                        && view.phase == Phase::Playing
                        && view.pending_four.is_none() =>
                {
                    let index = view.hand.iter().position(|c| c.id == card.id).unwrap();
                    let home = table_view::hand_card(
                        view.you,
                        view.you,
                        view.players.len(),
                        view.hand.len(),
                        index,
                    );
                    if let Some(point) = table_view::pointer_on_plane(
                        self.camera.pose,
                        scene_rect,
                        m,
                        home.position.y,
                    ) {
                        self.card_drag = Some(CardDrag {
                            card,
                            home,
                            pose: home,
                            offset: home.position - point,
                        });
                    }
                }
                Some(ui::TableHit::Deck) if view.can_draw => self.send(Action::Draw),
                _ => {}
            }
        }
        if let Some(drag) = &mut self.card_drag {
            if let Some(point) =
                table_view::pointer_on_plane(self.camera.pose, scene_rect, m, drag.home.position.y)
            {
                drag.pose.position = point + drag.offset;
                let distance = vec2(
                    drag.pose.position.x - drag.home.position.x,
                    drag.pose.position.z - drag.home.position.z,
                )
                .length();
                let t = (distance / 1.2).clamp(0., 1.);
                drag.pose.rotation = drag.home.rotation.slerp(Quat::IDENTITY, t);
                let height = drag.home.position.y * (1. - t) + 0.09 * t;
                if let Some(point) =
                    table_view::pointer_on_plane(self.camera.pose, scene_rect, m, height)
                {
                    drag.pose.position = point + drag.offset;
                }
            }
        }
        if self.card_drag.is_some()
            && (is_mouse_button_released(MouseButton::Left) || is_key_pressed(KeyCode::Escape))
        {
            let drag = self.card_drag.take().unwrap();
            let dropped = !is_key_pressed(KeyCode::Escape)
                && over_table
                && table_view::over_discard(self.camera.pose, scene_rect, m);
            if dropped && view.playable.contains(&drag.card.id) {
                let card = drag.card;
                self.pending_drag = Some(drag);
                self.play_card(card);
            } else {
                if dropped {
                    self.notify("That card cannot be played. Your turn continues.");
                }
                self.card_return = Some(CardReturn {
                    drag,
                    start: get_time(),
                    owner: view.you,
                    round: view.round,
                });
            }
        }
        let held = self
            .card_drag
            .as_ref()
            .map(|d| (d.card, d.pose))
            .or_else(|| self.pending_drag.as_ref().map(|d| (d.card, d.pose)))
            .or_else(|| {
                self.card_return.as_ref().map(|r| {
                    let t = ((get_time() - r.start) / 0.28).clamp(0., 1.) as f32;
                    let pose = r.drag.pose.interpolate(r.drag.home, t * t * (3. - 2. * t));
                    (r.drag.card, pose)
                })
            });
        self.art.scene(
            Some(view),
            scene_rect,
            self.camera.pose,
            Some(&effects),
            pointer,
            held,
        );
        ui::table_edge_shade();
        if self.show_roster {
            if let Some(seat) = ui::player_roster(
                view,
                scene_rect,
                free && !self.player_menu && self.card_drag.is_none() && !self.camera_dragging,
            ) {
                self.focused_seat = (seat + view.players.len() - view.you) % view.players.len();
                self.camera.focus_seat(seat, view.you, view.players.len());
            }
        } else {
            ui::player_names(view, self.camera.pose, scene_rect);
        }
        let menu_enabled = free && self.card_drag.is_none() && !self.camera_dragging;
        let menu_button = Rect::new(scene_rect.x + 12., 110., 152., 34.);
        let menu = Rect::new(scene_rect.x + 12., 148., 250., 116.);
        if ui::quiet_button(
            "Players / view",
            menu_button,
            menu_enabled,
            self.player_menu,
        ) {
            self.player_menu = !self.player_menu;
        }
        if self.player_menu {
            ui::panel(menu, INK);
            for (i, (label, roster)) in [("Names at the table", false), ("Cards and scores", true)]
                .into_iter()
                .enumerate()
            {
                if ui::quiet_button(
                    label,
                    Rect::new(
                        menu.x + 8.,
                        menu.y + 10. + i as f32 * 48.,
                        menu.w - 16.,
                        38.,
                    ),
                    menu_enabled,
                    self.show_roster == roster,
                ) {
                    self.show_roster = roster;
                    self.player_menu = false;
                }
            }
            if menu_enabled
                && ((is_mouse_button_pressed(MouseButton::Left)
                    && !menu.contains(m)
                    && !menu_button.contains(m))
                    || is_key_pressed(KeyCode::Escape))
            {
                self.player_menu = false;
            }
        }
        // Minimal text HUD; no hand panel or duplicate discard preview.
        let turn_name = &view.players[view.turn].name;
        let status = if self.disconnected {
            "Connection lost".into()
        } else if self.connecting {
            "Reconnecting to your table...".into()
        } else if view.paused {
            "Paused - waiting for a player".into()
        } else if self.self_play.is_some() {
            format!(
                "{}'s turn / seat {}: {}",
                turn_name,
                view.you + 1,
                view.players[view.you].name
            )
        } else if view.turn == view.you {
            "Your turn".into()
        } else {
            format!("{turn_name}'s turn")
        };
        ui::text(
            &ui::trim(&status, 620., 24),
            28.,
            40.,
            24.,
            if view.turn == view.you { ACCENT } else { TEXT },
        );
        ui::text(
            &format!(
                "Round {} / {} / {} in deck",
                view.round,
                if view.direction > 0 {
                    "clockwise"
                } else {
                    "counterclockwise"
                },
                view.deck_count
            ),
            28.,
            65.,
            14.,
            MUTED,
        );
        draw_circle(34., 89., 6., ui::suit(view.active_color));
        ui::text(
            &format!("{} is active", view.active_color.name()),
            48.,
            95.,
            16.,
            TEXT,
        );
        if let Some(local) = &mut self.self_play {
            if ui::quiet_button("Next hand", Rect::new(670., 16., 112., 34.), free, false) {
                local.next_hand();
            }
            if ui::quiet_button("Turn hand", Rect::new(790., 16., 112., 34.), free, false) {
                local.follow_turn();
            }
        } else if self.server.is_some()
            && ui::quiet_button("Copy invite", Rect::new(780., 16., 122., 34.), free, false)
        {
            self.copy_invite();
        }
        if ui::quiet_button(
            "History",
            Rect::new(918., 16., 100., 34.),
            free && !finished,
            false,
        ) || (free && !finished && is_key_pressed(KeyCode::H))
        {
            self.table_talk = true;
        }
        if ui::quiet_button("Rules", Rect::new(1030., 16., 92., 34.), free, false) {
            self.rules = true;
        }
        if ui::quiet_button("Leave", Rect::new(1134., 16., 118., 34.), free, false) {
            self.leaving = true;
        }
        self.camera_controls(view, free);
        // Layout is derived from visible actions, keeping every icon + caption
        // centered as a group even when +4 and UNO checks coexist.
        let late = view.vulnerable == Some(view.you);
        let mut actions = vec![(
            ui::Icon::Uno,
            if late {
                "Say UNO"
            } else if self.uno {
                "UNO armed"
            } else {
                "Call UNO"
            },
            if late {
                "Declare before the next player catches you"
            } else {
                "UNO with this play; a false declaration can cost 2 cards"
            }
            .to_string(),
            enabled && (late || (view.turn == view.you && view.pending_four.is_none())),
            late || self.uno,
            late.then_some(Action::DeclareUno),
        )];
        if view
            .pending_four
            .as_ref()
            .is_some_and(|four| four.victim == view.you)
        {
            actions.push((
                ui::Icon::Accept,
                "Take +4",
                "Accept four cards and lose your turn".into(),
                enabled,
                false,
                Some(Action::AcceptFour),
            ));
            actions.push((
                ui::Icon::Challenge,
                "Check +4",
                "Challenge +4; a failed challenge costs 6 cards".into(),
                enabled,
                false,
                Some(Action::ChallengeFour),
            ));
        } else if view.pending_two > 0 && view.turn == view.you {
            actions.push((
                ui::Icon::Accept,
                "Take cards",
                format!("Accept +{} and pass", view.pending_two),
                enabled && view.can_draw,
                false,
                Some(Action::Draw),
            ));
        }
        if view.turn == view.you {
            if let Some(target) = view.vulnerable.filter(|p| *p != view.you) {
                actions.push((
                    ui::Icon::Catch,
                    "Missed UNO",
                    "Catch a missing declaration: that player draws 2".into(),
                    enabled,
                    false,
                    Some(Action::CatchUno { player: target }),
                ));
            }
            if let Some(target) = view.declared_uno.filter(|p| *p != view.you) {
                actions.push((
                    ui::Icon::Challenge,
                    "Check UNO",
                    "Verify UNO; a false declaration costs that player 2 cards".into(),
                    enabled,
                    true,
                    Some(Action::CheckUno { player: target }),
                ));
            }
        }
        let count = actions.len();
        for (index, (icon, label, tooltip, available, active, action)) in
            actions.into_iter().enumerate()
        {
            if self.art.action_button(
                icon,
                label,
                &tooltip,
                ui::action_rect(index, count),
                available,
                active,
            ) {
                if let Some(action) = action {
                    self.send(action);
                } else {
                    self.uno = !self.uno;
                }
            }
        }
        self.table_talk_modal(view);
        if matches!(view.phase, Phase::OpeningColor)
            && view.turn == view.you
            && free
            && !self.rules
            && !self.leaving
            && !self.table_talk
            && !view.paused
            && !self.disconnected
            && !self.connecting
        {
            self.choice = Some(Choice::default());
        }
        match view.phase {
            Phase::RoundOver { winner, points } => self.round_over(view, winner, Some(points)),
            Phase::MatchOver { winner } => self.round_over(view, winner, None),
            _ => {}
        }
    }
    fn camera_controls(&mut self, view: &View, enabled: bool) {
        let area = ui::screen_rect();
        let reveal = Rect::new(area.right() - 90., 260., 90., 310.).contains(ui::mouse());
        let target = if reveal { 1. } else { 0. };
        self.camera_reveal +=
            (target - self.camera_reveal) * (1. - (-12. * get_frame_time().min(0.1)).exp());
        let x = area.right() - 10. - 54. * self.camera_reveal;
        let controls_enabled = enabled && reveal;
        if self.art.icon_button(
            ui::Icon::Seat,
            "My seat (1 / R)",
            Rect::new(x, 280., 48., 48.),
            controls_enabled,
            false,
        ) || (enabled && (is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Key1)))
        {
            self.camera.my_seat();
            self.focused_seat = 0;
        }
        if self.art.icon_button(
            ui::Icon::Above,
            "Above (2)",
            Rect::new(x, 336., 48., 48.),
            controls_enabled,
            false,
        ) || (enabled && is_key_pressed(KeyCode::Key2))
        {
            self.camera.overview();
        }
        if self.art.icon_button(
            ui::Icon::Cards,
            "My cards (3)",
            Rect::new(x, 392., 48., 48.),
            controls_enabled,
            false,
        ) || (enabled && is_key_pressed(KeyCode::Key3))
        {
            self.camera.hand();
        }
        let previous = self.art.icon_button(
            ui::Icon::Previous,
            "Previous seat",
            Rect::new(x, 448., 48., 48.),
            controls_enabled,
            false,
        );
        let next = self.art.icon_button(
            ui::Icon::Next,
            "Next seat",
            Rect::new(x, 504., 48., 48.),
            controls_enabled,
            false,
        );
        let count = view.players.len();
        if previous || next {
            self.focused_seat = (self.focused_seat + if next { 1 } else { count - 1 }) % count;
            self.camera
                .focus_seat((view.you + self.focused_seat) % count, view.you, count);
        }
    }
    fn table_talk_modal(&mut self, view: &View) {
        if !self.table_talk {
            return;
        }
        overlay();
        ui::panel(Rect::new(270., 180., 740., 420.), PANEL);
        ui::text("TABLE TALK / latest events", 294., 218., 22., GOLD);
        let can_close = !self.rules && !self.leaving && self.choice.is_none();
        if ui::button("Close", Rect::new(870., 193., 116., 32.), can_close, false)
            || (can_close && is_key_pressed(KeyCode::Escape))
        {
            self.table_talk = false;
        }
        let mut y = 253.;
        for (i, entry) in view.log.iter().rev().enumerate() {
            if y > 565. {
                break;
            }
            y = ui::wrap_bounded(
                entry,
                Rect::new(294., y, 690., 565. - y),
                18.,
                if i == 0 { TEXT } else { MUTED },
            );
            y += 15.;
        }
    }
    fn return_pending(&mut self) {
        if let (Some(drag), Some(view)) = (self.pending_drag.take(), &self.view) {
            let mut returning = CardReturn {
                drag,
                start: get_time(),
                owner: view.you,
                round: view.round,
            };
            if returning.refresh(view) {
                self.card_return = Some(returning);
            }
        }
    }
    fn play_card(&mut self, card: Card) {
        if card.face.wild() {
            self.choice = Some(Choice {
                card: Some(card),
                ..Default::default()
            });
        } else {
            self.send(Action::Play {
                card: card.id,
                color: None,
                target: None,
                uno: self.uno,
            });
        }
    }
    fn round_over(&mut self, view: &View, winner: usize, points: Option<u32>) {
        ui::panel(Rect::new(213., 222., 538., 239.), INK);
        ui::center(
            if points.is_some() {
                "ROUND COMPLETE"
            } else {
                "500 POINTS / MATCH WON"
            },
            Rect::new(225., 242., 514., 35.),
            16.,
            GOLD,
        );
        ui::center(
            &ui::trim(&format!("{} wins!", view.players[winner].name), 514., 34),
            Rect::new(225., 284., 514., 46.),
            34.,
            ACCENT,
        );
        let detail = points
            .map(|p| format!("+{p} points / {} total", view.players[winner].score))
            .unwrap_or_else(|| "A well-played match. Ready for a rematch?".into());
        ui::center(&detail, Rect::new(225., 337., 514., 30.), 19., TEXT);
        if view.you == view.host || self.self_play.is_some() {
            if ui::button(
                if points.is_some() {
                    "Next round  >"
                } else {
                    "Start a new match  >"
                },
                Rect::new(329., 393., 306., 43.),
                !self.rules
                    && !self.leaving
                    && !self.disconnected
                    && !self.connecting
                    && !view.paused,
                true,
            ) {
                self.send(Action::NextRound);
            }
        } else {
            ui::center(
                "Waiting for the host to deal again",
                Rect::new(225., 391., 514., 43.),
                18.,
                MUTED,
            );
        }
    }
    fn choice_modal(&mut self, view: &View) {
        let Some(mut choice) = self.choice.take() else {
            return;
        };
        if self.rules || self.leaving {
            self.choice = Some(choice);
            return;
        }
        overlay();
        ui::panel(Rect::new(340., 163., 600., 474.), PANEL);
        let opening = choice.card.is_none();
        let targeted = choice.card.is_some_and(|c| c.face == Face::TargetTwo);
        let title = choice
            .card
            .map(|c| format!("Play {}", c.face.label()))
            .unwrap_or_else(|| "Opening wild card".into());
        ui::text(&title, 372., 211., 30., TEXT);
        ui::text("CHOOSE THE NEXT COLOR", 373., 254., 15., MUTED);
        for (i, color) in Suit::ALL.into_iter().enumerate() {
            let rect = Rect::new(373. + i as f32 * 138., 272., 120., 58.);
            ui::panel(rect, ui::suit(color));
            ui::center(color.name(), rect, 19., WHITE);
            if choice.color == Some(color) {
                draw_rectangle_lines(rect.x - 3., rect.y - 3., rect.w + 6., rect.h + 6., 3., TEXT);
            }
            if rect.contains(ui::mouse()) && is_mouse_button_pressed(MouseButton::Left) {
                choice.color = Some(color);
            }
        }
        if targeted {
            ui::text(
                "WHO DRAWS TWO? (THEIR TURN IS NOT SKIPPED)",
                373.,
                364.,
                14.,
                MUTED,
            );
            let opponents: Vec<_> = view
                .players
                .iter()
                .enumerate()
                .filter(|(id, _)| *id != view.you)
                .collect();
            for (i, (id, player)) in opponents.into_iter().enumerate() {
                let rect = Rect::new(
                    373. + (i % 3) as f32 * 184.,
                    385. + (i / 3) as f32 * 47.,
                    170.,
                    37.,
                );
                if ui::button(
                    &ui::trim(&player.name, 150., 18),
                    rect,
                    true,
                    choice.target == Some(id),
                ) {
                    choice.target = Some(id);
                }
            }
        } else {
            let explanation = if choice.card.is_some_and(|c| c.face == Face::WildFour) {
                "Wild +4 can be challenged. If you hold a card of the active color, a successful challenge makes YOU draw four. Matching just the number is allowed."
            } else if opening {
                "Choose the active color, then take your turn."
            } else {
                "A wild can be played on any color. Your chosen color becomes active for the next player."
            };
            ui::wrap(explanation, 373., 380., 530., 19., MUTED);
        }
        let mut close = false;
        if ui::button(
            if opening { "Leave table" } else { "Cancel" },
            Rect::new(373., 566., 162., 43.),
            true,
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            self.leaving = opening;
            self.return_pending();
            close = true;
        }
        let valid = !close
            && choice.color.is_some()
            && (!targeted || choice.target.is_some())
            && !view.paused
            && !self.disconnected
            && !self.connecting;
        if ui::button(
            if opening { "Choose color" } else { "Play card" },
            Rect::new(551., 566., 356., 43.),
            valid,
            true,
        ) {
            if let Some(card) = choice.card {
                self.send(Action::Play {
                    card: card.id,
                    color: choice.color,
                    target: choice.target,
                    uno: self.uno,
                });
            } else {
                self.send(Action::ChooseOpeningColor {
                    color: choice.color.unwrap(),
                });
            }
            close = true;
        }
        if !close {
            self.choice = Some(choice);
        }
    }
    fn rules_modal(&mut self) {
        if !self.rules {
            return;
        }
        overlay();
        ui::panel(Rect::new(250., 100., 780., 609.), PANEL);
        ui::text("A familiar game. A few house rules.", 282., 151., 30., TEXT);
        let entries = [
            ("01  MATCH & WIN", "Drag a card onto the discard: match color or face, or play a wild. A wrong card returns to your hand without ending your turn. Empty your hand to win."),
            ("02  ONE CARD AT A TIME", "Click the deck to draw exactly one card, even if you can already play. Play any matching card or draw again; there is no voluntary pass. Drop outside the discard or press Esc to return a card."),
            ("03  THE ? CARD", "Choose a new color AND any opponent to draw two immediately. That opponent keeps their turn. Four ? cards are added to the classic 108-card deck."),
            ("04  ACTION CARDS", "Stack any +2, or draw the total and pass. Reverse has no effect with two players. Skip skips a turn. +4: accept and pass, or challenge. Active color held: actor draws 4; otherwise you draw 6 and pass."),
            ("05  SAY UNO", "Arm UNO before playing. Next player may check: a false UNO costs you 2 cards. If you leave one card without calling, they may catch you for 2. Say it late before being caught. Checks close on the next valid action."),
            ("06  SCORE TO 500", "The winner scores all opponents' cards: number = face value, actions = 20, wilds (including ?) = 50. Final-card penalties resolve before scoring."),
        ];
        for (i, (title, body)) in entries.into_iter().enumerate() {
            let x = if i < 3 { 283. } else { 653. };
            let y = 195. + (i % 3) as f32 * 150.;
            ui::text(title, x, y, 14., GOLD);
            ui::wrap(body, x, y + 24., 339., 15., TEXT);
        }
        if ui::button(
            "Back to the table",
            Rect::new(737., 651., 261., 39.),
            !self.leaving,
            true,
        ) || (!self.leaving && is_key_pressed(KeyCode::Escape))
        {
            self.rules = false;
        }
    }
    fn leave_modal(&mut self) -> bool {
        if !self.leaving {
            return false;
        }
        overlay();
        ui::panel(Rect::new(376., 253., 528., 250.), PANEL);
        ui::text("Leave this table?", 408., 302., 30., TEXT);
        ui::wrap(
            if self.self_play.is_some() {
                "This is an offline self-play table. Leaving discards the match for all local seats."
            } else if self.server.is_some() {
                "You are hosting. Leaving shuts down the room for everyone and discards this match."
            } else {
                "Leaving forgets your rejoin session and pauses the table. To recover from a network interruption, use Rejoin instead."
            },
            408.,
            341.,
            457.,
            19.,
            MUTED,
        );
        if ui::button("Stay", Rect::new(408., 430., 219., 43.), true, true) {
            self.leaving = false;
            self.exit_requested = false;
        }
        if ui::button("Leave", Rect::new(644., 430., 228., 43.), true, false) {
            self.leave();
            return self.exit_requested;
        }
        false
    }
    fn connection_banner(&mut self) {
        if (!self.disconnected && !self.connecting)
            || self.view.is_none()
            || self.rules
            || self.leaving
            || self.table_talk
            || self.choice.is_some()
        {
            return;
        }
        ui::panel(Rect::new(328., 149., 604., 61.), INK);
        ui::text(
            if self.connecting {
                "Reconnecting. Waiting for your hand..."
            } else {
                "Connection lost. Your seat is reserved."
            },
            342.,
            185.,
            19.,
            ERROR,
        );
        if ui::button(
            "Rejoin",
            Rect::new(794., 163., 125., 35.),
            !self.connecting,
            true,
        ) {
            if let Some(invite) = self.invite.clone() {
                self.connect(invite);
            }
        }
    }
    fn toast(&self) {
        let (message, color) = if !self.error.is_empty() {
            (self.error.as_str(), ERROR)
        } else if get_time() < self.notice_until {
            (self.notice.as_str(), ACCENT)
        } else {
            return;
        };
        ui::panel(Rect::new(230., 6., 818., 53.), INK);
        ui::wrap(message, 245., 29., 788., 16., color);
    }
}
fn overlay() {
    draw_rectangle(-2000., -2000., 6000., 6000., Color::new(0., 0., 0., 0.72));
}

/// Opt-in GPU smoke test: render real UI into PNGs without starting a network room.
async fn render_smoke(app: &mut App, directory: &str) {
    std::fs::create_dir_all(directory).expect("Create screenshot directory");
    let mut game = uno::game::Game::new(23);
    for name in ["Alex", "Morgan", "Jamie", "You"] {
        game.add_player(name.into()).unwrap();
    }
    game.act(0, Action::Start).unwrap();
    let mut crowded = uno::game::Game::new(23);
    for id in 0..10 {
        crowded
            .add_player(format!("Player {} long name", id + 1))
            .unwrap();
    }
    crowded.act(0, Action::Start).unwrap();
    let mut pair = uno::game::Game::new(23);
    for name in ["Alex", "Morgan"] {
        pair.add_player(name.into()).unwrap();
    }
    pair.act(0, Action::Start).unwrap();
    for stage in [
        "home",
        "home-self-play",
        "self-play-two",
        "self-play-ten",
        "table",
        "table-ten",
        "table-roster",
        "table-player-menu",
        "table-uno-check",
        "table-uno-armed",
        "table-two",
        "table-seat",
        "table-zoom",
        "table-above",
        "table-hand",
        "table-large-hand",
        "table-pan",
        "table-talk",
        "table-four",
        "table-draw-flight",
        "table-play-flight",
        "table-return-flight",
        "opening-color",
        "reconnecting",
        "paused-opening",
        "round-over",
        "rules",
        "choice",
    ] {
        app.leave();
        app.home_tab = if stage == "home-self-play" {
            HomeTab::SelfPlay
        } else {
            HomeTab::Host
        };
        if stage.starts_with("self-play-") {
            app.local_players = if stage == "self-play-ten" { 10 } else { 2 };
            app.start_self_play();
            app.poll();
            assert!(app.active());
            assert!(app.client.is_none() && app.server.is_none());
        }
        app.camera = table_view::TableCamera::default();
        app.choice = None;
        app.table_talk = stage == "table-talk";
        app.show_roster = stage == "table-roster";
        app.player_menu = stage == "table-player-menu";
        app.uno = stage == "table-uno-armed";
        if stage == "table-seat" {
            app.camera.focus_seat(9, 0, 10);
        } else if stage == "table-zoom" {
            app.camera.drag(vec2(150., 1000.));
            app.camera.zoom(1000.);
        } else if stage == "table-above" {
            app.camera.overview();
        } else if stage == "table-hand" || stage == "table-large-hand" {
            app.camera.hand();
        } else if stage == "table-pan" {
            app.camera.pan(vec2(180., -80.));
        }
        for _ in 0..120 {
            app.camera.update(0.016);
        }
        for frame in 0..3 {
            clear_background(INK);
            ui::camera();
            app.rules = stage == "rules";
            if stage.starts_with("home") {
                app.home();
            } else {
                let mut view = match stage {
                    "self-play-two" | "self-play-ten" => app.view.clone().unwrap(),
                    "table-ten" | "table-seat" | "table-zoom" | "table-roster" => crowded.view(0),
                    "table-two" => pair.view(0),
                    _ => game.view(game.view(0).turn),
                };
                if view.players.len() == 10 && app.self_play.is_none() {
                    // Exercise public count/connection indicators without exposing hands.
                    view.players[3].cards = 1;
                    view.players[5].cards = 37;
                    view.players[8].connected = false;
                }
                if stage == "table-large-hand" {
                    view.hand = uno::game::deck().into_iter().take(37).collect();
                    view.players[view.you].cards = view.hand.len();
                    view.playable = view.hand.iter().map(|card| card.id).collect();
                }
                match stage {
                    "table-uno-check" => {
                        view.declared_uno = Some((view.you + 1) % view.players.len());
                        view.pending_four = Some(uno::game::FourView {
                            actor: (view.you + 1) % view.players.len(),
                            victim: view.you,
                        });
                    }
                    "table-four" => {
                        view.pending_four = Some(uno::game::FourView {
                            actor: (view.you + 1) % view.players.len(),
                            victim: view.you,
                        });
                        view.vulnerable = Some((view.you + 1) % view.players.len());
                        view.can_draw = false;
                        view.playable.clear();
                    }
                    "opening-color" | "paused-opening" => {
                        view.phase = Phase::OpeningColor;
                        view.paused = stage == "paused-opening";
                    }
                    "reconnecting" => app.connecting = true,
                    "round-over" => {
                        view.phase = Phase::RoundOver {
                            winner: view.you,
                            points: 42,
                        }
                    }
                    _ => {}
                }
                if stage == "table-draw-flight" {
                    let old = view.clone();
                    let card = uno::game::deck()
                        .into_iter()
                        .find(|c| !view.hand.contains(c) && Some(*c) != view.top)
                        .unwrap();
                    view.hand.push(card);
                    view.players[view.you].cards += 1;
                    if frame == 0 {
                        app.motion.observe(&old, &view, None, get_time() - 0.15);
                    }
                } else if stage == "table-play-flight" {
                    let mut old = view.clone();
                    old.turn = (view.you + 1) % view.players.len();
                    view.top = uno::game::deck()
                        .into_iter()
                        .find(|c| !view.hand.contains(c) && Some(*c) != view.top);
                    view.players[old.turn].cards -= 1;
                    if frame == 0 {
                        app.motion.observe(&old, &view, None, get_time() - 0.15);
                    }
                } else if stage == "table-return-flight" {
                    let home = table_view::hand_card(
                        view.you,
                        view.you,
                        view.players.len(),
                        view.hand.len(),
                        0,
                    );
                    app.card_return = Some(CardReturn {
                        drag: CardDrag {
                            card: view.hand[0],
                            home,
                            pose: table_view::CardPose::flat(vec3(0.6, 0.1, 0.3), 0., home.size),
                            offset: Vec3::ZERO,
                        },
                        start: get_time() - 0.10,
                        owner: view.you,
                        round: view.round,
                    });
                }
                app.view = Some(view.clone());
                app.table(&view);
                if stage == "choice" {
                    app.choice = Some(Choice {
                        card: uno::game::deck()
                            .into_iter()
                            .find(|c| c.face == Face::TargetTwo),
                        color: Some(Suit::Green),
                        target: Some((view.you + 1) % 4),
                    });
                }
                app.choice_modal(&view);
                app.connection_banner();
                app.rules_modal();
            }
            set_default_camera();
            if frame == 2 {
                get_screen_data().export_png(&format!("{directory}/{stage}.png"));
            }
            next_frame().await;
        }
    }
    // Exercise the actual hover materials without relying on the OS pointer.
    let view = game.view(game.view(0).turn);
    let pose = table_view::TablePose::default();
    let area = ui::screen_rect();
    for (name, position, expected) in [
        (
            "deck-hover",
            vec3(-0.85, (view.deck_count as f32 * 0.002).min(0.18) + 0.03, 0.),
            ui::TableHit::Deck,
        ),
        (
            "card-hover",
            table_view::hand_card(
                view.you,
                view.you,
                view.players.len(),
                view.hand.len(),
                view.hand.len() - 1,
            )
            .position,
            ui::TableHit::Card(*view.hand.last().unwrap()),
        ),
    ] {
        for frame in 0..3 {
            clear_background(INK);
            let hit = app.art.scene(
                Some(&view),
                area,
                pose,
                None,
                pose.project(position, area),
                None,
            );
            assert_eq!(hit, Some(expected));
            ui::table_edge_shade();
            ui::player_names(&view, pose, area);
            app.art.action_button(
                ui::Icon::Uno,
                "UNO armed",
                "UNO with play",
                ui::action_rect(0, 1),
                true,
                true,
            );
            set_default_camera();
            if frame == 2 {
                get_screen_data().export_png(&format!("{directory}/{name}.png"));
            }
            next_frame().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn returning_card() -> (CardReturn, View) {
        let mut game = uno::game::Game::new(23);
        for name in ["Host", "Guest"] {
            game.add_player(name.into()).unwrap();
        }
        game.act(0, Action::Start).unwrap();
        let mut view = game.view(0);
        view.phase = Phase::Playing;
        let home = table_view::hand_card(0, 0, 2, view.hand.len(), 1);
        (
            CardReturn {
                drag: CardDrag {
                    card: view.hand[1],
                    home,
                    pose: home,
                    offset: Vec3::ZERO,
                },
                start: 0.,
                owner: view.you,
                round: view.round,
            },
            view,
        )
    }

    #[test]
    fn action_controls_are_centered_and_never_overlap() {
        for count in 1..=4 {
            let first = ui::action_rect(0, count);
            let last = ui::action_rect(count - 1, count);
            assert_eq!((first.x + last.right()) / 2., 640.);
            for i in 1..count {
                assert!(!ui::action_rect(i - 1, count).overlaps(&ui::action_rect(i, count)));
            }
        }
    }

    #[test]
    fn return_animation_survives_penalty_draws_and_turn_changes() {
        let (mut returning, mut view) = returning_card();
        let before = returning.drag.home.position;
        view.hand.push(view.top.unwrap());
        view.turn = 1;
        assert!(returning.refresh(&view));
        assert_ne!(returning.drag.home.position, before);
        assert_eq!(
            returning.drag.home.position,
            table_view::hand_card(0, 0, 2, view.hand.len(), 1).position
        );
        // Pausing affects authority, not completion of an already submitted animation.
        view.paused = true;
        assert!(returning.refresh(&view));
    }

    #[test]
    fn return_animation_never_leaks_into_another_hand_or_round() {
        let changes: &[fn(&mut View)] = &[
            |v| v.you = 1,
            |v| v.round += 1,
            |v| {
                v.phase = Phase::RoundOver {
                    winner: 0,
                    points: 0,
                }
            },
            |v| {
                v.hand.remove(1);
            },
        ];
        for change in changes {
            let (mut returning, mut view) = returning_card();
            change(&mut view);
            assert!(!returning.refresh(&view));
        }
    }
}

#[macroquad::main(config)]
async fn main() {
    prevent_quit();
    let mut app = App::new();
    if let Ok(directory) = std::env::var("UNO_SCREENSHOT_DIR") {
        render_smoke(&mut app, &directory).await;
        return;
    }
    loop {
        app.poll();
        if is_quit_requested() {
            if app.active() {
                app.leaving = true;
                app.exit_requested = true;
            } else {
                break;
            }
        }
        if is_key_pressed(KeyCode::F11) {
            app.fullscreen = !app.fullscreen;
            miniquad::window::set_fullscreen(app.fullscreen);
        }
        clear_background(INK);
        ui::camera();
        let view = app.view.clone();
        if !app.active() {
            app.home();
        } else if let Some(view) = &view {
            if view.phase == Phase::Lobby {
                app.lobby(view);
            } else {
                app.table(view);
            }
            app.choice_modal(view);
        } else {
            app.connecting();
        }
        app.connection_banner();
        app.rules_modal();
        if app.leave_modal() {
            break;
        }
        app.toast();
        // Do not let typing outside a field leak into the next focused field.
        clear_input_queue();
        set_default_camera();
        next_frame().await;
    }
}

use crate::table_view::{
    hand_card, seat_angle, seat_position, CardPose, TablePose, FIELD_OF_VIEW, TABLE_RADIUS,
};
use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Comparison, Equation};
use macroquad::prelude::*;
use std::collections::HashMap;
use uno::game::{self, Card, Color as Suit, View};

pub const INK: Color = Color::new(0.035, 0.065, 0.085, 1.0);
pub const PANEL: Color = Color::new(0.065, 0.105, 0.13, 1.0);
pub const LINE: Color = Color::new(0.17, 0.25, 0.28, 1.0);
pub const TEXT: Color = Color::new(0.93, 0.94, 0.88, 1.0);
pub const MUTED: Color = Color::new(0.52, 0.65, 0.66, 1.0);
pub const ACCENT: Color = Color::new(0.65, 0.94, 0.64, 1.0);
pub const GOLD: Color = Color::new(1.0, 0.79, 0.36, 1.0);
pub const ERROR: Color = Color::new(1.0, 0.48, 0.43, 1.0);

pub fn suit(color: Suit) -> Color {
    match color {
        Suit::Red => Color::from_hex(0xdd514c),
        Suit::Yellow => Color::from_hex(0xe6b83f),
        Suit::Green => Color::from_hex(0x368968),
        Suit::Blue => Color::from_hex(0x397ec5),
    }
}
pub fn scale() -> f32 {
    (screen_width() / 1280.).min(screen_height() / 800.)
}
pub fn origin() -> Vec2 {
    vec2(
        (screen_width() - 1280. * scale()) / 2.,
        (screen_height() - 800. * scale()) / 2.,
    )
}
/// Full window in UI coordinates, including ultrawide/tall letterbox margins.
pub fn screen_rect() -> Rect {
    let o = origin() / scale();
    Rect::new(
        -o.x,
        -o.y,
        screen_width() / scale(),
        screen_height() / scale(),
    )
}
pub fn mouse() -> Vec2 {
    (Vec2::from(mouse_position()) - origin()) / scale()
}
pub fn camera() {
    let s = scale();
    let o = origin();
    let mut camera = Camera2D::from_display_rect(Rect::new(
        -o.x / s,
        -o.y / s,
        screen_width() / s,
        screen_height() / s,
    ));
    // Screen cameras invert Y internally; our UI uses top-left coordinates.
    camera.zoom.y *= -1.;
    set_camera(&camera);
}
pub fn text(value: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        value,
        x,
        y,
        TextParams {
            font_size: 64,
            font_scale: size / 64.,
            color,
            ..Default::default()
        },
    );
}
fn measure(value: &str, size: f32) -> TextDimensions {
    measure_text(value, None, 64, size / 64.)
}
pub fn center(value: &str, rect: Rect, size: f32, color: Color) {
    let m = measure(value, size);
    text(
        value,
        rect.x + (rect.w - m.width) / 2.,
        rect.y + (rect.h + size * 0.68) / 2.,
        size,
        color,
    );
}
pub fn trim(value: &str, width: f32, size: u16) -> String {
    if measure(value, size as f32).width <= width {
        return value.into();
    }
    let mut s = value.to_string();
    while !s.is_empty() && measure(&format!("{s}..."), size as f32).width > width {
        s.pop();
    }
    format!("{s}...")
}
pub fn wrap(value: &str, x: f32, y: f32, width: f32, size: f32, color: Color) -> f32 {
    wrap_bounded(value, Rect::new(x, y, width, f32::INFINITY), size, color)
}
/// Wrap between baseline bounds; never draw a line below the supplied rectangle.
pub fn wrap_bounded(value: &str, bounds: Rect, size: f32, color: Color) -> f32 {
    let (x, mut y, width) = (bounds.x, bounds.y, bounds.w);
    let mut line = String::new();
    for word in value.split_whitespace() {
        let next = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if measure(&next, size).width > width && !line.is_empty() {
            text(&trim(&line, width, size as u16), x, y, size, color);
            y += size * 1.35;
            if y > bounds.y + bounds.h {
                return y;
            }
            line = word.into();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        text(&trim(&line, width, size as u16), x, y, size, color);
        y += size * 1.35;
    }
    y
}
pub fn panel(rect: Rect, color: Color) {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1., LINE);
}
pub fn button(label: &str, rect: Rect, enabled: bool, primary: bool) -> bool {
    let hover = enabled && rect.contains(mouse());
    let bg = if !enabled {
        Color::new(0.08, 0.12, 0.14, 1.)
    } else if primary {
        if hover {
            TEXT
        } else {
            ACCENT
        }
    } else if hover {
        LINE
    } else {
        PANEL
    };
    draw_rectangle(
        rect.x,
        rect.y + 3.,
        rect.w,
        rect.h,
        Color::new(0., 0., 0., 0.2),
    );
    panel(rect, bg);
    center(
        label,
        rect,
        18.,
        if !enabled {
            MUTED
        } else if primary {
            INK
        } else {
            TEXT
        },
    );
    hover && is_mouse_button_pressed(MouseButton::Left)
}
/// Soft edge fades keep floating controls legible, including at extreme close-ups.
pub fn table_edge_shade() {
    let area = screen_rect();
    let clear = Color::new(INK.r, INK.g, INK.b, 0.);
    let dark = Color::new(INK.r, INK.g, INK.b, 0.92);
    for (top, bottom, a, b) in [
        (area.y, 120., dark, clear),
        (680., area.y + area.h, clear, dark),
    ] {
        draw_mesh(&Mesh {
            vertices: vec![
                Vertex::new2(vec3(area.x, top, 0.), Vec2::ZERO, a),
                Vertex::new2(vec3(area.x + area.w, top, 0.), Vec2::ZERO, a),
                Vertex::new2(vec3(area.x + area.w, bottom, 0.), Vec2::ZERO, b),
                Vertex::new2(vec3(area.x, bottom, 0.), Vec2::ZERO, b),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: None,
        });
    }
}
/// Lightweight in-game controls: text on the scene, no persistent panels.
pub fn quiet_button(label: &str, rect: Rect, enabled: bool, active: bool) -> bool {
    let hover = enabled && rect.contains(mouse());
    center(
        label,
        rect,
        17.,
        if !enabled {
            MUTED
        } else if hover {
            GOLD
        } else if active {
            ACCENT
        } else {
            TEXT
        },
    );
    if hover || (enabled && active) {
        draw_line(
            rect.x + 8.,
            rect.y + rect.h - 2.,
            rect.x + rect.w - 8.,
            rect.y + rect.h - 2.,
            1.,
            if hover { GOLD } else { ACCENT },
        );
    }
    hover && is_mouse_button_pressed(MouseButton::Left)
}
#[derive(Clone, Copy, PartialEq)]
pub enum Field {
    Name,
    Invite,
    Address,
    Port,
}
#[derive(Default)]
pub struct InputFocus {
    field: Option<Field>,
    selected: bool,
}

impl InputFocus {
    fn delete_selection(&mut self, value: &mut String) -> bool {
        if self.selected {
            value.clear();
            self.selected = false;
            true
        } else {
            false
        }
    }
}

pub fn input(
    label: &str,
    rect: Rect,
    value: &mut String,
    id: Field,
    focus: &mut InputFocus,
    limit: usize,
) -> Result<(), String> {
    let mut result = Ok(());
    text(label, rect.x, rect.y - 10., 15., MUTED);
    if rect.contains(mouse()) && is_mouse_button_pressed(MouseButton::Left) {
        focus.field = Some(id);
        focus.selected = false;
    }
    let active = focus.field == Some(id);
    panel(
        rect,
        if active {
            Color::from_hex(0x192b32)
        } else {
            INK
        },
    );
    if active {
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2., ACCENT);
    }
    if active {
        let modifier = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::RightControl)
            || is_key_down(KeyCode::LeftSuper)
            || is_key_down(KeyCode::RightSuper);
        // Always consume shortcut characters too; otherwise a queued 'v' or 'c'
        // can be inserted on a later frame after the modifier is released.
        let mut typed = Vec::new();
        while let Some(ch) = get_char_pressed() {
            typed.push(ch);
        }
        if modifier && is_key_pressed(KeyCode::V) {
            match crate::clipboard::get() {
                Ok(paste) => {
                    *value = crate::clipboard::field_text(&paste, limit);
                    focus.selected = false;
                }
                Err(error) => result = Err(error),
            }
        } else if modifier && is_key_pressed(KeyCode::C) {
            result = crate::clipboard::set(value);
        } else if modifier && is_key_pressed(KeyCode::X) {
            result = crate::clipboard::set(value);
            if result.is_ok() {
                value.clear();
                focus.selected = false;
            }
        } else if modifier && is_key_pressed(KeyCode::A) {
            focus.selected = true;
        } else if !modifier {
            if is_key_pressed(KeyCode::Backspace) && !focus.delete_selection(value) {
                value.pop();
            }
            if is_key_pressed(KeyCode::Delete) {
                focus.delete_selection(value);
            }
            // Macroquad 0.4.14 returns queued characters newest-first.
            for ch in typed.into_iter().rev().filter(|c| !c.is_control()) {
                focus.delete_selection(value);
                if value.chars().count() < limit {
                    value.push(ch);
                }
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
            *focus = InputFocus::default();
        }
    }
    let mut shown = value.clone();
    while measure(&shown, 18.).width > rect.w - 28. && !shown.is_empty() {
        shown.remove(0);
    }
    if active && focus.selected {
        draw_rectangle(
            rect.x + 11.,
            rect.y + 8.,
            measure(&shown, 18.).width + 2.,
            28.,
            LINE,
        );
    }
    text(&shown, rect.x + 12., rect.y + 29., 18., TEXT);
    if active && !focus.selected && (get_time() * 2.) as i64 % 2 == 0 {
        let x = rect.x + 14. + measure(&shown, 18.).width;
        draw_line(x, rect.y + 12., x, rect.y + 32., 1., ACCENT);
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableHit {
    Card(Card),
    Deck,
}

fn hand_face(view: &View, seat: usize, index: usize) -> Option<Card> {
    if seat == view.you {
        view.hand.get(index).copied()
    } else {
        None
    }
}

/// Resolve the closest physical card, not a screen-aligned bounding box.
/// Opponent backs and the discard occlude picks but never expose a private face.
pub fn table_hit(view: &View, pose: TablePose, area: Rect, pointer: Vec2) -> Option<TableHit> {
    let (origin, direction) = pose.ray(pointer, area);
    let mut closest = f32::INFINITY;
    let mut result = None;
    let mut test = |geometry: CardPose, hit: Option<TableHit>| {
        if let Some(distance) = geometry.hit(origin, direction) {
            if distance < closest {
                closest = distance;
                result = hit;
            }
        }
    };
    if view.deck_count > 0 || view.can_draw {
        test(
            CardPose::flat(
                vec3(-0.85, (view.deck_count as f32 * 0.002).min(0.18) + 0.03, 0.),
                0.,
                1.25,
            ),
            Some(TableHit::Deck),
        );
    }
    test(CardPose::flat(vec3(0.85, 0.045, 0.), -0.07, 1.25), None);
    for (id, player) in view.players.iter().enumerate() {
        let count = if id == view.you {
            view.hand.len()
        } else {
            player.cards
        };
        for index in 0..count {
            let geometry = hand_card(id, view.you, view.players.len(), count, index);
            let card = hand_face(view, id, index)
                .filter(|_| geometry.face_visible(pose.position()))
                .map(TableHit::Card);
            test(geometry, card);
        }
    }
    result
}

#[derive(Clone, Copy)]
pub enum Icon {
    Seat,
    Above,
    Cards,
    Uno,
    Accept,
    Challenge,
    Catch,
    Previous,
    Next,
}

/// Center the complete action group, including conditional buttons.
pub fn action_rect(index: usize, count: usize) -> Rect {
    Rect::new(
        640. - count as f32 * 44. + index as f32 * 88. + 20.,
        720.,
        48.,
        48.,
    )
}

pub struct Art {
    cards: HashMap<String, RenderTarget>,
    back: RenderTarget,
    icon_shader: Material,
    glow_shader: Material,
    ghost_shader: Material,
}
impl Art {
    pub fn new() -> Self {
        // Populate one font size BEFORE issuing GPU draws. Atlas growth during a
        // batch invalidates previously queued glyph UVs on some OpenGL drivers.
        let ascii: String = (32u8..127).map(char::from).collect();
        let _ = measure(&ascii, 64.);
        let mut cards = HashMap::new();
        for card in game::deck() {
            cards
                .entry(card.label())
                .or_insert_with(|| make_card(Some(card)));
        }
        let back = make_card(None);
        set_default_camera();
        let icon_shader = load_material(
            ShaderSource::Glsl {
                vertex: ICON_VERTEX,
                fragment: ICON_FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("State", UniformType::Float2),
                    UniformDesc::new("Time", UniformType::Float1),
                ],
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("Icon shader");
        let glow_shader = load_material(
            ShaderSource::Glsl {
                vertex: ICON_VERTEX,
                fragment: GLOW_FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("Time", UniformType::Float1),
                    UniformDesc::new("Strength", UniformType::Float1),
                ],
                pipeline_params: PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: false,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::One,
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("Card glow shader");
        let ghost_shader = load_material(
            ShaderSource::Glsl {
                vertex: ICON_VERTEX,
                fragment: GHOST_FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![UniformDesc::new("Opacity", UniformType::Float1)],
                pipeline_params: PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: false,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("Transparent hand shader");
        Self {
            cards,
            back,
            icon_shader,
            glow_shader,
            ghost_shader,
        }
    }
    pub fn icon_button(
        &self,
        icon: Icon,
        tooltip: &str,
        rect: Rect,
        enabled: bool,
        active: bool,
    ) -> bool {
        let hover = rect.contains(mouse());
        self.icon_shader.set_uniform(
            "State",
            (
                if active && enabled { 1.0f32 } else { 0.0 },
                if !enabled {
                    -1.0f32
                } else if hover {
                    1.0
                } else {
                    0.0
                },
            ),
        );
        self.icon_shader.set_uniform("Time", get_time() as f32);
        gl_use_material(&self.icon_shader);
        draw_rectangle(
            rect.x - 12.,
            rect.y - 12.,
            rect.w + 24.,
            rect.h + 24.,
            WHITE,
        );
        gl_use_default_material();
        let c = if !enabled {
            MUTED
        } else if active {
            ACCENT
        } else if hover {
            GOLD
        } else {
            TEXT
        };
        let x = rect.x + rect.w / 2.;
        let y = rect.y + rect.h / 2.;
        match icon {
            Icon::Uno => {
                draw_poly(x, y, 6, 18., 30., Color::new(c.r, c.g, c.b, 0.12));
                draw_poly_lines(x, y, 6, 18., 30., 1.8, c);
                draw_line(x - 10., y + 9., x + 10., y + 9., 1.5, c);
                let m = measure("UNO", 14.);
                text(
                    "UNO",
                    x - m.width / 2.,
                    y + m.offset_y - m.height / 2.,
                    14.,
                    c,
                );
            }
            Icon::Accept => {
                draw_line(x - 8., y, x - 2., y + 6., 2., c);
                draw_line(x - 2., y + 6., x + 9., y - 7., 2., c);
            }
            Icon::Challenge => {
                draw_circle_lines(x, y, 11., 1.6, c);
                center("?", rect, 21., c);
            }
            Icon::Catch => {
                draw_circle_lines(x, y, 11., 1.6, c);
                center("!", rect, 21., c);
            }
            Icon::Above => {
                draw_rectangle_lines(x - 10., y - 8., 20., 16., 1.6, c);
                draw_circle(x, y, 2.5, c);
            }
            Icon::Seat => {
                draw_circle_lines(x, y - 5., 4., 1.6, c);
                draw_line(x - 9., y + 8., x + 9., y + 8., 1.6, c);
                draw_line(x - 9., y + 8., x - 5., y + 1., 1.6, c);
                draw_line(x + 9., y + 8., x + 5., y + 1., 1.6, c);
            }
            Icon::Cards => {
                for offset in [-6., 0., 6.] {
                    draw_rectangle_lines(
                        x - 5. + offset,
                        y - 8. + offset.abs() * 0.3,
                        10.,
                        16.,
                        1.4,
                        c,
                    );
                }
            }
            Icon::Previous | Icon::Next => {
                let d = if matches!(icon, Icon::Next) { 1. } else { -1. };
                draw_line(x - d * 4., y - 7., x + d * 4., y, 1.8, c);
                draw_line(x + d * 4., y, x - d * 4., y + 7., 1.8, c);
            }
        }
        if hover {
            let w = measure(tooltip, 15.).width + 24.;
            let area = screen_rect();
            let tip = Rect::new(
                (x - w / 2.).clamp(area.x + 8., area.right() - w - 8.),
                rect.y - 37.,
                w,
                28.,
            );
            panel(tip, INK);
            center(tooltip, tip, 15., TEXT);
        }
        enabled && hover && is_mouse_button_pressed(MouseButton::Left)
    }
    pub fn action_button(
        &self,
        icon: Icon,
        label: &str,
        tooltip: &str,
        rect: Rect,
        enabled: bool,
        active: bool,
    ) -> bool {
        let clicked = self.icon_button(icon, tooltip, rect, enabled, active);
        center(
            label,
            Rect::new(rect.x - 20., rect.bottom() + 7., rect.w + 40., 18.),
            13.,
            if enabled { TEXT } else { MUTED },
        );
        clicked
    }
    pub fn texture(&self, card: Option<Card>) -> &Texture2D {
        card.and_then(|c| self.cards.get(&c.label()))
            .map(|r| &r.texture)
            .unwrap_or(&self.back.texture)
    }
    fn card_3d(&self, card: Option<Card>, geometry: CardPose, eye: Vec3, highlight: Option<Color>) {
        let corners = geometry.corners();
        let uvs = [vec2(0., 1.), vec2(1., 1.), vec2(1., 0.), vec2(0., 0.)];
        // Two-sided appearance: a face is never painted on the underside.
        let face = if geometry.face_visible(eye) {
            card
        } else {
            None
        };
        draw_mesh(&Mesh {
            vertices: corners
                .iter()
                .zip(uvs)
                .map(|(p, uv)| Vertex::new2(*p, uv, WHITE))
                .collect(),
            indices: vec![0, 2, 1, 0, 3, 2],
            texture: Some(self.texture(face).clone()),
        });
        if let Some(color) = highlight {
            let normal = geometry.rotation * Vec3::Y;
            let border = CardPose {
                size: geometry.size * 1.5,
                position: geometry.position
                    + normal
                        * if geometry.face_visible(eye) {
                            0.006
                        } else {
                            -0.006
                        },
                ..geometry
            }
            .corners();
            self.glow_shader.set_uniform("Time", get_time() as f32);
            self.glow_shader.set_uniform("Strength", color.a * 1.6);
            gl_use_material(&self.glow_shader);
            draw_mesh(&Mesh {
                vertices: border
                    .iter()
                    .zip(uvs)
                    .map(|(p, uv)| Vertex::new2(*p, uv, WHITE))
                    .collect(),
                indices: vec![0, 2, 1, 0, 3, 2],
                texture: None,
            });
            gl_use_default_material();
        }
    }
    pub fn scene(
        &self,
        view: Option<&View>,
        rect: Rect,
        pose: TablePose,
        effects: Option<&crate::card_motion::Effects>,
        pointer: Option<Vec2>,
        held: Option<(Card, CardPose)>,
    ) -> Option<TableHit> {
        let hit = view.and_then(|v| pointer.and_then(|p| table_hit(v, pose, rect, p)));
        let eye = pose.position();
        let s = scale();
        let o = origin();
        let camera3d = Camera3D {
            position: pose.position(),
            target: pose.target,
            up: Vec3::Y,
            fovy: FIELD_OF_VIEW.to_radians(),
            aspect: Some(rect.w / rect.h),
            viewport: Some((
                ((o.x + rect.x * s) * screen_dpi_scale()) as i32,
                ((screen_height() - o.y - (rect.y + rect.h) * s) * screen_dpi_scale()) as i32,
                (rect.w * s * screen_dpi_scale()) as i32,
                (rect.h * s * screen_dpi_scale()) as i32,
            )),
            ..Default::default()
        };
        set_camera(&camera3d);
        // Table is actual perspective geometry, with a felt surface and layered rim.
        draw_cylinder_ex(
            vec3(0., -0.36, 0.),
            TABLE_RADIUS,
            TABLE_RADIUS - 0.1,
            0.26,
            None,
            Color::from_hex(0x152b30),
            DrawCylinderParams {
                sides: 80,
                ..Default::default()
            },
        );
        draw_cylinder_ex(
            vec3(0., -0.10, 0.),
            TABLE_RADIUS - 0.02,
            TABLE_RADIUS,
            0.06,
            None,
            Color::from_hex(0x9f8855),
            DrawCylinderParams {
                sides: 80,
                ..Default::default()
            },
        );
        draw_cylinder_ex(
            vec3(0., -0.04, 0.),
            TABLE_RADIUS - 0.14,
            TABLE_RADIUS - 0.14,
            0.04,
            None,
            Color::from_hex(0x204c47),
            DrawCylinderParams {
                sides: 80,
                ..Default::default()
            },
        );
        for r in [TABLE_RADIUS - 0.36, TABLE_RADIUS - 0.32] {
            ring(Vec3::new(0., 0.008, 0.), r, Color::from_hex(0x447168));
        }
        ring(Vec3::new(0., 0.009, 0.), 1.6, Color::from_hex(0x356158));
        let deck_count = view.map_or(70, |v| v.deck_count);
        let height = (deck_count as f32 * 0.002).min(0.18);
        if deck_count > 0 || view.is_some_and(|v| v.can_draw) {
            card_shadow(&[CardPose::flat(vec3(-0.85, height + 0.03, 0.), 0., 1.25)]);
            draw_cube(
                vec3(-0.85, height / 2. + 0.025, 0.),
                vec3(1.06, height, 1.58),
                None,
                Color::from_hex(0xd3d5c4),
            );
            self.card_3d(
                None,
                CardPose::flat(vec3(-0.85, height + 0.03, 0.), 0., 1.25),
                eye,
                if hit == Some(TableHit::Deck) {
                    Some(GOLD)
                } else if view.is_some_and(|v| v.can_draw && v.playable.is_empty()) {
                    let pulse = 0.70 + 0.15 * (get_time() as f32 * 3.).sin();
                    Some(Color::new(ACCENT.r, ACCENT.g, ACCENT.b, pulse))
                } else {
                    None
                },
            );
        }
        let top = view.map_or_else(
            || {
                game::deck()
                    .into_iter()
                    .find(|c| c.color == Some(Suit::Red) && c.face == game::Face::Number(7))
            },
            |v| v.top,
        );
        let top = effects.and_then(|e| e.discard).unwrap_or(top);
        card_shadow(&[
            CardPose::flat(vec3(0.85, 0.024, 0.), 0.1, 1.25),
            CardPose::flat(vec3(0.85, 0.045, 0.), -0.07, 1.25),
        ]);
        self.card_3d(
            None,
            CardPose::flat(vec3(0.85, 0.024, 0.), 0.1, 1.25),
            eye,
            None,
        );
        self.card_3d(
            top,
            CardPose::flat(vec3(0.85, 0.045, 0.), -0.07, 1.25),
            eye,
            None,
        );
        if let Some(v) = view {
            ring(vec3(0.85, 0.018, 0.), 1.02, suit(v.active_color));
        }
        let count = view.map_or(4, |v| v.players.len()).max(2);
        let you = view.map_or(0, |v| v.you);
        let seat = |id: usize| {
            let angle = seat_angle(id, you, count);
            (seat_position(id, you, count) + vec3(0., 0.02, 0.), angle)
        };
        let fading = held.is_some() || effects.is_some_and(|e| e.discard.is_some());
        let mut transparent = Vec::new();
        for id in 0..count {
            let (position, _) = seat(id);
            let cards = view.map_or(7, |v| {
                if id == v.you {
                    v.hand.len()
                } else {
                    v.players.get(id).map_or(0, |p| p.cards)
                }
            });
            if view.is_some_and(|v| v.turn == id) {
                ring(position + vec3(0., 0.004, 0.), 0.65, ACCENT);
                ring(
                    position + vec3(0., 0.006, 0.),
                    0.7,
                    Color::from_hex(0x659a70),
                );
            }
            let geometries: Vec<_> = (0..cards)
                .filter_map(|c| {
                    let card = view.and_then(|v| hand_face(v, id, c));
                    if held.is_some_and(|(held, _)| card == Some(held))
                        || effects.is_some_and(|e| e.hidden.contains(&(id, c)))
                    {
                        None
                    } else {
                        Some((card, hand_card(id, you, count, cards, c)))
                    }
                })
                .collect();
            if fading {
                let opacity = 0.24 / (cards as f32 / 8.).max(1.);
                transparent.extend(geometries.into_iter().map(|(c, p)| (c, p, opacity)));
            } else {
                card_shadow(&geometries.iter().map(|(_, pose)| *pose).collect::<Vec<_>>());
                for (card, geometry) in geometries {
                    // No legal-move hints: every card has the same appearance.
                    let hover = card.is_some_and(|c| hit == Some(TableHit::Card(c)));
                    self.card_3d(card, geometry, eye, hover.then_some(GOLD));
                }
            }
        }
        if let Some(effects) = effects {
            for (card, geometry) in &effects.moving {
                self.card_3d(*card, *geometry, eye, None);
            }
        }
        if let Some((card, geometry)) = held {
            self.card_3d(Some(card), geometry, eye, None);
        }
        // Opaque flights first; transparent hands last, back-to-front without
        // depth writes so the travelling card is visible even behind the hand.
        transparent.sort_by(|a, b| {
            b.1.position
                .distance_squared(eye)
                .total_cmp(&a.1.position.distance_squared(eye))
        });
        for (card, geometry, opacity) in transparent {
            self.ghost_shader.set_uniform("Opacity", opacity);
            gl_use_material(&self.ghost_shader);
            self.card_3d(card, geometry, eye, None);
        }
        gl_use_default_material();
        camera();
        hit
    }
}
const ICON_VERTEX: &str = "#version 100
attribute vec3 position;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;
varying lowp vec2 uv;
void main() { gl_Position = Projection * Model * vec4(position, 1.0); uv = texcoord; }
";
const ICON_FRAGMENT: &str = "#version 100
precision mediump float;
varying lowp vec2 uv;
uniform vec2 State;
uniform float Time;
void main() {
    vec2 p = abs(uv - 0.5) - vec2(0.22);
    float d = length(max(p, 0.0)) + min(max(p.x, p.y), 0.0) - 0.10;
    float body = 1.0 - smoothstep(-0.008, 0.008, d);
    float edge = exp(-abs(d) * 140.0);
    vec3 rainbow = 0.62 + 0.38 * cos(Time * 1.6 + uv.x * 5.0 - uv.y * 4.0 + vec3(0.0, 2.1, 4.2));
    float power = State.y < 0.0 ? 0.08 : 0.48 + State.x * 0.8 + State.y * 0.8;
    float pulse = 0.88 + 0.12 * sin(Time * 3.0 + uv.y * 8.0);
    float halo = exp(-max(d, 0.0) * 24.0) * power * pulse;
    vec3 tint = vec3(0.055, 0.12, 0.16) + rainbow * (edge * power * 1.8 + State.x * 0.12);
    float fade = 1.0 - smoothstep(0.44, 0.5, max(abs(uv.x - 0.5), abs(uv.y - 0.5)));
    gl_FragColor = vec4(mix(rainbow, tint, body), max(body * 0.97, halo * 0.8) * fade);
}
";
const GLOW_FRAGMENT: &str = "#version 100
precision mediump float;
varying lowp vec2 uv;
uniform float Time;
uniform float Strength;
void main() {
    // The quad is 1.5x the card. The SDF edge matches the original card.
    vec2 p = abs(uv - 0.5) * vec2(1.0, 1.5) - vec2(0.318, 0.485);
    float d = length(max(p, 0.0)) + min(max(p.x, p.y), 0.0) - 0.015;
    float rim = exp(-abs(d) * 155.0);
    float halo = exp(-abs(d) * 24.0);
    vec3 rainbow = 0.65 + 0.35 * cos(Time * 1.8 + uv.x * 6.0 + uv.y * 5.0 + vec3(0.0, 2.1, 4.2));
    float shimmer = 0.85 + 0.15 * sin(Time * 4.0 - uv.y * 12.0);
    float fade = 1.0 - smoothstep(0.44, 0.5, max(abs(uv.x - 0.5), abs(uv.y - 0.5)));
    gl_FragColor = vec4(rainbow + vec3(rim * 0.6), (rim + halo * 0.65) * Strength * shimmer * fade);
}
";
const GHOST_FRAGMENT: &str = "#version 100
precision mediump float;
varying lowp vec2 uv;
uniform sampler2D Texture;
uniform float Opacity;
void main() {
    vec4 card = texture2D(Texture, uv);
    gl_FragColor = vec4(card.rgb, card.a * Opacity);
}
";

/// Names project a single immutable world anchor per seat. No screen clamping,
/// hand bounds or collision reflow: camera motion cannot make labels jump.
pub fn player_names(view: &View, pose: TablePose, area: Rect) {
    for player in &view.players {
        let _ = measure(&player.name, 19.);
    }
    for (id, player) in view.players.iter().enumerate() {
        let angle = seat_angle(id, view.you, view.players.len());
        let anchor = vec3(
            angle.sin() * (TABLE_RADIUS + 0.5),
            0.35,
            angle.cos() * (TABLE_RADIUS + 0.5),
        );
        let Some(point) = pose.project(anchor, area) else {
            continue;
        };
        let name = trim(&player.name, 180., 19);
        let bounds = Rect::new(point.x - 100., point.y - 13., 200., 26.);
        // A fine glyph outline instead of an opaque nameplate.
        for (x, y) in [(-1.5, 0.), (1.5, 0.), (0., -1.5), (0., 1.5)] {
            center(
                &name,
                Rect::new(bounds.x + x, bounds.y + y, bounds.w, bounds.h),
                19.,
                INK,
            );
        }
        center(
            &name,
            bounds,
            19.,
            if !player.connected {
                ERROR
            } else if id == view.turn {
                ACCENT
            } else {
                TEXT
            },
        );
    }
}

/// Screen-space seats, in permanent seat order. Deliberately independent of the
/// camera, card geometry, turn, connection state and local hand ownership.
fn player_badge(area: Rect, seat: usize) -> Rect {
    Rect::new(area.x + 20., 154. + seat as f32 * 54., 240., 52.)
}

/// Share the exact visible bounds with input handling: never pick cards through
/// a player badge, even when roster interaction is temporarily disabled.
pub fn player_badge_at(area: Rect, players: usize, pointer: Vec2) -> Option<usize> {
    (0..players).find(|&seat| player_badge(area, seat).contains(pointer))
}

pub fn player_roster(view: &View, area: Rect, enabled: bool) -> Option<usize> {
    // Cache every name before drawing any glyphs (including non-ASCII names).
    // Atlas growth must not invalidate text already queued in this roster.
    for player in &view.players {
        let _ = measure(&player.name, 17.);
    }
    let hovered = player_badge_at(area, view.players.len(), mouse()).filter(|_| enabled);
    for (id, player) in view.players.iter().enumerate() {
        let r = player_badge(area, id);
        let current = id == view.turn;
        let hover = hovered == Some(id);
        let tint = if !player.connected {
            ERROR
        } else if current {
            ACCENT
        } else {
            MUTED
        };
        draw_rectangle(r.x, r.y + 2., r.w, r.h, Color::new(0., 0., 0., 0.16));
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            if hover {
                Color::from_hex(0x203a3d)
            } else if current {
                Color::from_hex(0x19372f)
            } else {
                PANEL
            },
        );
        if current || hover {
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1., if hover { GOLD } else { ACCENT });
        }
        draw_rectangle(r.x, r.y + 8., 3., r.h - 16., tint);
        center(
            &format!("{:02}", id + 1),
            Rect::new(r.x + 8., r.y + 6., 28., 22.),
            13.,
            tint,
        );
        if id == view.you {
            center("YOU", Rect::new(r.x + 6., r.y + 30., 32., 14.), 10., ACCENT);
        }
        text(
            &trim(&player.name, 140., 17),
            r.x + 44.,
            r.y + 21.,
            17.,
            TEXT,
        );
        let state = if !player.connected {
            "Offline"
        } else if current {
            if id == view.you {
                "Your turn"
            } else {
                "Turn"
            }
        } else {
            ""
        };
        let status = if state.is_empty() {
            format!("{} pts", player.score)
        } else {
            format!("{state} / {} pts", player.score)
        };
        text(&trim(&status, 140., 12), r.x + 44., r.y + 41., 12., tint);
        let count = Rect::new(r.right() - 48., r.y + 3., 42., 28.);
        center(
            &player.cards.to_string(),
            count,
            23.,
            if player.cards == 1 { GOLD } else { TEXT },
        );
        center(
            if player.cards == 1 { "UNO" } else { "cards" },
            Rect::new(count.x, r.y + 32., count.w, 14.),
            11.,
            if player.cards == 1 { GOLD } else { MUTED },
        );
    }
    // Full names stay discoverable without changing the row's width or position.
    if let Some(id) = hovered {
        let r = player_badge(area, id);
        let name = &view.players[id].name;
        let tip = Rect::new(
            r.right() + 10.,
            r.y,
            (measure(name, 17.).width + 24.).max(240.),
            58.,
        );
        panel(tip, INK);
        text(name, tip.x + 12., tip.y + 23., 17., TEXT);
        text(
            "Click to view this seat",
            tip.x + 12.,
            tip.y + 44.,
            12.,
            MUTED,
        );
    }
    hovered.filter(|_| is_mouse_button_pressed(MouseButton::Left))
}

fn card_shadow(cards: &[CardPose]) {
    let outline = crate::table_view::shadow_outline(cards);
    if outline.len() < 3 {
        return;
    }
    draw_mesh(&Mesh {
        vertices: outline
            .iter()
            .map(|p| {
                Vertex::new2(
                    vec3(p.x, 0.014, p.y),
                    Vec2::ZERO,
                    Color::new(0., 0., 0., 0.22),
                )
            })
            .collect(),
        indices: (1..outline.len() as u16 - 1)
            .flat_map(|i| [0, i + 1, i])
            .collect(),
        texture: None,
    });
}

fn ring(center: Vec3, radius: f32, color: Color) {
    for n in 0..80 {
        let a = n as f32 / 80. * std::f32::consts::TAU;
        let b = (n + 1) as f32 / 80. * std::f32::consts::TAU;
        draw_line_3d(
            center + vec3(a.sin() * radius, 0., a.cos() * radius),
            center + vec3(b.sin() * radius, 0., b.cos() * radius),
            color,
        );
    }
}
fn make_card(card: Option<Card>) -> RenderTarget {
    let rt = render_target(240, 360);
    rt.texture.set_filter(FilterMode::Linear);
    let mut cam = Camera2D::from_display_rect(Rect::new(0., 0., 240., 360.));
    cam.render_target = Some(rt.clone());
    set_camera(&cam);
    clear_background(Color::from_hex(0xf0eedf));
    let color = card
        .and_then(|c| c.color)
        .map(suit)
        .unwrap_or(Color::from_hex(0x18262f));
    draw_rectangle(9., 9., 222., 342., color);
    draw_rectangle_lines(16., 16., 208., 328., 2., Color::new(1., 1., 1., 0.25));
    if let Some(c) = card {
        if c.color.is_none() {
            for (i, color) in Suit::ALL.into_iter().enumerate() {
                draw_rectangle(23. + i as f32 * 49., 64., 48., 10., suit(color));
                draw_rectangle(23. + i as f32 * 49., 286., 48., 10., suit(color));
            }
        }
        draw_ellipse(120., 181., 96., 102., -0.2, Color::from_hex(0xf0eedf));
        let label = c.face.label();
        let size = match label.len() {
            1 => 112.,
            2 => 88.,
            _ => 46.,
        };
        center(&label, Rect::new(22., 112., 196., 126.), size, color);
        let corner = if label.len() > 2 { 25. } else { 36. };
        text(&label, 25., 48., corner, WHITE);
        let w = measure(&label, corner).width;
        text(&label, 215. - w, 330., corner, WHITE);
        if c.face == game::Face::TargetTwo {
            center("CHOOSE +2", Rect::new(24., 247., 192., 24.), 21., color);
        }
        // Color name is a redundant cue for color-vision accessibility.
        if let Some(color) = c.color {
            center(
                &color.name().to_uppercase(),
                Rect::new(25., 283., 190., 24.),
                17.,
                WHITE,
            );
        }
    } else {
        for y in (28..335).step_by(22) {
            draw_line(
                23.,
                y as f32,
                217.,
                y as f32 + 18.,
                1.,
                Color::from_hex(0x34504f),
            );
        }
        draw_ellipse(120., 180., 93., 85., -0.3, ACCENT);
        center("UNO", Rect::new(20., 124., 200., 75.), 64., INK);
        center("TABLE", Rect::new(20., 197., 200., 40.), 24., INK);
        for (i, color) in Suit::ALL.into_iter().enumerate() {
            draw_circle(72. + i as f32 * 32., 304., 8., suit(color));
        }
    }
    set_default_camera();
    rt
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use crate::table_view::TableCamera;

    fn view(players: usize, you: usize) -> View {
        let mut game = game::Game::new(23);
        for i in 0..players {
            game.add_player(format!("Player {i}")).unwrap();
        }
        game.act(0, game::Action::Start).unwrap();
        game.view(you)
    }

    #[test]
    fn only_owner_has_faces_and_camera_does_not_change_ownership() {
        for players in 2..=10 {
            for you in 0..players {
                let view = view(players, you);
                for seat in 0..players {
                    for index in 0..view.players[seat].cards {
                        assert_eq!(
                            hand_face(&view, seat, index),
                            if seat == you {
                                view.hand.get(index).copied()
                            } else {
                                None
                            }
                        );
                    }
                    let mut camera = TableCamera::default();
                    camera.focus_seat(seat, you, players);
                    for _ in 0..120 {
                        camera.update(0.016);
                    }
                    let card = hand_card(seat, you, players, view.players[seat].cards, 0);
                    let area = Rect::new(0., 0., 1280., 800.);
                    if let Some(point) = camera.pose.project(card.position, area) {
                        if let Some(TableHit::Card(card)) =
                            table_hit(&view, camera.pose, area, point)
                        {
                            assert!(view.hand.contains(&card));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn player_badges_fit_all_seats_without_overlapping_hud_or_each_other() {
        for area in [
            Rect::new(0., 0., 1280., 800.),
            Rect::new(-320., 0., 1920., 800.),
            Rect::new(0., -240., 1280., 1280.),
        ] {
            for players in 2..=10 {
                for id in 0..players {
                    let r = player_badge(area, id);
                    assert!(area.contains(vec2(r.x, r.y)));
                    assert!(area.contains(vec2(r.right(), r.bottom())));
                    assert!(r.y >= 112. && r.bottom() < 700.);
                    assert!(r.right() < 528.); // Bottom actions and right-side camera controls.
                    assert!((0..id).all(|other| !r.overlaps(&player_badge(area, other))));
                    let center = vec2(r.x + r.w / 2., r.y + r.h / 2.);
                    assert_eq!(player_badge_at(area, players, center), Some(id));
                }
                assert_eq!(player_badge_at(area, players, vec2(640., 400.)), None);
                let last = player_badge(area, players - 1);
                assert_eq!(
                    player_badge_at(area, players, vec2(last.x + 10., last.bottom() + 2.)),
                    None
                );
            }
        }
    }

    #[test]
    fn player_badges_stay_fixed_through_camera_transitions_and_game_updates() {
        let area = Rect::new(0., 0., 1280., 800.);
        let mut view = view(10, 0);
        let original: Vec<_> = (0..10).map(|id| player_badge(area, id)).collect();
        let mut camera = TableCamera::default();
        for preset in 0..5 {
            match preset {
                0 => camera.overview(),
                1 => camera.hand(),
                2 => camera.focus_seat(9, 0, 10),
                3 => {
                    camera.pan(vec2(1000., -1000.));
                    camera.zoom(1000.);
                }
                _ => camera.my_seat(),
            }
            for frame in 0..120 {
                camera.update(0.016);
                view.you = frame % 10;
                view.turn = (frame + 1) % 10;
                view.players[frame % 10].connected = frame % 2 == 0;
                view.players[frame % 10].cards = frame;
                for (id, expected) in original.iter().enumerate() {
                    let r = player_badge(area, id);
                    assert_eq!(r, *expected);
                    assert_eq!(
                        player_badge_at(area, view.players.len(), vec2(r.x + 1., r.y + 1.)),
                        Some(id)
                    );
                }
            }
        }
    }

    #[test]
    fn large_hands_have_no_unreachable_cards_or_pages() {
        let area = Rect::new(0., 0., 1280., 800.);
        let mut camera = TableCamera::default();
        camera.hand();
        for _ in 0..120 {
            camera.update(0.016);
        }
        for players in [2, 10] {
            for count in [7, 37, 111] {
                let mut view = view(players, 0);
                view.hand = game::deck().into_iter().take(count).collect();
                for player in &mut view.players {
                    player.cards = 0;
                }
                view.players[0].cards = count;
                for (index, card) in view.hand.iter().enumerate() {
                    let geometry = hand_card(0, 0, players, count, index);
                    let mut reachable = false;
                    'sample: for x in 0..24 {
                        for z in 0..12 {
                            let p = geometry.position
                                + geometry.rotation
                                    * (vec3(
                                        -0.415 + x as f32 * 0.83 / 23.,
                                        0.,
                                        -0.62 + z as f32 * 1.24 / 11.,
                                    ) * geometry.size);
                            if let Some(point) = camera.pose.project(p, area) {
                                if table_hit(&view, camera.pose, area, point)
                                    == Some(TableHit::Card(*card))
                                {
                                    reachable = true;
                                    break 'sample;
                                }
                            }
                        }
                    }
                    assert!(
                        reachable,
                        "unreachable card {index} of {count}, {players} seats"
                    );
                }
            }
        }
    }

    #[test]
    fn deck_remains_pickable_when_stock_can_be_recycled() {
        let mut view = view(4, 0);
        let pose = TablePose::default();
        let area = Rect::new(0., 0., 1280., 800.);
        let point = pose
            .project(
                vec3(-0.85, (view.deck_count as f32 * 0.002).min(0.18) + 0.03, 0.),
                area,
            )
            .unwrap();
        assert_eq!(table_hit(&view, pose, area, point), Some(TableHit::Deck));
        view.deck_count = 0;
        view.can_draw = false;
        assert_eq!(table_hit(&view, pose, area, point), None);
        view.can_draw = true;
        assert_eq!(table_hit(&view, pose, area, point), Some(TableHit::Deck));
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn replacing_selection_clears_it_only_once() {
        let mut focus = InputFocus {
            field: Some(Field::Invite),
            selected: true,
        };
        let mut value = "uno://localhost:34197/token".to_string();
        assert!(focus.delete_selection(&mut value));
        assert!(value.is_empty());
        assert!(!focus.selected);
        value.push('é');
        assert!(!focus.delete_selection(&mut value));
        assert_eq!(value, "é");
    }

    #[test]
    fn deleting_without_selection_preserves_text() {
        let mut focus = InputFocus::default();
        let mut value = "Player".to_string();
        assert!(!focus.delete_selection(&mut value));
        assert_eq!(value, "Player");
    }
}

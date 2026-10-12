//! Table geometry, camera and ray picking, independent of rendering/input.
use macroquad::prelude::*;

pub const FIELD_OF_VIEW: f32 = 48.;
pub const TABLE_RADIUS: f32 = 6.;
pub const HAND_RADIUS: f32 = 4.75;
const MIN_DISTANCE: f32 = 6.;
const MAX_DISTANCE: f32 = 28.;

#[derive(Clone, Copy, Debug)]
pub struct TablePose {
    pub orbit: f32,
    pub elevation: f32,
    pub distance: f32,
    pub target: Vec3,
}
impl Default for TablePose {
    fn default() -> Self {
        Self {
            orbit: 0.,
            elevation: 26.,
            distance: 12.5,
            target: vec3(0., 0., 0.7),
        }
    }
}
impl TablePose {
    pub fn position(self) -> Vec3 {
        let elevation = self.elevation.to_radians();
        self.target
            + vec3(
                self.orbit.sin() * elevation.cos(),
                elevation.sin(),
                self.orbit.cos() * elevation.cos(),
            ) * self.distance
    }
    pub fn matrix(self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh_gl(FIELD_OF_VIEW.to_radians(), aspect, 0.01, 100.)
            * Mat4::look_at_rh(self.position(), self.target, Vec3::Y)
    }
    pub fn ray(self, point: Vec2, area: Rect) -> (Vec3, Vec3) {
        let ndc = vec2(
            (point.x - area.x) / area.w * 2. - 1.,
            1. - (point.y - area.y) / area.h * 2.,
        );
        let inverse = self.matrix(area.w / area.h).inverse();
        let near = inverse.project_point3(ndc.extend(-1.));
        let far = inverse.project_point3(ndc.extend(1.));
        (near, (far - near).normalize())
    }
    pub fn project(self, point: Vec3, area: Rect) -> Option<Vec2> {
        let clip = self.matrix(area.w / area.h) * point.extend(1.);
        if clip.w <= 0. {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1. || ndc.y.abs() > 1. || ndc.z.abs() > 1. {
            return None;
        }
        Some(vec2(
            area.x + (ndc.x + 1.) * area.w / 2.,
            area.y + (1. - ndc.y) * area.h / 2.,
        ))
    }
}

#[derive(Default)]
pub struct TableCamera {
    pub pose: TablePose,
    target: TablePose,
}
impl TableCamera {
    pub fn my_seat(&mut self) {
        self.target = TablePose::default();
    }
    pub fn overview(&mut self) {
        self.target = TablePose {
            elevation: 78.,
            distance: 17.,
            target: Vec3::ZERO,
            ..Default::default()
        };
    }
    pub fn hand(&mut self) {
        self.target = TablePose {
            elevation: 48.,
            distance: 8.,
            target: vec3(0., 0., 3.),
            ..Default::default()
        };
    }
    pub fn focus_seat(&mut self, seat: usize, you: usize, count: usize) {
        let orbit = seat_angle(seat, you, count);
        self.target = TablePose {
            orbit,
            target: vec3(orbit.sin() * 0.7, 0., orbit.cos() * 0.7),
            ..Default::default()
        };
    }
    pub fn drag(&mut self, delta: Vec2) {
        self.target = self.pose;
        self.target.orbit -= delta.x * 0.003;
        self.target.elevation = (self.target.elevation + delta.y * 0.08).clamp(12., 85.);
        self.pose = self.target;
    }
    pub fn pan(&mut self, delta: Vec2) {
        self.target = self.pose;
        let right = vec3(self.pose.orbit.cos(), 0., -self.pose.orbit.sin());
        let forward = vec3(self.pose.orbit.sin(), 0., self.pose.orbit.cos());
        self.target.target += (right * delta.x + forward * delta.y) * self.pose.distance * 0.0015;
        self.target.target = self
            .target
            .target
            .clamp(vec3(-4., 0., -4.), vec3(4., 0., 4.));
        self.pose = self.target;
    }
    pub fn zoom(&mut self, wheel: f32) {
        self.target.distance =
            (self.target.distance - wheel * 0.325).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }
    pub fn update(&mut self, dt: f32) {
        let amount = 1. - (-10. * dt.clamp(0., 0.1)).exp();
        let delta = (self.target.orbit - self.pose.orbit + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.pose.orbit = (self.pose.orbit + delta * amount).rem_euclid(std::f32::consts::TAU);
        self.pose.elevation += (self.target.elevation - self.pose.elevation) * amount;
        self.pose.distance += (self.target.distance - self.pose.distance) * amount;
        self.pose.target = self.pose.target.lerp(self.target.target, amount);
    }
}

pub fn seat_angle(seat: usize, you: usize, count: usize) -> f32 {
    ((seat + count - you) % count) as f32 / count as f32 * std::f32::consts::TAU
}
pub fn seat_position(seat: usize, you: usize, count: usize) -> Vec3 {
    let angle = seat_angle(seat, you, count);
    vec3(angle.sin() * HAND_RADIUS, 0., angle.cos() * HAND_RADIUS)
}

#[derive(Clone, Copy, Debug)]
pub struct CardPose {
    pub position: Vec3,
    pub rotation: Quat,
    pub size: f32,
}
impl CardPose {
    pub fn flat(position: Vec3, angle: f32, size: f32) -> Self {
        Self {
            position,
            rotation: Quat::from_rotation_y(angle),
            size,
        }
    }
    pub fn interpolate(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0., 1.);
        Self {
            position: self.position.lerp(to.position, t),
            rotation: self.rotation.slerp(to.rotation, t),
            size: self.size + (to.size - self.size) * t,
        }
    }
    pub fn corners(self) -> [Vec3; 4] {
        [
            vec3(-0.42, 0., -0.63),
            vec3(0.42, 0., -0.63),
            vec3(0.42, 0., 0.63),
            vec3(-0.42, 0., 0.63),
        ]
        .map(|p| self.position + self.rotation * (p * self.size))
    }
    pub fn face_visible(self, eye: Vec3) -> bool {
        (eye - self.position).dot(self.rotation * Vec3::Y) > 0.
    }
    pub fn hit(self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let inverse = self.rotation.inverse();
        let origin = inverse * (origin - self.position);
        let direction = inverse * direction;
        if direction.y.abs() < 0.00001 {
            return None;
        }
        let distance = -origin.y / direction.y;
        let hit = origin + direction * distance;
        (distance > 0. && hit.x.abs() <= 0.42 * self.size && hit.z.abs() <= 0.63 * self.size)
            .then_some(distance)
    }
}

/// All cards are on the felt, in overlapping rows rather than screen-space pages.
/// Every client rotates its own seat to the near edge; ownership never follows orbit.
pub fn hand_card(seat: usize, you: usize, players: usize, cards: usize, index: usize) -> CardPose {
    let angle = seat_angle(seat, you, players);
    let columns = cards.clamp(1, 14);
    let row = index / columns;
    let column = index % columns;
    let row_count = (cards - row * columns).min(columns);
    let size = if players > 6 { 0.86 } else { 1.05 };
    let width =
        (2. * HAND_RADIUS * (std::f32::consts::PI / players.max(2) as f32).sin() * 0.82).min(5.4);
    let step = ((width - 0.84 * size) / (columns - 1).max(1) as f32).min(0.78 * size);
    let x = (column as f32 - (row_count - 1) as f32 / 2.) * step;
    let offset = vec3(
        x,
        0.48 + column as f32 * 0.012,
        HAND_RADIUS - row as f32 * 0.40,
    );
    CardPose {
        position: Quat::from_rotation_y(angle) * offset,
        rotation: Quat::from_rotation_y(angle) * Quat::from_rotation_x(32_f32.to_radians()),
        size,
    }
}

/// Intersect the cursor ray with a horizontal dragging plane.
pub fn pointer_on_plane(pose: TablePose, area: Rect, pointer: Vec2, height: f32) -> Option<Vec3> {
    let (origin, direction) = pose.ray(pointer, area);
    if direction.y.abs() < 0.00001 {
        return None;
    }
    let distance = (height - origin.y) / direction.y;
    (distance > 0.).then(|| origin + direction * distance)
}

pub fn over_discard(pose: TablePose, area: Rect, pointer: Vec2) -> bool {
    let (origin, direction) = pose.ray(pointer, area);
    CardPose::flat(vec3(0.85, 0.045, 0.), -0.07, 1.5)
        .hit(origin, direction)
        .is_some()
}

/// One convex silhouette per card group: triangles share edges, never opacity.
pub fn shadow_outline(cards: &[CardPose]) -> Vec<Vec2> {
    let mut points: Vec<_> = cards
        .iter()
        .flat_map(|c| c.corners())
        .map(|p| vec2(p.x + 0.045, p.z + 0.06))
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let mut hull: Vec<Vec2> = Vec::new();
    for point in &points {
        while hull.len() >= 2
            && (hull[hull.len() - 1] - hull[hull.len() - 2]).perp_dot(*point - hull[hull.len() - 1])
                <= 0.
        {
            hull.pop();
        }
        hull.push(*point);
    }
    let lower = hull.len();
    for point in points.iter().rev().skip(1) {
        while hull.len() > lower
            && (hull[hull.len() - 1] - hull[hull.len() - 2]).perp_dot(*point - hull[hull.len() - 1])
                <= 0.
        {
            hull.pop();
        }
        hull.push(*point);
    }
    hull.pop();
    hull
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle(camera: &mut TableCamera) {
        for _ in 0..180 {
            camera.update(0.016);
        }
    }

    #[test]
    fn defaults_to_own_seat_and_presets_reset_pan() {
        let mut camera = TableCamera::default();
        assert_eq!(camera.pose.orbit, 0.);
        assert!(camera.pose.elevation < 30.);
        camera.focus_seat(9, 0, 10);
        camera.update(0.016);
        assert!(camera.pose.orbit > std::f32::consts::PI);
        settle(&mut camera);
        assert!((camera.pose.orbit - seat_angle(9, 0, 10)).abs() < 0.001);
        camera.pan(vec2(500., 500.));
        camera.my_seat();
        settle(&mut camera);
        assert!(camera.pose.target.distance(TablePose::default().target) < 0.001);
        assert!(camera.pose.orbit.sin().abs() < 0.001);
    }
    #[test]
    fn camera_has_wide_bounded_motion() {
        let mut camera = TableCamera::default();
        camera.zoom(1000.);
        assert_eq!(camera.target.distance, MIN_DISTANCE);
        camera.zoom(-1000.);
        assert_eq!(camera.target.distance, MAX_DISTANCE);
        camera.drag(vec2(0., 1000.));
        assert_eq!(camera.pose.elevation, 85.);
        camera.drag(vec2(0., -1000.));
        assert_eq!(camera.pose.elevation, 12.);
        camera.pan(vec2(100000., -100000.));
        assert_eq!(camera.pose.target, vec3(4., 0., -4.));
    }
    #[test]
    fn overview_fits_table() {
        let mut camera = TableCamera::default();
        camera.overview();
        settle(&mut camera);
        let area = Rect::new(0., 0., 1280., 800.);
        for degree in 0..360 {
            let a = (degree as f32).to_radians();
            assert!(camera
                .pose
                .project(
                    vec3(a.sin() * TABLE_RADIUS, 0., a.cos() * TABLE_RADIUS),
                    area
                )
                .is_some());
        }
    }
    #[test]
    fn hands_stay_on_table_and_local_faces_are_visible() {
        for players in 2..=10 {
            for you in 0..players {
                assert_eq!(seat_angle(you, you, players), 0.);
                for count in [1, 7, 14, 37, 111] {
                    for i in 0..count {
                        let card = hand_card(you, you, players, count, i);
                        assert!(card.face_visible(TablePose::default().position()));
                        for p in card.corners() {
                            assert!(p.y > 0.);
                            assert!(vec2(p.x, p.z).length() < TABLE_RADIUS);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn projected_card_can_be_picked_from_any_camera_preset() {
        let area = Rect::new(-100., 0., 1480., 800.);
        let mut camera = TableCamera::default();
        for preset in 0..3 {
            match preset {
                1 => camera.overview(),
                2 => camera.hand(),
                _ => camera.my_seat(),
            }
            settle(&mut camera);
            for i in 0..7 {
                let card = hand_card(2, 2, 4, 7, i);
                let pixel = camera.pose.project(card.position, area).unwrap();
                let (origin, direction) = camera.pose.ray(pixel, area);
                let distance = card.hit(origin, direction).unwrap();
                assert!((origin + direction * distance).distance(card.position) < 0.01);
                assert!(card.hit(card.position + Vec3::Y, Vec3::Y).is_none());
            }
        }
    }
    #[test]
    fn drag_direction_and_sensitivity_are_predictable() {
        let mut camera = TableCamera::default();
        let elevation = camera.pose.elevation;
        camera.drag(vec2(100., 100.));
        assert!((camera.pose.orbit + 0.3).abs() < 0.001);
        assert!((camera.pose.elevation - (elevation + 8.)).abs() < 0.001);
        camera.drag(vec2(-100., -100.));
        assert!((camera.pose.elevation - elevation).abs() < 0.001);
        assert!(camera.pose.orbit.abs() < 0.001);
    }
    #[test]
    fn vertical_pan_is_inverted_at_every_seat() {
        for seat in 0..4 {
            let mut camera = TableCamera::default();
            camera.focus_seat(seat, 0, 4);
            settle(&mut camera);
            let before = camera.pose.target;
            let forward = vec3(camera.pose.orbit.sin(), 0., camera.pose.orbit.cos());
            camera.pan(vec2(0., -50.));
            assert!((camera.pose.target - before).dot(forward) < 0.);
            camera.pan(vec2(0., 50.));
            assert!(camera.pose.target.distance(before) < 0.001);
        }
    }
    #[test]
    fn horizontal_pan_is_inverted_and_zoom_is_half_speed() {
        for seat in 0..4 {
            let mut camera = TableCamera::default();
            camera.focus_seat(seat, 0, 4);
            settle(&mut camera);
            let before = camera.pose.target;
            let right = vec3(camera.pose.orbit.cos(), 0., -camera.pose.orbit.sin());
            camera.pan(vec2(50., 0.));
            assert!((camera.pose.target - before).dot(right) > 0.);
            camera.pan(vec2(-50., 0.));
            assert!(camera.pose.target.distance(before) < 0.001);
        }
        let mut camera = TableCamera::default();
        let before = camera.target.distance;
        camera.zoom(1.);
        assert!((before - camera.target.distance - 0.325).abs() < 0.0001);
    }
    #[test]
    fn card_interpolation_restores_position_size_and_tilt() {
        let home = hand_card(0, 0, 4, 7, 2);
        let flat = CardPose::flat(vec3(0.85, 0.09, 0.), 0., 1.25);
        let returned = flat.interpolate(home, 1.);
        assert!(returned.position.distance(home.position) < 0.001);
        assert!((returned.rotation * Vec3::Y).distance(home.rotation * Vec3::Y) < 0.001);
        assert!((returned.size - home.size).abs() < 0.001);
    }
    #[test]
    fn dragging_and_drop_target_follow_camera_projection() {
        let area = Rect::new(-100., 0., 1480., 800.);
        let mut camera = TableCamera::default();
        for preset in 0..4 {
            match preset {
                1 => camera.overview(),
                2 => camera.hand(),
                3 => camera.focus_seat(2, 0, 4),
                _ => camera.my_seat(),
            }
            settle(&mut camera);
            let center = vec3(0.85, 0.045, 0.);
            let pixel = camera.pose.project(center, area).unwrap();
            assert!(over_discard(camera.pose, area, pixel));
            let dragged = pointer_on_plane(camera.pose, area, pixel, 0.5).unwrap();
            assert!(camera.pose.project(dragged, area).unwrap().distance(pixel) < 0.1);
            let deck = camera.pose.project(vec3(-0.85, 0.2, 0.), area).unwrap();
            assert!(!over_discard(camera.pose, area, deck));
        }
    }
    #[test]
    fn group_shadow_is_one_convex_adaptive_outline() {
        assert!(shadow_outline(&[]).is_empty());
        for count in [1, 7, 37, 111] {
            let cards: Vec<_> = (0..count).map(|i| hand_card(0, 0, 4, count, i)).collect();
            let hull = shadow_outline(&cards);
            assert!(hull.len() >= 4);
            for i in 0..hull.len() {
                let a = hull[i];
                let b = hull[(i + 1) % hull.len()];
                let c = hull[(i + 2) % hull.len()];
                assert!((b - a).perp_dot(c - b) > 0.);
                for point in cards.iter().flat_map(|p| p.corners()) {
                    let point = vec2(point.x + 0.045, point.z + 0.06);
                    assert!((b - a).perp_dot(point - a) >= -0.00001);
                }
            }
            // Repeated overlapping cards must never multiply shadow opacity/geometry.
            assert_eq!(shadow_outline(&[cards[0]; 10]), shadow_outline(&cards[..1]));
            if count > 1 {
                assert_ne!(hull, shadow_outline(&cards[..count - 1]));
            }
        }
    }
    #[test]
    fn backside_is_detected() {
        let card = hand_card(0, 0, 4, 7, 0);
        let normal = card.rotation * Vec3::Y;
        assert!(card.face_visible(card.position + normal));
        assert!(!card.face_visible(card.position - normal));
    }
}

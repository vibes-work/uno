//! Snapshot-driven presentation. Cards enter their static piles only after arrival.
use crate::table_view::{hand_card, CardPose};
use macroquad::prelude::*;
use uno::game::{Card, View};

const FLIGHT_TIME: f64 = 0.48;

#[derive(Default)]
pub struct Effects {
    pub discard: Option<Option<Card>>,
    pub moving: Vec<(Option<Card>, CardPose)>,
    pub hidden: Vec<(usize, usize)>,
}
struct DrawFlight {
    seat: usize,
    index: usize,
    card: Option<Card>,
    start: f64,
    from: CardPose,
}
struct PlayFlight {
    card: Card,
    previous: Option<Card>,
    from: CardPose,
    start: f64,
}
#[derive(Default)]
pub struct CardMotion {
    draws: Vec<DrawFlight>,
    plays: Vec<PlayFlight>,
}
impl CardMotion {
    pub fn observe(&mut self, old: &View, new: &View, held: Option<CardPose>, now: f64) {
        if old.round != new.round || old.players.len() != new.players.len() {
            *self = Self::default();
            return;
        }
        if old.you != new.you {
            // A hot-seat switch rotates the table and changes which faces are private.
            *self = Self::default();
        }
        if new.top != old.top {
            if let Some(card) = new.top {
                let actor = old.turn;
                let index = old.hand.iter().position(|c| *c == card).unwrap_or(0);
                let from = if old.you == new.you { held } else { None }.unwrap_or_else(|| {
                    hand_card(
                        actor,
                        new.you,
                        new.players.len(),
                        old.players[actor].cards,
                        index,
                    )
                });
                let start = self
                    .plays
                    .last()
                    .map_or(now, |p| (p.start + FLIGHT_TIME).max(now));
                self.plays.push(PlayFlight {
                    card,
                    previous: old.top,
                    from,
                    start,
                });
            }
        }
        for (seat, player) in new.players.iter().enumerate() {
            let before = old.players[seat].cards;
            let indices: Vec<_> = if seat == new.you && old.you == new.you {
                new.hand
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| !old.hand.contains(c))
                    .map(|(i, _)| i)
                    .collect()
            } else {
                (before..player.cards).collect()
            };
            for (order, index) in indices.into_iter().enumerate() {
                self.draws.push(DrawFlight {
                    seat,
                    index,
                    card: if seat == new.you {
                        new.hand.get(index).copied()
                    } else {
                        None
                    },
                    start: now + order as f64 * 0.065,
                    from: CardPose::flat(
                        vec3(-0.85, (old.deck_count as f32 * 0.002).min(0.18) + 0.03, 0.),
                        0.,
                        1.25,
                    ),
                });
            }
        }
    }
    pub fn effects(&mut self, view: &View, now: f64) -> Effects {
        self.draws.retain_mut(|f| {
            if let Some(card) = f.card {
                let Some(index) = view.hand.iter().position(|c| *c == card) else {
                    return false;
                };
                f.index = index;
            }
            now < f.start + FLIGHT_TIME && f.index < view.players[f.seat].cards
        });
        self.plays.retain(|f| now < f.start + FLIGHT_TIME);
        let mut effects = Effects {
            discard: self.plays.first().map(|f| f.previous),
            ..Default::default()
        };
        for f in &self.draws {
            effects.hidden.push((f.seat, f.index));
            if now < f.start {
                continue;
            }
            let t = ((now - f.start) / FLIGHT_TIME) as f32;
            let to = hand_card(
                f.seat,
                view.you,
                view.players.len(),
                view.players[f.seat].cards,
                f.index,
            );
            let mut pose = f.from.interpolate(to, smooth(t));
            pose.position.y += (t * std::f32::consts::PI).sin() * 0.65;
            let face = if f.seat == view.you {
                view.hand.get(f.index).copied()
            } else {
                None
            };
            effects.moving.push((face, pose));
        }
        if let Some(f) = self.plays.first().filter(|f| now >= f.start) {
            let t = ((now - f.start) / FLIGHT_TIME) as f32;
            let to = CardPose::flat(vec3(0.85, 0.065, 0.), -0.07, 1.25);
            effects
                .moving
                .push((Some(f.card), f.from.interpolate(to, smooth(t))));
        }
        effects
    }
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uno::game::{Action, Color, Game, Phase};
    fn view() -> View {
        let mut g = Game::new(23);
        g.add_player("A".into()).unwrap();
        g.add_player("B".into()).unwrap();
        g.act(0, Action::Start).unwrap();
        let turn = g.view(0).turn;
        if g.phase == Phase::OpeningColor {
            g.act(turn, Action::ChooseOpeningColor { color: Color::Red })
                .unwrap();
        }
        g.view(turn)
    }
    #[test]
    fn drawn_card_is_hidden_until_arrival_and_other_faces_stay_private() {
        let old = view();
        let mut new = old.clone();
        new.hand.push(new.top.unwrap());
        new.players[new.you].cards += 1;
        let other = 1 - new.you;
        new.players[other].cards += 1;
        let mut motion = CardMotion::default();
        motion.observe(&old, &new, None, 10.);
        let effects = motion.effects(&new, 10.2);
        assert_eq!(effects.hidden.len(), 2);
        assert_eq!(
            effects.moving.iter().filter(|(c, _)| c.is_some()).count(),
            1
        );
        assert!(motion.effects(&new, 11.).hidden.is_empty());
    }
    #[test]
    fn moving_draw_tracks_identity_when_another_card_leaves_hand() {
        let old = view();
        let mut new = old.clone();
        new.hand.push(new.top.unwrap());
        new.players[new.you].cards += 1;
        let mut motion = CardMotion::default();
        motion.observe(&old, &new, None, 10.);
        new.hand.remove(0);
        new.players[new.you].cards -= 1;
        let effects = motion.effects(&new, 10.2);
        assert_eq!(effects.hidden, vec![(new.you, new.hand.len() - 1)]);
        assert_eq!(effects.moving[0].0, new.top);
        new.round += 1;
        motion.observe(&old, &new, None, 10.3);
        assert!(motion.effects(&new, 10.4).moving.is_empty());
    }
    #[test]
    fn hotseat_switch_clears_private_faces_in_flight() {
        let old = view();
        let mut new = old.clone();
        new.hand.push(new.top.unwrap());
        new.players[new.you].cards += 1;
        let mut motion = CardMotion::default();
        motion.observe(&old, &new, None, 10.);
        let mut switched = new.clone();
        switched.you = 1 - new.you;
        switched.hand.clear();
        motion.observe(&new, &switched, None, 10.1);
        assert!(motion.effects(&switched, 10.2).moving.is_empty());
    }
    #[test]
    fn previous_discard_remains_until_play_lands_even_with_queued_updates() {
        let old = view();
        let mut new = old.clone();
        new.top = Some(old.hand[0]);
        let mut motion = CardMotion::default();
        motion.observe(&old, &new, None, 10.);
        let mut next = new.clone();
        next.top = Some(old.hand[1]);
        motion.observe(&new, &next, None, 10.1);
        assert_eq!(motion.effects(&next, 10.2).discard, Some(old.top));
        assert_eq!(motion.effects(&next, 10.6).discard, Some(new.top));
        assert_eq!(motion.effects(&next, 11.).discard, None);
    }
}

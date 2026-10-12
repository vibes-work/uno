//! Offline table: one person controls every seat using the normal game rules.
use uno::game::{Action, Game, Phase, View, MAX_PLAYERS};

pub struct SelfPlay {
    game: Game,
    seat: usize,
    changed: bool,
}

impl SelfPlay {
    pub fn new(name: &str, players: usize) -> Result<Self, String> {
        if !(2..=MAX_PLAYERS).contains(&players) {
            return Err(format!("Choose 2-{MAX_PLAYERS} seats."));
        }
        let mut game = Game::default();
        game.add_player(name.into())?;
        for seat in 1..players {
            game.add_player(format!("Player {}", seat + 1))?;
        }
        game.act(game.host, Action::Start)?;
        let seat = game.view(game.host).turn;
        Ok(Self {
            game,
            seat,
            changed: true,
        })
    }

    pub fn act(&mut self, action: Action) -> Result<(), String> {
        // The local user owns every seat, including the host's round controls.
        let actor = if matches!(action, Action::NextRound) {
            self.game.host
        } else {
            self.seat
        };
        self.game.act(actor, action)?;
        self.follow_turn();
        Ok(())
    }

    pub fn next_hand(&mut self) {
        self.seat = (self.seat + 1) % self.game.players.len();
        self.changed = true;
    }

    pub fn follow_turn(&mut self) {
        self.seat = match self.game.phase {
            Phase::Lobby | Phase::RoundOver { .. } | Phase::MatchOver { .. } => self.game.host,
            _ => self.game.view(self.seat).turn,
        };
        self.changed = true;
    }

    /// Apply UI changes on the next frame, just like incoming network snapshots.
    pub fn take_view(&mut self) -> Option<View> {
        if !std::mem::take(&mut self.changed) {
            return None;
        }
        Some(self.game.view(self.seat))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uno::game::{Card, Color, Face};

    fn table(players: usize) -> SelfPlay {
        let mut game = Game::new(23);
        for i in 0..players {
            game.add_player(format!("P{i}")).unwrap();
        }
        game.act(0, Action::Start).unwrap();
        let mut table = SelfPlay {
            game,
            seat: 0,
            changed: false,
        };
        table.follow_turn();
        if table.game.phase == Phase::OpeningColor {
            table
                .act(Action::ChooseOpeningColor { color: Color::Red })
                .unwrap();
        }
        table
    }

    fn play(table: &mut SelfPlay, face: Face, uno: bool) {
        let color = table.game.view(table.seat).active_color;
        table.game.players[table.seat].hand = vec![
            Card {
                id: 200,
                color: (!face.wild()).then_some(color),
                face,
            },
            Card {
                id: 201,
                color: Some(color),
                face: Face::Number(3),
            },
        ];
        table
            .act(Action::Play {
                card: 200,
                color: Some(Color::Blue),
                target: Some((table.seat + 1) % table.game.players.len()),
                uno,
            })
            .unwrap();
    }

    #[test]
    fn setup_validates_names_and_seat_count() {
        for count in [0, 1, MAX_PLAYERS + 1] {
            assert!(SelfPlay::new("Tester", count).is_err());
        }
        assert!(SelfPlay::new("", 2).is_err());
        for count in 2..=MAX_PLAYERS {
            let mut table = SelfPlay::new("Tester", count).unwrap();
            let view = table.take_view().unwrap();
            assert_eq!(view.players.len(), count);
            assert_eq!(view.you, view.turn);
            assert!(!view.paused);
            assert!(table.take_view().is_none());
        }
    }

    #[test]
    fn manual_hand_switch_does_not_change_turn_or_bypass_rules() {
        let mut table = table(3);
        let before = table.take_view().unwrap();
        table.next_hand();
        let other = table.take_view().unwrap();
        assert_ne!(other.you, before.you);
        assert_eq!(other.turn, before.turn);
        assert_eq!(other.hand, table.game.players[other.you].hand);
        assert!(!other.can_draw);
        assert!(table.act(Action::Draw).is_err());
        assert!(table.take_view().is_none());
        table.follow_turn();
        assert_eq!(table.take_view().unwrap().you, before.turn);
    }

    #[test]
    fn follows_skip_reverse_and_targeted_penalty() {
        for count in [2, 3, 10] {
            for face in [
                Face::Number(5),
                Face::Skip,
                Face::Reverse,
                Face::DrawTwo,
                Face::TargetTwo,
            ] {
                let mut table = table(count);
                let actor = table.seat;
                play(&mut table, face, true);
                let view = table.take_view().unwrap();
                assert_eq!(view.you, view.turn);
                if count == 2 {
                    assert_eq!(view.you, if face == Face::Skip { actor } else { 1 - actor });
                }
                assert_eq!(view.hand, table.game.players[view.turn].hand);
            }
        }
    }

    #[test]
    fn single_draw_keeps_the_hand_and_cannot_be_passed() {
        let mut table = table(3);
        if table.game.view(table.seat).pending_two > 0 {
            table.act(Action::Draw).unwrap();
        }
        let before = table.take_view().unwrap();
        table.act(Action::Draw).unwrap();
        let drawn = table.take_view().unwrap();
        assert_eq!(drawn.you, before.you);
        assert_eq!(drawn.turn_id, before.turn_id);
        assert_eq!(drawn.hand.len(), before.hand.len() + 1);
        assert!(!drawn.can_pass);
        assert!(table.act(Action::Pass).is_err());
        table.next_hand();
        table.follow_turn();
        let restored = table.take_view().unwrap();
        assert_eq!(restored.hand, drawn.hand);
        assert_eq!(restored.playable, drawn.playable);
    }

    #[test]
    fn wrong_card_does_not_switch_hotseat_or_mutate_state() {
        let mut table = table(2);
        if table.game.view(table.seat).pending_two > 0 {
            table.act(Action::Draw).unwrap();
        }
        let view = table.game.view(table.seat);
        let wrong = uno::game::deck()
            .into_iter()
            .find(|c| {
                !c.face.wild()
                    && c.color != Some(view.active_color)
                    && Some(c.face) != view.top.map(|c| c.face)
            })
            .unwrap();
        table.game.players[table.seat].hand = vec![wrong];
        let before = table.game.view(table.seat);
        assert!(table
            .act(Action::Play {
                card: wrong.id,
                color: None,
                target: None,
                uno: false
            })
            .is_err());
        let after = table.game.view(table.seat);
        assert_eq!(after.you, before.you);
        assert_eq!(after.turn_id, before.turn_id);
        assert_eq!(after.hand, before.hand);
        assert_eq!(after.top, before.top);
    }

    #[test]
    fn four_decision_is_controlled_by_victim() {
        for action in [Action::AcceptFour, Action::ChallengeFour] {
            let mut table = table(3);
            let actor = table.seat;
            play(&mut table, Face::WildFour, true);
            let view = table.take_view().unwrap();
            let victim = view.pending_four.unwrap().victim;
            assert_eq!(view.you, victim);
            table.act(action).unwrap();
            let view = table.take_view().unwrap();
            assert_eq!(view.you, view.turn);
            assert!(view.pending_four.is_none());
            assert_ne!(view.you, actor);
        }
    }

    #[test]
    fn can_catch_or_switch_back_to_declare_uno() {
        for declare in [false, true] {
            let mut table = table(3);
            let actor = table.seat;
            play(&mut table, Face::Number(5), false);
            assert_eq!(table.take_view().unwrap().vulnerable, Some(actor));
            if declare {
                while table.seat != actor {
                    table.next_hand();
                }
                table.act(Action::DeclareUno).unwrap();
            } else {
                table.act(Action::CatchUno { player: actor }).unwrap();
            }
            let view = table.take_view().unwrap();
            assert!(view.vulnerable.is_none());
            assert_eq!(view.you, view.turn);
            assert_eq!(
                table.game.players[actor].hand.len(),
                if declare { 1 } else { 3 }
            );
        }
    }

    #[test]
    fn next_round_and_rematch_work_from_any_hand() {
        for phase in [
            Phase::RoundOver {
                winner: 1,
                points: 10,
            },
            Phase::MatchOver { winner: 1 },
        ] {
            let mut table = table(3);
            table.game.phase = phase.clone();
            table.game.players[1].score = 500;
            table.follow_turn();
            assert_eq!(table.seat, table.game.host);
            table.next_hand();
            table.act(Action::NextRound).unwrap();
            let view = table.take_view().unwrap();
            assert_eq!(view.you, view.turn);
            assert!(matches!(view.phase, Phase::Playing | Phase::OpeningColor));
            assert_eq!(
                view.players[1].score,
                if matches!(phase, Phase::MatchOver { .. }) {
                    0
                } else {
                    500
                }
            );
        }
    }

    #[test]
    fn complete_rounds_using_only_local_controls() {
        for count in [2, 3, 10] {
            let mut table = table(count);
            let mut finished = false;
            for _ in 0..4000 {
                let view = table.take_view().unwrap();
                assert_eq!(view.hand, table.game.players[view.you].hand);
                let action = match view.phase {
                    Phase::OpeningColor => Action::ChooseOpeningColor { color: Color::Red },
                    Phase::Playing if view.pending_four.is_some() => Action::ChallengeFour,
                    Phase::Playing => {
                        if let Some(card) = view.playable.first() {
                            Action::Play {
                                card: *card,
                                color: Some(Color::Red),
                                target: Some((view.you + 1) % count),
                                uno: true,
                            }
                        } else if view.can_pass {
                            Action::Pass
                        } else {
                            Action::Draw
                        }
                    }
                    Phase::RoundOver { .. } | Phase::MatchOver { .. } => {
                        assert_eq!(view.you, view.host);
                        table.act(Action::NextRound).unwrap();
                        finished = true;
                        break;
                    }
                    Phase::Lobby => panic!("Local game should already be started"),
                };
                table.act(action).unwrap();
            }
            assert!(finished, "{count}-seat round did not finish");
        }
    }
}

//! Opt-in README capture: scripted legal moves, not a playable bot mode.
use super::*;
use uno::game::Game;

const FPS: usize = 15;
const FRAMES: usize = 180;

fn demo_game() -> Game {
    let mut game = Game::new(23);
    for name in ["You", "Alex", "Morgan", "Jamie"] {
        game.add_player(name.into()).unwrap();
    }
    game.act(0, Action::Start).unwrap();
    if game.phase == Phase::OpeningColor {
        let turn = game.view(0).turn;
        game.act(turn, Action::ChooseOpeningColor { color: Suit::Red })
            .unwrap();
    }
    game
}

fn advance(game: &mut Game) {
    let turn = game.view(0).turn;
    let view = game.view(turn);
    let action = if matches!(view.phase, Phase::RoundOver { .. }) {
        Action::NextRound
    } else if view.phase == Phase::OpeningColor {
        Action::ChooseOpeningColor { color: Suit::Blue }
    } else if view.pending_four.is_some() {
        Action::AcceptFour
    } else if let Some(card) = view
        .hand
        .iter()
        .filter(|card| view.playable.contains(&card.id))
        .min_by_key(|card| card.face.wild())
    {
        Action::Play {
            card: card.id,
            color: card.face.wild().then_some(Suit::Blue),
            target: (card.face == Face::TargetTwo).then_some((turn + 1) % view.players.len()),
            uno: view.hand.len() == 2,
        }
    } else {
        Action::Draw
    };
    game.act(turn, action).expect("Legal capture action");
}

pub(super) async fn render(app: &mut App, directory: &str) {
    std::fs::create_dir_all(directory).expect("Create capture directory");
    let mut game = demo_game();
    let mut previous = game.view(0);
    let mut view = previous.clone();
    let mut action_frame = 0;
    // Render a fixed timeline, independent of GPU speed and PNG encoding time.
    for frame in 0..FRAMES {
        if (15..150).contains(&frame) && (frame - 15) % 20 == 0 {
            previous = view.clone();
            advance(&mut game);
            view = game.view(0);
            action_frame = frame;
        }
        app.motion = card_motion::CardMotion::default();
        app.motion.observe(
            &previous,
            &view,
            None,
            get_time() - (frame - action_frame) as f64 / FPS as f64,
        );
        app.view = Some(view.clone());
        app.camera = table_view::TableCamera::default();
        // A gentle orbit at the end shows the table's depth without exposing hands.
        let t = ((frame as f32 - 135.) / 44.).clamp(0., 1.);
        let sweep = (t * std::f32::consts::PI).sin();
        app.camera.drag(vec2(-100. * sweep, 110. * sweep));
        clear_background(INK);
        ui::camera();
        app.table(&view);
        set_default_camera();
        get_screen_data().export_png(&format!("{directory}/frame-{frame:03}.png"));
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_sequence_uses_legal_moves_and_keeps_a_private_view() {
        let mut game = demo_game();
        let initial = game.view(0);
        for _ in 0..7 {
            advance(&mut game);
            let view = game.view(0);
            assert_eq!(view.you, 0);
            assert_eq!(view.hand.len(), view.players[0].cards);
            assert_eq!(view.phase, Phase::Playing);
        }
        let final_view = game.view(0);
        assert_ne!(initial.top, final_view.top);
        assert_ne!(initial.turn_id, final_view.turn_id);
    }
}

use crate::prelude::*;

pub struct JumpySessionPlugin;

/// Stage label for the game session stages
#[derive(StageLabel)]
pub enum SessionStage {
    /// Update the game session.
    Update,
}

impl Plugin for JumpySessionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(bones_bevy_renderer::BonesRendererPlugin::<Session>::with_sync_time(false))
            .add_plugin(jumpy_core::metadata::JumpyCoreAssetsPlugin)
            .init_resource::<CurrentEditorInput>()
            .add_stage_before(
                CoreStage::Update,
                SessionStage::Update,
                SystemStage::single_threaded()
                    .with_system(
                        ensure_2_players
                            .run_in_state(EngineState::InGame)
                            .run_in_state(InGameState::Playing),
                    )
                    .with_system(update_input)
                    .with_system(
                        update_game
                            .run_in_state(EngineState::InGame)
                            .run_in_state(InGameState::Playing),
                    )
                    .with_system(play_sounds)
                    .with_run_criteria(FixedTimestep::step(1.0 / jumpy_core::FPS as f64)),
            );
    }
}

/// A resource containing an in-progress game session.
#[derive(Resource, Deref, DerefMut)]
pub struct Session(pub GameSession);

// Give bones_bevy_render plugin access to the bones world in our game session.
impl bones_bevy_renderer::HasBonesWorld for Session {
    fn world(&mut self) -> &mut bones::World {
        &mut self.0.world
    }
}

/// Helper for creating and stopping game sessions.
#[derive(SystemParam)]
pub struct SessionManager<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub menu_camera: Query<'w, 's, &'static mut Camera, With<MenuCamera>>,
    pub session: Option<ResMut<'w, Session>>,
    pub core_meta_arc: Res<'w, CoreMetaArc>,
}

impl<'w, 's> SessionManager<'w, 's> {
    /// Start a game session
    pub fn start(&mut self, info: GameSessionInfo) {
        let session = Session(GameSession::new(info));
        self.commands.insert_resource(session);
        self.menu_camera.for_each_mut(|mut x| x.is_active = false);
    }

    /// Restart a game session without changing the settings
    pub fn restart(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.restart();
        }
    }

    /// Stop a game session
    pub fn stop(&mut self) {
        self.commands.remove_resource::<Session>();
        self.menu_camera.for_each_mut(|mut x| x.is_active = true);
    }
}

/// Helper system to make sure there are two players on the board, if ever the game is in the middle
/// of playing and there are no players on the board.
///
/// This is primarily for the editor, which may be started without going through the player
/// selection screen.
fn ensure_2_players(session: Option<ResMut<Session>>, core_meta: Res<CoreMetaArc>) {
    if let Some(session) = session {
        let player_inputs = session.world.resource::<jumpy_core::input::PlayerInputs>();
        let mut player_inputs = player_inputs.borrow_mut();

        if player_inputs.players.iter().all(|x| !x.active) {
            for i in 0..2 {
                player_inputs.players[i].active = true;
                player_inputs.players[i].selected_player = core_meta.players[i].clone();
            }
        }
    }
}

/// Update the input to the game session.
fn update_input(
    session: Option<ResMut<Session>>,
    player_input_collectors: Query<(&PlayerInputCollector, &ActionState<PlayerAction>)>,
    mut current_editor_input: ResMut<CurrentEditorInput>,
) {
    let Some(mut session) = session else {
        return;
    };

    let mut editor_input = current_editor_input.take();

    session.update_input(|inputs| {
        // TODO: Properly handle which player is taking the editor input, which is important in
        // networked multiplayer.
        inputs.players[0].editor_input = editor_input.take();

        for (player_idx, action_state) in &player_input_collectors {
            let control = &mut inputs.players[player_idx.0].control;

            let jump_pressed = action_state.pressed(PlayerAction::Jump);
            control.jump_just_pressed = jump_pressed && !control.jump_pressed;
            control.jump_pressed = jump_pressed;

            let grab_pressed = action_state.pressed(PlayerAction::Grab);
            control.grab_just_pressed = grab_pressed && !control.grab_pressed;
            control.grab_pressed = grab_pressed;

            let shoot_pressed = action_state.pressed(PlayerAction::Shoot);
            control.shoot_just_pressed = shoot_pressed && !control.shoot_pressed;
            control.shoot_pressed = shoot_pressed;

            let was_moving = control.move_direction.length_squared() > f32::MIN_POSITIVE;
            control.move_direction = action_state.axis_pair(PlayerAction::Move).unwrap().xy();
            let is_moving = control.move_direction.length_squared() > f32::MIN_POSITIVE;
            control.just_moved = !was_moving && is_moving;
        }
    });
}

/// Update the game session simulation.
fn update_game(world: &mut World) {
    let Some(mut session) = world.remove_resource::<Session>() else {
        return;
    };

    // Advance the game session
    session.advance(world);

    world.insert_resource(session);
}

/// Play sounds from the game session.
fn play_sounds(audio: Res<AudioChannel<EffectsChannel>>, session: Option<Res<Session>>) {
    let Some(session) = session else {
        return;
    };

    // Get the sound queue out of the world
    let queue = session
        .world
        .run_initialized_system(move |mut audio_events: bones::ResMut<bones::AudioEvents>| {
            Ok(audio_events.queue.drain(..).collect::<Vec<_>>())
        })
        .unwrap();

    // Play all the sounds in the queue
    for event in queue {
        match event {
            bones::AudioEvent::PlaySound {
                sound_source,
                volume,
            } => {
                audio
                    .play(sound_source.get_bevy_handle_untyped().typed())
                    .with_volume(volume.into());
            }
        }
    }
}

use super::*;

pub const ID: Key = key!("core::dead");

pub fn player_state_transition(
    entities: Res<Entities>,
    killed_players: Comp<PlayerKilled>,
    mut player_states: CompMut<PlayerState>,
) {
    for (_ent, (state, _killed)) in entities.iter_with((&mut player_states, &killed_players)) {
        state.current = ID;
    }
}

pub fn handle_player_state(
    entities: Res<Entities>,
    player_states: Comp<PlayerState>,
    killed_players: Comp<PlayerKilled>,
    sprites: Comp<AtlasSprite>,
    transform: Comp<Transform>,
    mut animations: CompMut<AnimationBankSprite>,
    mut player_events: ResMut<PlayerEvents>,
) {
    for (player_ent, (state, animation, killed_player)) in
        entities.iter_with((&player_states, &mut animations, &killed_players))
    {
        if state.current != ID {
            continue;
        };

        if state.age == 0 {
            let sprite = sprites.get(player_ent).unwrap();
            let player_on_right = !sprite.flip_x;
            let transform = transform.get(player_ent).unwrap();

            animation.current = match killed_player.hit_from {
                Some(hit_from)
                    if {
                        let is_hit_right = transform.translation.x < hit_from.x;
                        (player_on_right && is_hit_right) || (!player_on_right && !is_hit_right)
                    } =>
                {
                    key!("death_spine")
                }
                _ => key!("death_belly"),
            };
        }

        if state.age >= 80 {
            player_events.despawn(player_ent);
        }
    }
}

use bevy::prelude::*;
use simulation::{DiffusionConfig, SimulationTime, normalization::BoundsNormalizer};

#[derive(Component)]
pub(super) struct OnScreenInfo;

pub(super) fn spawn_on_screen_info(
    commands: &mut Commands,
    diffusion: &DiffusionConfig,
    bounds: &BoundsNormalizer,
    time: &SimulationTime,
) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(12.0),
            left: px(12.0),
            max_width: percent(95.0),
            padding: UiRect::all(px(10.0)),
            flex_direction: FlexDirection::Column,
            row_gap: px(4.0),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((Text::new("Simulation Info"), TextFont::from_font_size(20.0)));
            parent.spawn((
                OnScreenInfo,
                Text::new(on_screen_info_text(diffusion, bounds, time)),
                TextFont::from_font_size(14.0),
            ));
        });
}

fn on_screen_info_text(
    diffusion: &DiffusionConfig,
    bounds: &BoundsNormalizer,
    time: &SimulationTime,
) -> String {
    let limits = match bounds.bounds() {
        Some((min, max)) => format!("({min:.2}, {max:.2})"),
        None => "N/A".to_string(),
    };

    let diffusion_details = match *diffusion {
        DiffusionConfig::None => "Diffusion: None".to_string(),
        DiffusionConfig::Enabled {
            neighbourhood,
            coefficient,
            spacing,
        } => format!(
            "Diffusion Neighborhood: {neighbourhood:?}\n\
             Diffusion Coefficient: {coefficient:.2}\n\
             Diffusion Spacing (h): {spacing:.2}"
        ),
    };

    let dt = time.timestep().as_secs_f64();

    format!(
        "{diffusion_details}\n\
         Bounds Averaging Method: {:?}\n\
         Bounds (min, max): {limits}\n\
         Simulation Time Step (dt): {dt:.2}",
        bounds.method(),
    )
}

pub(super) fn update_on_screen_info(
    diffusion: Res<DiffusionConfig>,
    bounds: Res<BoundsNormalizer>,
    time: Res<SimulationTime>,
    mut text: Single<&mut Text, With<OnScreenInfo>>,
) {
    if !diffusion.is_changed() && !bounds.is_changed() && !time.is_changed() {
        return;
    }

    text.set_if_neq(Text::new(on_screen_info_text(&diffusion, &bounds, &time)));
}

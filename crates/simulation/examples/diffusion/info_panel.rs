use bevy::prelude::*;
use simulation::{field::Diffuser, normalization::BoundsNormalizer};

use super::{
    BoundsNormalizerRes, DIFFUSION_COEFFICIENT, DIFFUSION_DT, DIFFUSION_SPACING, DiffuserRes,
};

#[derive(Component)]
pub(super) struct OnScreenInfo;

pub(super) fn spawn_on_screen_info(
    commands: &mut Commands,
    diffuser: &Diffuser,
    bounds: &BoundsNormalizer,
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
                Text::new(on_screen_info_text(diffuser, bounds)),
                TextFont::from_font_size(14.0),
            ));
        });
}

fn on_screen_info_text(diffuser: &Diffuser, bounds: &BoundsNormalizer) -> String {
    let limits = match bounds.bounds() {
        Some((min, max)) => format!("({min:.2}, {max:.2})"),
        None => "N/A".to_string(),
    };

    format!(
        "Diffusion Neighborhood: {:?}\n\
         Bounds Averaging Method: {:?}\n\
         Bounds (min, max): {limits}\n\
         Diffusion Coefficient: {DIFFUSION_COEFFICIENT:.2}\n\
         Diffusion Time Step (dt): {DIFFUSION_DT:.2}\n\
         Diffusion Spacing (h): {DIFFUSION_SPACING:.2}",
        diffuser.neighbourhood(),
        bounds.method(),
    )
}

pub(super) fn update_on_screen_info(
    diffuser: Res<DiffuserRes>,
    bounds: Res<BoundsNormalizerRes>,
    mut text: Single<&mut Text, With<OnScreenInfo>>,
) {
    if !diffuser.is_changed() && !bounds.is_changed() {
        return;
    }

    text.set_if_neq(Text::new(on_screen_info_text(&diffuser, &bounds)));
}

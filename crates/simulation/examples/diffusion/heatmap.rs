use bevy::prelude::*;
use simulation::{field::ResourceField, normalization::BoundsNormalizer};

const FONT_SIZE: f32 = 0.0;
const TILE_SIZE: f32 = 6.0;
const TILE_GAP: f32 = 0.0;

#[derive(Resource)]
pub(super) struct Heatmap {
    pub(super) tiles: Vec<Entity>,
}

#[derive(Component)]
pub(super) struct Tile;

pub(super) fn spawn_heatmap(
    commands: &mut Commands,
    resources: &ResourceField,
    bounds: &BoundsNormalizer,
) {
    let height = resources.height();
    let width = resources.width();
    let mut tiles = Vec::with_capacity(height * width);
    let offset = Vec2::new(width as f32, height as f32) * TILE_SIZE * 0.5;

    for y in 0..height {
        for x in 0..width {
            let value = resources.get_resource_at((x, y)).unwrap();
            let color = heatmap_color(bounds.normalize(value));
            let position =
                Vec2::new((x as f32 + 0.5) * TILE_SIZE, (y as f32 + 0.5) * TILE_SIZE) - offset;

            let mut tile = commands.spawn((
                Tile,
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(TILE_SIZE - TILE_GAP)),
                    ..default()
                },
                Transform::from_xyz(position.x, position.y, 0.0),
            ));

            if FONT_SIZE > 0.0 {
                tile.with_child((
                    Text2d::new(format!("{value:.2}")),
                    TextFont {
                        font_size: FontSize::Px(FONT_SIZE),
                        ..default()
                    },
                    TextColor(contrasting_text_color(color)),
                    Transform::from_xyz(0.0, 0.0, 1.0).with_scale(Vec3::splat(0.5)),
                ));
            }

            tiles.push(tile.id());
        }
    }

    commands.insert_resource(Heatmap { tiles });
}

pub(super) fn update_heatmap(
    heatmap: Res<Heatmap>,
    resources: Res<ResourceField>,
    bounds: Res<BoundsNormalizer>,
    mut tiles: Query<(&mut Sprite, Option<&Children>), With<Tile>>,
    mut texts: Query<(&mut Text2d, &mut TextColor)>,
) {
    if !resources.is_changed() && !bounds.is_changed() {
        return;
    }

    for (&entity, &value) in heatmap.tiles.iter().zip(resources.resources()) {
        let Ok((mut sprite, children)) = tiles.get_mut(entity) else {
            continue;
        };

        let color = heatmap_color(bounds.normalize(value));

        if sprite.color != color {
            sprite.color = color;
        }

        let Some(children) = children else {
            continue;
        };

        let label_color = contrasting_text_color(color);

        for child in children.iter() {
            if let Ok((mut text, mut text_color)) = texts.get_mut(child) {
                let label = format!("{value:.2}");

                if text.0 != label {
                    text.0 = label;
                }

                if text_color.0 != label_color {
                    text_color.0 = label_color;
                }
            }
        }
    }
}

fn contrasting_text_color(background: Color) -> Color {
    let rgb = background.to_linear();
    let luminance = 0.2126 * rgb.red + 0.7152 * rgb.green + 0.0722 * rgb.blue;

    if luminance > 0.179 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

pub(super) fn heatmap_color(t: f32) -> Color {
    const MIN: Color = Color::linear_rgb(35.0 / 255.0, 55.0 / 255.0, 140.0 / 255.0);
    const MID: Color = Color::linear_rgb(37.0 / 255.0, 173.0 / 255.0, 154.0 / 255.0);
    const MAX: Color = Color::linear_rgb(253.0 / 255.0, 220.0 / 255.0, 70.0 / 255.0);

    let t = t.clamp(0.0, 1.0);

    if t < 0.5 {
        MIN.mix(&MID, t * 2.0)
    } else {
        MID.mix(&MAX, (t - 0.5) * 2.0)
    }
}

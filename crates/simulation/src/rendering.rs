use bevy::prelude::*;

use crate::{Position, field::ResourceField, normalization::BoundsNormalizer};

pub const UI_COLORS: catppuccin::FlavorColors = catppuccin::PALETTE.mocha.colors;

const FONT_SIZE: f32 = 0.0;
pub const TILE_SIZE: f32 = 6.0;
const TILE_GAP: f32 = 0.0;

/// Sprite entities in the resource field's row-major order.
#[derive(Resource)]
pub struct Heatmap {
    pub tiles: Vec<Entity>,
}

#[derive(Component)]
pub struct Tile;

/// Returns the centre of a cell in the heatmap's world coordinates.
pub fn cell_center(field: &ResourceField, position: Position) -> Vec2 {
    let offset = Vec2::new(field.width() as f32, field.height() as f32) * TILE_SIZE * 0.5;
    Vec2::new(position.x as f32 + 0.5, position.y as f32 + 0.5) * TILE_SIZE - offset
}

pub fn spawn_heatmap(
    commands: &mut Commands,
    resources: &ResourceField,
    bounds: &BoundsNormalizer,
) {
    let height = resources.height();
    let width = resources.width();
    let mut tiles = Vec::with_capacity(height * width);

    for y in 0..height {
        for x in 0..width {
            let value = resources.get_resource_at((x, y)).unwrap();
            let color = heatmap_color(bounds.normalize(value));
            let position = cell_center(resources, Position::new(x, y));

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

pub fn redraw_heatmap(
    heatmap: &Heatmap,
    resources: &ResourceField,
    bounds: &BoundsNormalizer,
    tiles: &mut Query<(&mut Sprite, Option<&Children>), With<Tile>>,
    texts: &mut Query<(&mut Text2d, &mut TextColor)>,
) {
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

pub fn heatmap_color(t: f32) -> Color {
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

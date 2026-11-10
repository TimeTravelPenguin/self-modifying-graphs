use std::time::Duration;

use bevy::prelude::*;
use simulation::simulation::{
    bounds::{BoundsAveraging, BoundsNormalizer},
    diffusion::{Diffuser, Neighbourhood},
    resources::ResourceField,
};

const GRID_ROWS: u32 = 100;
const GRID_COLS: u32 = 100;
const RANDOM_SEED: u32 = 43;
const SCALE: f64 = 10.0;
const MAX_RESOURCE: f64 = 100.0;

const FONT_SIZE: f32 = 0.0;
const TILE_SIZE: f32 = 6.0;
const TILE_GAP: f32 = 0.0;

const DIFFUSION_COEFFICIENT: f64 = 0.8;
const DIFFUSION_DT: f64 = 0.1;
const DIFFUSION_SPACING: f64 = 1.0;
const STEP_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Resource)]
struct Heatmap {
    tiles: Vec<Entity>,
}

#[derive(Component)]
struct Tile;

#[derive(Resource, Deref, DerefMut)]
struct DiffuserRes(Diffuser);

#[derive(Resource, Deref, DerefMut)]
struct ResourceFieldRes(ResourceField);

#[derive(Resource, Deref, DerefMut)]
struct BoundsNormalizerRes(BoundsNormalizer);

fn main() -> Result<(), String> {
    let diffuser = Diffuser::new(Neighbourhood::VonNeumann);
    let mut bounds_norm = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });
    // let mut bounds_norm = BoundsNormalizer::new(BoundsAveraging::None);
    let resources =
        ResourceField::new_random(GRID_ROWS, GRID_COLS, RANDOM_SEED, SCALE, MAX_RESOURCE);

    bounds_norm.update_bounds(&resources, DIFFUSION_DT);

    let _ = App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Time::<Fixed>::from_duration(STEP_INTERVAL))
        .insert_resource(DiffuserRes(diffuser))
        .insert_resource(BoundsNormalizerRes(bounds_norm))
        .insert_resource(ResourceFieldRes(resources))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, apply_diffusion)
        .add_systems(Update, update_heatmap)
        .run();

    Ok(())
}

fn setup(
    mut commands: Commands,
    resources: Res<ResourceFieldRes>,
    bounds: Res<BoundsNormalizerRes>,
) {
    commands.spawn(Camera2d);

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

fn apply_diffusion(
    mut diffuser: ResMut<DiffuserRes>,
    mut resources: ResMut<ResourceFieldRes>,
    mut bounds: ResMut<BoundsNormalizerRes>,
) {
    diffuser.diffuse(
        &mut resources,
        DIFFUSION_COEFFICIENT,
        DIFFUSION_DT,
        DIFFUSION_SPACING,
    );
    bounds.update_bounds(&resources, DIFFUSION_DT);
}

fn update_heatmap(
    heatmap: Res<Heatmap>,
    resources: Res<ResourceFieldRes>,
    bounds: Res<BoundsNormalizerRes>,
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

fn heatmap_color(t: f32) -> Color {
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

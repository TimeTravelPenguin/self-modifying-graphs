use std::time::Duration;

use bevy::prelude::*;
use bevy_rand::{global::GlobalRng, plugin::EntropyPlugin, prelude::ChaCha12Rng};
use rand::RngExt;
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
struct OnScreenInfo;

#[derive(Component)]
struct Tile;

#[derive(Resource, Deref, DerefMut)]
struct DiffuserRes(Diffuser);

#[derive(Resource, Deref, DerefMut)]
struct ResourceFieldRes(ResourceField);

#[derive(Resource, Deref, DerefMut)]
struct BoundsNormalizerRes(BoundsNormalizer);

fn main() -> AppExit {
    let resources =
        ResourceField::new_random(GRID_ROWS, GRID_COLS, RANDOM_SEED, SCALE, MAX_RESOURCE);

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1280, 850).into(),
                ..default()
            }),
            ..default()
        }),
        EntropyPlugin::<ChaCha12Rng>::default(),
    ));

    configure_simulation(&mut app, resources);

    println!("{:#?}", WindowPlugin::default().primary_window);
    app.run()
}

fn configure_simulation(app: &mut App, resources: ResourceField) {
    let diffuser = Diffuser::new(Neighbourhood::VonNeumann);
    let mut bounds_norm = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });
    // let mut bounds_norm = BoundsNormalizer::new(BoundsAveraging::None);
    bounds_norm.update_bounds(&resources, DIFFUSION_DT);

    app.insert_resource(Time::<Fixed>::from_duration(STEP_INTERVAL))
        .insert_resource(DiffuserRes(diffuser))
        .insert_resource(BoundsNormalizerRes(bounds_norm))
        .insert_resource(ResourceFieldRes(resources))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, apply_diffusion)
        .add_systems(Update, handle_keyboard_input)
        .add_systems(
            Update,
            (update_heatmap, update_on_screen_info).after(handle_keyboard_input),
        );
}

fn setup(
    mut commands: Commands,
    resources: Res<ResourceFieldRes>,
    bounds: Res<BoundsNormalizerRes>,
    diffuser: Res<DiffuserRes>,
) {
    commands.spawn(Camera2d);

    let height = resources.height();
    let width = resources.width();
    let mut tiles = Vec::with_capacity(height * width);
    let offset = Vec2::new(width as f32, height as f32) * TILE_SIZE * 0.5;

    spawn_tiles(
        &mut commands,
        &mut tiles,
        &resources,
        &bounds,
        height,
        width,
        offset,
    );

    commands.insert_resource(Heatmap { tiles });

    spawn_on_screen_info(&mut commands, &diffuser, &bounds);
}

fn spawn_tiles(
    commands: &mut Commands,
    tiles: &mut Vec<Entity>,
    resources: &ResourceField,
    bounds: &BoundsNormalizer,
    height: usize,
    width: usize,
    offset: Vec2,
) {
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
}

fn spawn_on_screen_info(commands: &mut Commands, diffuser: &Diffuser, bounds: &BoundsNormalizer) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12.0),
                left: px(12.0),
                max_width: percent(95.0),
                padding: UiRect::all(px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.0),
                ..default()
            },
            // BackgroundColor(Color::srgb(0.08, 0.09, 0.12)),
        ))
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

fn handle_keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut resources: ResMut<ResourceFieldRes>,
    mut diffuser: ResMut<DiffuserRes>,
    mut bounds: ResMut<BoundsNormalizerRes>,
    mut rng: Single<&mut ChaCha12Rng, With<GlobalRng>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        let neighbourhood = match diffuser.neighbourhood() {
            Neighbourhood::VonNeumann => Neighbourhood::Moore,
            Neighbourhood::Moore => Neighbourhood::VonNeumann,
        };

        diffuser.set_neighbourhood(neighbourhood);
        println!("Switched to {:?}", neighbourhood);
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) {
        let seed = rng.random();
        resources.randomize(seed, SCALE, MAX_RESOURCE);
        bounds.reset_bounds();
        bounds.update_bounds(&resources, DIFFUSION_DT);
        println!("Reset resource field with seed {seed}");
    }
}

fn update_on_screen_info(
    diffuser: Res<DiffuserRes>,
    bounds: Res<BoundsNormalizerRes>,
    mut text: Single<&mut Text, With<OnScreenInfo>>,
) {
    if !diffuser.is_changed() && !bounds.is_changed() {
        return;
    }

    text.set_if_neq(Text::new(on_screen_info_text(&diffuser, &bounds)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_screen_info_tracks_diffusion_and_keyboard_input() {
        let mut resources = ResourceField::new_random(2, 2, RANDOM_SEED, SCALE, MAX_RESOURCE);
        resources
            .resources_mut()
            .copy_from_slice(&[20.0, 40.0, 60.0, 80.0]);

        let mut app = App::new();
        app.add_plugins(EntropyPlugin::<ChaCha12Rng>::with_seed([0; 32]))
            .init_resource::<ButtonInput<KeyCode>>();
        configure_simulation(&mut app, resources);

        app.update();
        let world = app.world_mut();
        let panel = world
            .query_filtered::<Entity, With<OnScreenInfo>>()
            .single(world)
            .unwrap();
        let text = world.get::<Text>(panel).unwrap();
        assert!(text.contains("Diffusion Neighborhood: VonNeumann"));
        assert!(text.contains("Bounds (min, max): (20.00, 80.00)"));

        world.run_schedule(FixedUpdate);
        app.update();
        let text = app.world().get::<Text>(panel).unwrap();
        assert!(text.contains("Bounds (min, max): (20.46, 79.54)"));

        let previous_field = app
            .world()
            .resource::<ResourceFieldRes>()
            .resources()
            .clone();
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.press(KeyCode::Space);
        keyboard.press(KeyCode::KeyR);
        app.update();

        let world = app.world();
        let bounds = world.resource::<BoundsNormalizerRes>();
        let (min, max) = bounds.bounds().unwrap();
        let text = world.get::<Text>(panel).unwrap();
        assert!(text.contains("Diffusion Neighborhood: Moore"));
        assert!(text.contains(&format!("Bounds (min, max): ({min:.2}, {max:.2})")));

        let resources = world.resource::<ResourceFieldRes>();
        assert_ne!(resources.resources(), &previous_field);

        let heatmap = world.resource::<Heatmap>();
        for (&entity, &value) in heatmap.tiles.iter().zip(resources.resources()) {
            let sprite = world.get::<Sprite>(entity).unwrap();
            assert_eq!(sprite.color, heatmap_color(bounds.normalize(value)));
        }
    }
}

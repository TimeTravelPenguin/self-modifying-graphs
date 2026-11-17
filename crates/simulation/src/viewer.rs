//! Bevy presentation of completed simulation snapshots.

use std::f32::consts::FRAC_PI_4;

use bevy::prelude::*;
use simulation::{
    normalization::{BoundsAveraging, BoundsNormalizer},
    rendering::{
        Heatmap, TILE_SIZE, Tile, UI_COLORS, cell_center, heatmap_color, redraw_heatmap,
        spawn_heatmap,
    },
    simulation::{SimulationSnapshot, SnapshotReader},
};

/// Draws completed snapshots from the app's [`SnapshotReader`] resource.
pub(super) struct ViewerPlugin {
    bounds_normalizer: BoundsNormalizer,
}

impl ViewerPlugin {
    /// Configures the heatmap and legend with the given bounds normalizer.
    ///
    /// The default normalizer holds the scale fixed until the field is reseeded.
    /// Exponential averaging also samples each newly displayed snapshot.
    pub(super) fn new(bounds_normalizer: BoundsNormalizer) -> Self {
        Self { bounds_normalizer }
    }
}

impl Plugin for ViewerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(UI_COLORS.base.into()))
            .insert_resource(SnapshotView {
                displayed_tick: 0,
                displayed_time: 0.0,
                field_seed: 0,
                bounds: self.bounds_normalizer.clone(),
            })
            .add_systems(Startup, setup)
            .add_systems(Update, refresh);
    }
}

#[derive(Resource)]
struct SnapshotView {
    displayed_tick: u64,
    displayed_time: f64,
    field_seed: u32,
    bounds: BoundsNormalizer,
}

#[derive(Component)]
struct VertexMarker;

#[derive(Component)]
struct RunStatus;

#[derive(Component)]
enum LegendBound {
    Minimum,
    Maximum,
}

impl LegendBound {
    fn text(&self, (min, max): (f64, f64)) -> Text {
        match self {
            Self::Minimum => Text::new(format!("Cell resource    {min:.1}")),
            Self::Maximum => Text::new(format!("{max:.1}")),
        }
    }
}

fn setup(mut commands: Commands, reader: Res<SnapshotReader>, mut view: ResMut<SnapshotView>) {
    let snapshot = reader.latest();
    view.bounds.reset_bounds();
    view.bounds.update_bounds(&snapshot.field, 0.0);
    view.displayed_tick = snapshot.tick;
    view.displayed_time = snapshot.simulation_time;
    view.field_seed = snapshot.field_seed;

    commands.spawn(Camera2d);
    spawn_heatmap(&mut commands, &snapshot.field, &view.bounds);

    let vertex = snapshot.vertices.first().expect("starting vertex exists");
    let position = cell_center(&snapshot.field, vertex.position);
    commands
        .spawn((
            VertexMarker,
            Sprite::from_color(UI_COLORS.crust, Vec2::splat(TILE_SIZE * 1.9)),
            Transform::from_xyz(position.x, position.y, 1.0)
                .with_rotation(Quat::from_rotation_z(FRAC_PI_4)),
        ))
        .with_child((
            Sprite::from_color(UI_COLORS.rosewater, Vec2::splat(TILE_SIZE * 1.25)),
            Transform::from_xyz(0.0, 0.0, 0.1),
        ));

    spawn_header(&mut commands, &snapshot);
    spawn_legend(&mut commands, &view.bounds);
}

fn refresh(
    reader: Res<SnapshotReader>,
    mut view: ResMut<SnapshotView>,
    heatmap: Res<Heatmap>,
    mut tiles: Query<(&mut Sprite, Option<&Children>), With<Tile>>,
    mut labels: Query<(&mut Text2d, &mut TextColor)>,
    mut marker: Single<&mut Transform, With<VertexMarker>>,
    mut status: Single<&mut Text, With<RunStatus>>,
    mut legend: Query<(&LegendBound, &mut Text), Without<RunStatus>>,
) {
    let snapshot = reader.latest();
    let reseeded = snapshot.field_seed != view.field_seed;

    if snapshot.tick == view.displayed_tick && !reseeded {
        return;
    }

    if reseeded {
        view.bounds.reset_bounds();
    }

    if reseeded || matches!(view.bounds.method(), BoundsAveraging::EMA { .. }) {
        let elapsed = snapshot.simulation_time - view.displayed_time;
        view.bounds.update_bounds(&snapshot.field, elapsed);

        let bounds = view.bounds.bounds().expect("simulation field is nonempty");

        for (bound, mut label) in &mut legend {
            label.set_if_neq(bound.text(bounds));
        }
    }

    redraw_heatmap(
        &heatmap,
        &snapshot.field,
        &view.bounds,
        &mut tiles,
        &mut labels,
    );

    let vertex = snapshot.vertices.first().expect("starting vertex exists");
    let position = cell_center(&snapshot.field, vertex.position);
    marker.translation = position.extend(1.0);
    status.set_if_neq(Text::new(status_text(&snapshot)));
    view.displayed_tick = snapshot.tick;
    view.displayed_time = snapshot.simulation_time;
    view.field_seed = snapshot.field_seed;
}

fn spawn_header(commands: &mut Commands, snapshot: &SimulationSnapshot) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                right: px(0.0),
                top: px(0.0),
                height: px(60.0),
                padding: UiRect::horizontal(px(24.0)),
                align_items: AlignItems::Center,
                align_content: AlignContent::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(4.0),
                column_gap: px(16.0),
                ..default()
            },
            BackgroundColor(UI_COLORS.surface0.into()),
        ))
        .with_children(|header| {
            header.spawn((
                Text::new("Graph simulation"),
                TextFont::from_font_size(21.0),
                TextColor(UI_COLORS.text.into()),
            ));
            header.spawn((
                RunStatus,
                Text::new(status_text(snapshot)),
                TextFont::from_font_size(15.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
        });
}

fn status_text(snapshot: &SimulationSnapshot) -> String {
    let vertex = snapshot.vertices.first().expect("starting vertex exists");

    format!(
        "Seed {}  |  Tick {}  |  Time {:.1}  |  Stored resource {:.2}",
        snapshot.seed, snapshot.tick, snapshot.simulation_time, vertex.stored_resource
    )
}

fn spawn_legend(commands: &mut Commands, bounds: &BoundsNormalizer) {
    let bounds = bounds.bounds().expect("simulation field is nonempty");

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            right: px(0.0),
            bottom: px(24.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: px(10.0),
            ..default()
        })
        .with_children(|legend| {
            legend.spawn((
                LegendBound::Minimum,
                LegendBound::Minimum.text(bounds),
                TextFont::from_font_size(14.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
            legend.spawn(Node::default()).with_children(|scale| {
                for index in 0..12 {
                    scale.spawn((
                        Node {
                            width: px(12.0),
                            height: px(8.0),
                            ..default()
                        },
                        BackgroundColor(heatmap_color(index as f32 / 11.0)),
                    ));
                }
            });
            legend.spawn((
                LegendBound::Maximum,
                LegendBound::Maximum.text(bounds),
                TextFont::from_font_size(14.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
        });
}

#[cfg(test)]
mod tests {
    use std::f64::consts::LN_2;

    use simulation::{
        field::Neighbourhood,
        graph::Uptake,
        simulation::{SimulationConfig, SimulationRun, SnapshotPublisher},
    };

    use super::*;

    #[test]
    fn viewer_draws_the_vertex_and_refreshes_from_a_completed_tick() {
        let config = SimulationConfig {
            width: 9,
            height: 7,
            spacing: 1.0,
            initial_resource: 100.0,
            resource_noise_scale: 10.0,
            diffusion_coefficient: 0.8,
            neighbourhood: Neighbourhood::Moore,
        };

        let uptake = Uptake::new(10.0, 0.75).unwrap();
        let mut run = SimulationRun::single_vertex_from_seed(config, 42, uptake, 0.1).unwrap();
        let initial = run.snapshot();
        let vertex = initial.vertices[0];
        let center = cell_center(&initial.field, vertex.position);
        let mut publisher = SnapshotPublisher::new(initial);
        let mut app = App::new();
        app.insert_resource(publisher.reader())
            .add_plugins(ViewerPlugin::new(BoundsNormalizer::default()));
        app.update();

        let world = app.world_mut();
        let marker = world
            .query_filtered::<Entity, With<VertexMarker>>()
            .single(world)
            .unwrap();
        let status = world
            .query_filtered::<Entity, With<RunStatus>>()
            .single(world)
            .unwrap();
        assert_eq!(
            world.get::<Transform>(marker).unwrap().translation,
            center.extend(1.0)
        );
        assert!(world.get::<Text>(status).unwrap().0.contains("Tick 0"));
        let tiles = world.resource::<Heatmap>().tiles.clone();
        assert_eq!(tiles.len(), 63);
        let initial_bounds = world.resource::<SnapshotView>().bounds.bounds();

        run.advance();
        let next = run.snapshot();
        publisher.publish(next.clone());
        app.update();

        let world = app.world();
        assert_eq!(world.resource::<Heatmap>().tiles, tiles);
        assert_eq!(world.get::<Text>(status).unwrap().0, status_text(&next));
        assert!(
            world
                .get::<Text>(status)
                .unwrap()
                .0
                .contains("Stored resource 0.75")
        );

        let view = world.resource::<SnapshotView>();
        assert_eq!(view.displayed_tick, 1);
        assert_eq!(view.bounds.bounds(), initial_bounds);

        for (tile, &amount) in tiles.into_iter().zip(next.field.resources()) {
            let sprite = world.get::<Sprite>(tile).unwrap();
            assert_eq!(sprite.color, heatmap_color(view.bounds.normalize(amount)));
        }

        let mut reseeded = next;
        reseeded.field_seed = 24;
        reseeded.field.randomize(reseeded.field_seed, 10.0, 200.0);
        publisher.publish(reseeded);
        app.update();

        let world = app.world_mut();
        let view = world.resource::<SnapshotView>();
        assert_eq!(view.field_seed, 24);
        let bounds = view.bounds.bounds().unwrap();
        let mut legend = world.query::<(&LegendBound, &Text)>();

        for (bound, label) in legend.iter(world) {
            assert_eq!(*label, bound.text(bounds));
        }
    }

    #[test]
    fn configured_normalizer_smooths_the_heatmap_and_legend_until_reseed() {
        let config = SimulationConfig {
            width: 3,
            height: 1,
            spacing: 1.0,
            initial_resource: 100.0,
            resource_noise_scale: 10.0,
            diffusion_coefficient: 0.0,
            neighbourhood: Neighbourhood::Moore,
        };

        let uptake = Uptake::new(1.0, 1.0).unwrap();
        let run = SimulationRun::single_vertex_from_seed(config, 42, uptake, LN_2 / 2.0).unwrap();
        let mut snapshot = run.snapshot();
        snapshot
            .field
            .resources_mut()
            .copy_from_slice(&[20.0, 35.0, 80.0]);
        let mut publisher = SnapshotPublisher::new(snapshot.clone());
        let mut app = App::new();
        app.insert_resource(publisher.reader())
            .add_plugins(ViewerPlugin::new(BoundsNormalizer::new(
                BoundsAveraging::EMA { tau: 1.0 },
            )));
        app.update();

        snapshot.tick = 2;
        snapshot.simulation_time = LN_2;
        snapshot
            .field
            .resources_mut()
            .copy_from_slice(&[40.0, 45.0, 60.0]);
        publisher.publish(snapshot.clone());
        app.update();

        let world = app.world_mut();
        let bounds = (30.0, 70.0);
        assert_eq!(
            world.resource::<SnapshotView>().bounds.bounds(),
            Some(bounds)
        );
        let middle = world.resource::<Heatmap>().tiles[1];
        assert_eq!(
            world.get::<Sprite>(middle).unwrap().color,
            heatmap_color(0.375)
        );
        let mut legend = world.query::<(&LegendBound, &Text)>();
        assert_eq!(legend.iter(world).count(), 2);

        for (bound, label) in legend.iter(world) {
            assert_eq!(*label, bound.text(bounds));
        }

        snapshot.field_seed = 43;
        publisher.publish(snapshot);
        app.update();

        let view = app.world().resource::<SnapshotView>();
        assert_eq!(view.bounds.bounds(), Some((40.0, 60.0)));
    }
}

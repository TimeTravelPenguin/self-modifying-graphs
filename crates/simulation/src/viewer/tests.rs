use std::f64::consts::LN_2;

use crate::{
    effects::EffectMultipliers,
    field::{Neighbourhood, ResourceField},
    graph::{Graph, HealthConfig, Uptake, VertexConfig, VertexId},
    normalization::{BoundsAveraging, BoundsNormalizer},
    palette::{self, heatmap_color},
    rendering::{Heatmap, cell_center},
    simulation::{
        SimulationConfig, SimulationRun, SimulationSnapshot, SnapshotPublisher, VertexSnapshot,
    },
};
use bevy::{prelude::*, window::PrimaryWindow};
use slotmap::Key;

use super::{
    SnapshotView, VertexMarker, ViewerPlugin, Viewport, layout::ViewerLayout, marker_at, ui::UiText,
};

const WINDOW_SIZE: Vec2 = Vec2::new(1040.0, 820.0);

fn headless_viewer(
    snapshot: SimulationSnapshot,
    bounds: BoundsNormalizer,
) -> (App, SnapshotPublisher) {
    let publisher = SnapshotPublisher::new(snapshot);
    let mut app = App::new();
    app.insert_resource(publisher.reader())
        .add_plugins(ViewerPlugin::new(bounds));
    app.update();
    (app, publisher)
}

fn ui_text(world: &mut World, kind: UiText) -> String {
    let mut labels = world.query::<(&UiText, &Text)>();
    labels
        .iter(world)
        .find(|(label, _)| **label == kind)
        .unwrap()
        .1
        .0
        .clone()
}

fn marker_entity(world: &mut World, id: VertexId) -> Option<Entity> {
    let mut markers = world.query::<(Entity, &VertexMarker)>();
    markers
        .iter(world)
        .find_map(|(entity, marker)| (marker.0 == id).then_some(entity))
}

fn show_window(app: &mut App) -> Entity {
    app.init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>();
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: (1040, 820).into(),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    app.update();
    window
}

fn screen_point(app: &App, position: Vec2) -> Vec2 {
    let viewport = app.world().resource::<Viewport>();
    let field = viewport.field.as_ref().unwrap();
    let window_size = viewport.window_size.unwrap();
    let offset = position - field.camera_offset;

    window_size * 0.5 + Vec2::new(offset.x, -offset.y) / field.scale
}

fn click(app: &mut App, window: Entity, point: Vec2) {
    let world = app.world_mut();
    world
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(point));
    world
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset(MouseButton::Left);
}

fn adjacent_vertices() -> (SimulationSnapshot, VertexSnapshot) {
    let mut graph = Graph::new();
    let uptake = Uptake::new(1.0, 1.0).unwrap();
    let config = VertexConfig::new(uptake, 0.0, HealthConfig::default()).unwrap();
    let position = (4, 2).into();
    let initial = VertexSnapshot {
        id: graph.add_vertex(position, config.clone()),
        position,
        stored_resource: 2.5,
        age: 0.0,
        health: Some(10.0),
        config,
        effects_revision: 0,
        effective_effects: EffectMultipliers::default(),
        effective_recovery: Some(0.25),
    };

    let config = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap();
    let position = (5, 2).into();
    let adjacent = VertexSnapshot {
        id: graph.add_vertex(position, config.clone()),
        position,
        stored_resource: 0.0,
        age: 0.0,
        health: None,
        config,
        effects_revision: 0,
        effective_effects: EffectMultipliers::default(),
        effective_recovery: None,
    };

    let field = ResourceField::new_random(7, 11, 42, 10.0, 0.0);
    let snapshot = SimulationSnapshot {
        seed: 42,
        field_seed: 42,
        tick: 0,
        simulation_time: 0.0,
        field,
        vertices: vec![initial],
    };

    (snapshot, adjacent)
}

#[test]
fn reconciles_vertices_by_id_and_follows_live_selected_values() {
    let (mut snapshot, adjacent) = adjacent_vertices();
    let initial = snapshot.vertices[0].clone();
    let center = cell_center(&snapshot.field, initial.position);
    let (mut app, mut publisher) = headless_viewer(snapshot.clone(), BoundsNormalizer::default());
    let window = show_window(&mut app);
    let world = app.world_mut();
    let original = marker_entity(world, initial.id).unwrap();
    assert_eq!(
        world.get::<Sprite>(original).unwrap().color,
        palette::MARKER_OUTLINE
    );

    assert_eq!(
        world
            .get::<Transform>(original)
            .unwrap()
            .translation
            .truncate(),
        center
    );
    assert_eq!(world.resource::<SnapshotView>().selected, None);
    assert_eq!(ui_text(world, UiText::InspectorPrompt), "Select a vertex");
    assert!(!ui_text(world, UiText::RunStatus).contains("Stored resource"));

    let point = screen_point(&app, center);
    click(&mut app, window, point);
    let world = app.world_mut();

    assert_eq!(world.resource::<SnapshotView>().selected, Some(initial.id));
    assert_eq!(
        world
            .resource::<SnapshotView>()
            .displayed
            .as_ref()
            .unwrap()
            .tick,
        0
    );
    assert_eq!(
        world.get::<Sprite>(original).unwrap().color,
        palette::SELECTED_MARKER_OUTLINE
    );
    assert_eq!(
        ui_text(world, UiText::VertexId),
        format!("{:?}", initial.id.data())
    );
    assert_eq!(ui_text(world, UiText::CellPosition), "(4, 2)");
    assert_eq!(ui_text(world, UiText::StoredResource), "2.50");
    assert_eq!(ui_text(world, UiText::CellResource), "0.00");
    assert_eq!(ui_text(world, UiText::Age), "0.0");
    assert_eq!(ui_text(world, UiText::Health), "10.00 / 10.00");
    assert_eq!(ui_text(world, UiText::UptakeCapacity), "1.00x");
    assert_eq!(ui_text(world, UiText::RecoveryPerTick), "0.250");

    snapshot.tick = 3;
    snapshot.simulation_time = 0.3;
    snapshot.vertices[0].stored_resource = 7.25;
    snapshot.vertices[0].position = (4, 3).into();
    snapshot.vertices[0].age = 12.5;
    snapshot.vertices[0].health = Some(8.25);
    snapshot.vertices[0].effective_effects.uptake_capacity = 1.5;
    snapshot.vertices[0].effective_recovery = Some(0.125);
    let cell_index = 3 * snapshot.field.width() + 4;
    snapshot.field.resources_mut()[cell_index] = 12.5;
    snapshot.vertices.push(adjacent.clone());
    let next_center = cell_center(&snapshot.field, (4, 3).into());
    publisher.publish(snapshot.clone());
    app.update();

    let world = app.world_mut();

    assert_eq!(marker_entity(world, initial.id), Some(original));
    assert!(marker_entity(world, adjacent.id).is_some());
    assert_eq!(
        world
            .get::<Transform>(original)
            .unwrap()
            .translation
            .truncate(),
        next_center
    );
    assert_eq!(world.resource::<SnapshotView>().selected, Some(initial.id));
    assert_eq!(ui_text(world, UiText::CellPosition), "(4, 3)");
    assert_eq!(ui_text(world, UiText::StoredResource), "7.25");
    assert_eq!(ui_text(world, UiText::CellResource), "12.50");
    assert_eq!(ui_text(world, UiText::Age), "12.5");
    assert_eq!(ui_text(world, UiText::Health), "8.25 / 10.00");
    assert_eq!(ui_text(world, UiText::UptakeCapacity), "1.50x");
    assert_eq!(ui_text(world, UiText::RecoveryPerTick), "0.125");

    snapshot.tick += 1;
    snapshot.simulation_time += 0.1;
    snapshot.vertices.clear();
    publisher.publish(snapshot);
    app.update();

    let world = app.world_mut();

    assert_eq!(world.query::<&VertexMarker>().iter(world).count(), 0);
    assert_eq!(world.resource::<SnapshotView>().selected, None);
    assert_eq!(ui_text(world, UiText::InspectorPrompt), "No live vertices");
    assert!(ui_text(world, UiText::Health).is_empty());
    assert!(ui_text(world, UiText::RecoveryPerTick).is_empty());
}

#[test]
fn selects_nearest_overlapping_marker_and_ignores_clicks_on_ui() {
    let (mut snapshot, adjacent) = adjacent_vertices();
    let initial = snapshot.vertices[0].clone();
    snapshot.vertices.insert(0, adjacent.clone());
    let initial_center = cell_center(&snapshot.field, initial.position);
    let adjacent_center = cell_center(&snapshot.field, adjacent.position);
    let midpoint = (initial_center + adjacent_center) * 0.5;

    assert_eq!(marker_at(&snapshot, initial_center), Some(initial.id));
    assert_eq!(marker_at(&snapshot, adjacent_center), Some(adjacent.id));
    assert_eq!(
        marker_at(&snapshot, midpoint),
        Some(initial.id.min(adjacent.id))
    );

    let (mut app, _) = headless_viewer(snapshot, BoundsNormalizer::default());
    let window = show_window(&mut app);
    let point = screen_point(&app, adjacent_center);
    click(&mut app, window, point);
    let world = app.world_mut();
    assert_eq!(ui_text(world, UiText::Health), "Disabled");
    assert_eq!(ui_text(world, UiText::RecoveryPerTick), "Disabled");

    let layout = ViewerLayout::new(WINDOW_SIZE);

    for region in [layout.header, layout.legend, layout.inspector] {
        click(&mut app, window, region.center());
        assert_eq!(
            app.world().resource::<SnapshotView>().selected,
            Some(adjacent.id)
        );
    }

    click(&mut app, window, layout.field.min + Vec2::splat(1.0));
    assert_eq!(app.world().resource::<SnapshotView>().selected, None);

    click(&mut app, window, point);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert_eq!(app.world().resource::<SnapshotView>().selected, None);

    let world = app.world_mut();
    let marker = marker_entity(world, adjacent.id).unwrap();
    assert_eq!(
        world.get::<Sprite>(marker).unwrap().color,
        palette::MARKER_OUTLINE
    );
}

#[test]
fn default_normalizer_keeps_the_heatmap_scale_until_reseed() {
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
    let vertex = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap();
    let mut run = SimulationRun::single_vertex_from_seed(config, 42, vertex, 0.1).unwrap();
    let (mut app, mut publisher) = headless_viewer(run.snapshot(), BoundsNormalizer::default());
    let world = app.world_mut();
    let tiles = world.resource::<Heatmap>().tiles.clone();
    let initial_bounds = world.resource::<SnapshotView>().bounds.bounds();

    assert_eq!(tiles.len(), 63);
    assert!(ui_text(world, UiText::RunStatus).contains("Tick 0"));

    run.advance();
    let next = run.snapshot();
    publisher.publish(next.clone());
    app.update();

    let world = app.world_mut();

    assert!(ui_text(world, UiText::RunStatus).contains("Tick 1"));
    assert_eq!(world.resource::<Heatmap>().tiles, tiles);
    let view = world.resource::<SnapshotView>();
    assert_eq!(view.bounds.bounds(), initial_bounds);

    for (tile, &amount) in tiles.into_iter().zip(next.field.resources()) {
        let sprite = world.get::<Sprite>(tile).unwrap();
        assert_eq!(sprite.color, heatmap_color(view.bounds.normalize(amount)));
    }

    let mut reseeded = next;
    reseeded.field_seed = 24;
    reseeded.field.randomize(reseeded.field_seed, 10.0, 200.0);
    let mut expected = BoundsNormalizer::default();
    expected.update_bounds(&reseeded.field, 0.0);
    publisher.publish(reseeded);
    app.update();

    let world = app.world_mut();
    let (min, max) = expected.bounds().unwrap();

    assert_eq!(
        world.resource::<SnapshotView>().bounds.bounds(),
        Some((min, max))
    );
    assert!(ui_text(world, UiText::LegendMinimum).contains(&format!("{min:.1}")));
    assert!(ui_text(world, UiText::LegendMaximum).contains(&format!("{max:.1}")));
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
    let vertex = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap();
    let run = SimulationRun::single_vertex_from_seed(config, 42, vertex, LN_2 / 2.0).unwrap();
    let mut snapshot = run.snapshot();
    snapshot
        .field
        .resources_mut()
        .copy_from_slice(&[20.0, 35.0, 80.0]);
    let bounds = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });
    let (mut app, mut publisher) = headless_viewer(snapshot.clone(), bounds);

    snapshot.tick = 2;
    snapshot.simulation_time = LN_2;
    snapshot
        .field
        .resources_mut()
        .copy_from_slice(&[40.0, 45.0, 60.0]);
    publisher.publish(snapshot.clone());
    app.update();

    let world = app.world_mut();
    let middle = world.resource::<Heatmap>().tiles[1];

    assert_eq!(
        world.resource::<SnapshotView>().bounds.bounds(),
        Some((30.0, 70.0))
    );
    assert_eq!(
        world.get::<Sprite>(middle).unwrap().color,
        heatmap_color(0.375)
    );
    assert!(ui_text(world, UiText::LegendMinimum).contains("30.0"));
    assert!(ui_text(world, UiText::LegendMaximum).contains("70.0"));

    snapshot.field_seed = 43;
    publisher.publish(snapshot);
    app.update();

    let world = app.world_mut();
    assert_eq!(
        world.resource::<SnapshotView>().bounds.bounds(),
        Some((40.0, 60.0))
    );
    assert!(ui_text(world, UiText::LegendMinimum).contains("40.0"));
    assert!(ui_text(world, UiText::LegendMaximum).contains("60.0"));
}

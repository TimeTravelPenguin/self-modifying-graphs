use bevy::camera::CameraProjection;

use super::*;

fn unstarted_app(layout: GridLayout) -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .insert_resource(layout)
        .add_plugins(RenderingPlugin);

    app
}

fn test_app(layout: GridLayout) -> App {
    let mut app = unstarted_app(layout);
    app.update();

    app
}

fn assert_edge_connects(world: &World, entity: Entity, source: Vec2, target: Vec2) {
    let transform = world.get::<Transform>(entity).unwrap();
    let size = world.get::<Sprite>(entity).unwrap().custom_size.unwrap();
    let half_edge = transform.rotation * Vec3::new(size.x / 2.0, 0.0, 0.0);

    assert!(
        (transform.translation - half_edge)
            .truncate()
            .distance(source)
            < 0.0001
    );
    assert!(
        (transform.translation + half_edge)
            .truncate()
            .distance(target)
            < 0.0001
    );
}

#[test]
fn observers_render_new_vertices_and_edges_before_startup() {
    let layout = GridLayout::new(3, 3, 50.0, 44.0, 8.0);
    let mut app = unstarted_app(layout);
    let source_coord = GridCoord::new(0, 0);
    let target_coord = GridCoord::new(2, 1);
    let source = app.world_mut().spawn(vertex(0, 0)).id();
    let target = app.world_mut().spawn((VertexVisual, target_coord)).id();
    let connection = app.world_mut().spawn(EdgeVisual::new(source, target)).id();
    app.world_mut().flush();

    for (entity, coord) in [(source, source_coord), (target, target_coord)] {
        assert!(app.world().get::<Sprite>(entity).is_some());
        assert_eq!(
            app.world()
                .get::<Transform>(entity)
                .unwrap()
                .translation
                .truncate(),
            layout.world_position(coord)
        );
    }

    assert_edge_connects(
        app.world(),
        connection,
        layout.world_position(source_coord),
        layout.world_position(target_coord),
    );
}

#[test]
fn adding_coordinates_to_an_existing_vertex_creates_its_visual() {
    let layout = GridLayout::new(3, 3, 50.0, 44.0, 8.0);
    let mut app = unstarted_app(layout);
    let entity = app.world_mut().spawn(VertexVisual).id();
    app.world_mut().flush();

    assert!(app.world().get::<Sprite>(entity).is_none());

    let coord = GridCoord::new(2, 0);
    app.world_mut().entity_mut(entity).insert(coord);
    app.world_mut().flush();

    assert!(app.world().get::<Sprite>(entity).is_some());
    assert_eq!(
        app.world()
            .get::<Transform>(entity)
            .unwrap()
            .translation
            .truncate(),
        layout.world_position(coord)
    );
}

#[test]
fn tile_fills_are_independent_while_the_image_is_shared() {
    let mut app = test_app(GridLayout::new(3, 3, 50.0, 44.0, 8.0));
    let tiles = app.world().resource::<GridTiles>();
    let center = tiles.get(GridCoord::new(1, 1)).unwrap();
    let neighbour = tiles.get(GridCoord::new(2, 1)).unwrap();
    let original_fill = app.world().get::<Sprite>(neighbour).unwrap().color;
    let custom_fill = app.world().resource::<RenderPalette>().selection;

    app.world_mut()
        .entity_mut(center)
        .insert(TileFill::Custom(custom_fill));
    app.update();

    let center_sprite = app.world().get::<Sprite>(center).unwrap();
    let neighbour_sprite = app.world().get::<Sprite>(neighbour).unwrap();

    assert_eq!(center_sprite.color, custom_fill);
    assert_eq!(neighbour_sprite.color, original_fill);
    assert_eq!(center_sprite.image, neighbour_sprite.image);
}

#[test]
fn layout_changes_preserve_existing_tiles_and_align_vertices() {
    let mut app = test_app(GridLayout::new(3, 3, 50.0, 44.0, 8.0));
    let coord = GridCoord::new(1, 1);
    let tile = app.world().resource::<GridTiles>().get(coord).unwrap();
    let removed_tile = app
        .world()
        .resource::<GridTiles>()
        .get(GridCoord::new(2, 2))
        .unwrap();
    let vertex = app
        .world_mut()
        .spawn((vertex(coord.column, coord.row), Name::new("example vertex")))
        .id();

    app.world_mut()
        .entity_mut(tile)
        .insert(TileFill::Resource(0.75));
    app.update();

    let changed_layout = GridLayout::new(4, 2, 64.0, 48.0, 9.0);
    app.world_mut().insert_resource(changed_layout);
    app.update();

    let tiles = app.world().resource::<GridTiles>();
    let tile_transform = app.world().get::<Transform>(tile).unwrap();
    let vertex_transform = app.world().get::<Transform>(vertex).unwrap();

    assert_eq!(tiles.get(coord), Some(tile));
    assert_eq!(
        app.world().get::<Name>(vertex).unwrap().as_str(),
        "example vertex"
    );
    assert_eq!(tiles.entities.len(), 8);
    assert!(app.world().get_entity(removed_tile).is_err());
    assert_eq!(
        tile_transform.translation.truncate(),
        changed_layout.world_position(coord)
    );
    assert_eq!(
        vertex_transform.translation.truncate(),
        tile_transform.translation.truncate()
    );
    assert_eq!(
        app.world().get::<Sprite>(tile).unwrap().custom_size,
        Some(Vec2::splat(48.0))
    );
    assert!(matches!(
        app.world().get::<TileFill>(tile),
        Some(TileFill::Resource(0.75))
    ));
}

#[test]
fn vertex_death_removes_its_edges_and_retains_its_tile() {
    let mut app = test_app(GridLayout::new(3, 3, 50.0, 44.0, 8.0));
    let source_coord = GridCoord::new(1, 1);
    let tile = app
        .world()
        .resource::<GridTiles>()
        .get(source_coord)
        .unwrap();
    let source = app
        .world_mut()
        .spawn(vertex(source_coord.column, source_coord.row))
        .id();
    let target = app.world_mut().spawn(vertex(2, 2)).id();
    let edge = app.world_mut().spawn(edge(source, target)).id();
    app.update();

    assert!(app.world().get::<Sprite>(edge).is_some());

    app.world_mut().despawn(source);
    app.world_mut().flush();

    assert!(app.world().get_entity(edge).is_err());
    assert!(app.world().get::<BackgroundGridTile>(tile).is_some());
    assert!(app.world().get::<Sprite>(target).is_some());
}

#[test]
fn removing_graph_markers_cleans_visuals_and_incident_edges() {
    let mut app = test_app(GridLayout::new(3, 3, 50.0, 44.0, 8.0));
    let source = app.world_mut().spawn(vertex(0, 0)).id();
    let target = app.world_mut().spawn(vertex(1, 1)).id();
    let connection = app.world_mut().spawn(edge(source, target)).id();
    app.world_mut().flush();

    app.world_mut()
        .entity_mut(connection)
        .remove::<EdgeVisual>();
    app.world_mut().flush();

    assert!(app.world().get_entity(connection).is_ok());
    assert!(app.world().get::<Sprite>(connection).is_none());

    app.world_mut()
        .entity_mut(connection)
        .insert(EdgeVisual::new(source, target));
    app.world_mut().flush();

    assert!(app.world().get::<Sprite>(connection).is_some());

    app.world_mut().entity_mut(source).remove::<VertexVisual>();
    app.world_mut().flush();

    assert!(app.world().get_entity(source).is_ok());
    assert!(app.world().get::<Sprite>(source).is_none());
    assert!(app.world().get_entity(connection).is_err());
    assert!(app.world().get::<Sprite>(target).is_some());
}

#[test]
fn removing_a_vertex_coordinate_removes_its_visual_and_edges() {
    let mut app = test_app(GridLayout::new(3, 3, 50.0, 44.0, 8.0));
    let source = app.world_mut().spawn(vertex(0, 0)).id();
    let target = app.world_mut().spawn(vertex(1, 1)).id();
    let connection = app.world_mut().spawn(edge(source, target)).id();
    app.world_mut().flush();

    app.world_mut().entity_mut(source).remove::<GridCoord>();
    app.world_mut().flush();

    assert!(app.world().get::<VertexVisual>(source).is_some());
    assert!(app.world().get::<Sprite>(source).is_none());
    assert!(app.world().get_entity(connection).is_err());
}

#[test]
fn coordinate_mutation_and_palette_changes_refresh_existing_graph_visuals() {
    let layout = GridLayout::new(5, 5, 50.0, 44.0, 8.0);
    let mut app = test_app(layout);
    let source_coord = GridCoord::new(1, 1);
    let source = app.world_mut().spawn(vertex(1, 1)).id();
    let target = app.world_mut().spawn(vertex(3, 3)).id();
    let connection = app.world_mut().spawn(edge(source, target)).id();
    app.world_mut().flush();

    let original_image = app.world().get::<Sprite>(target).unwrap().image.clone();
    let target_coord = GridCoord::new(4, 1);
    *app.world_mut().get_mut::<GridCoord>(target).unwrap() = target_coord;

    let (vertex_color, edge_color) = {
        let mut palette = app.world_mut().resource_mut::<RenderPalette>();
        palette.vertex = palette.selection;
        palette.edge = palette.resource_high;

        (palette.vertex, palette.edge)
    };

    app.update();

    let target_sprite = app.world().get::<Sprite>(target).unwrap();

    assert_eq!(target_sprite.color, vertex_color);
    assert_eq!(target_sprite.image, original_image);
    assert_eq!(
        app.world().get::<Sprite>(connection).unwrap().color,
        edge_color
    );
    assert_eq!(
        app.world()
            .get::<Transform>(target)
            .unwrap()
            .translation
            .truncate(),
        layout.world_position(target_coord)
    );
    assert_edge_connects(
        app.world(),
        connection,
        layout.world_position(source_coord),
        layout.world_position(target_coord),
    );
}

#[test]
fn camera_keeps_the_grid_centered_and_visible_across_aspect_ratios() {
    let layout = GridLayout::default();
    let padded_size = layout.bounds_size() + Vec2::splat(CAMERA_PADDING * 2.0);
    let Projection::Orthographic(mut projection) = grid_projection(&layout) else {
        panic!("grid camera must be orthographic");
    };

    for (width, height) in [(1280.0, 720.0), (720.0, 1280.0), (320.0, 240.0)] {
        projection.update(width, height);

        assert!(projection.area.center().length() < 0.001);
        assert!(projection.area.width() + 0.001 >= padded_size.x);
        assert!(projection.area.height() + 0.001 >= padded_size.y);
    }
}

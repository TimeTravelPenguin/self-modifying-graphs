use std::thread;

use simulation::{
    field::Neighbourhood,
    graph::Uptake,
    simulation::{SimulationConfig, SimulationRun, SnapshotPublisher},
};

#[test]
fn reader_keeps_initial_snapshot_while_worker_publishes_completed_ticks() {
    let config = SimulationConfig {
        width: 9,
        height: 7,
        spacing: 1.0,
        initial_resource: 100.0,
        resource_noise_scale: 10.0,
        diffusion_coefficient: 0.0,
        neighbourhood: Neighbourhood::Moore,
    };

    let seed = 42;
    let dt = 0.125;
    let uptake = Uptake::new(5.0, 1.0).unwrap();
    let mut run = SimulationRun::single_vertex_from_seed(config, seed, uptake, dt).unwrap();
    let initial_field = run.view().field.clone();
    let mut publisher = SnapshotPublisher::new(run.snapshot());
    let reader = publisher.reader();
    let initial = reader.latest();

    assert_eq!(initial.seed, seed);
    assert_eq!(initial.tick, 0);
    assert_eq!(initial.simulation_time, 0.0);
    assert_eq!((initial.field.width(), initial.field.height()), (9, 7));
    assert_eq!(initial.vertices.len(), 1);

    let initial_vertex = &initial.vertices[0];
    let initial_available = initial_field
        .get_resource_at(initial_vertex.position)
        .unwrap();

    thread::spawn(move || {
        for _ in 0..4 {
            run.advance();
            publisher.publish(run.snapshot());
        }
    })
    .join()
    .unwrap();

    let latest = reader.latest();
    assert_eq!(latest.seed, seed);
    assert_eq!(latest.tick, 4);
    assert_eq!(latest.simulation_time, 4.0 * dt);
    assert_eq!((latest.field.width(), latest.field.height()), (9, 7));
    assert_eq!(latest.vertices.len(), 1);

    let latest_vertex = &latest.vertices[0];
    assert_eq!(latest_vertex.id, initial_vertex.id);
    assert_eq!(latest_vertex.position, initial_vertex.position);
    assert_eq!(latest_vertex.stored_resource, 2.5);
    assert_eq!(
        latest.field.get_resource_at(latest_vertex.position),
        Some(initial_available - latest_vertex.stored_resource)
    );

    assert_eq!(initial.tick, 0);
    assert_eq!(initial.simulation_time, 0.0);
    assert_eq!(initial.field, initial_field);
    assert_eq!(initial_vertex.stored_resource, 0.0);
}

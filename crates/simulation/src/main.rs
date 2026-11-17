use simulation::{
    field::Neighbourhood,
    graph::Uptake,
    simulation::{
        SimulationConfig, SimulationError, SimulationRun, SimulationSnapshot, SnapshotPublisher,
    },
};

fn main() -> Result<(), SimulationError> {
    let config = SimulationConfig {
        width: 10,
        height: 10,
        spacing: 1.0,
        initial_resource: 100.0,
        resource_noise_scale: 10.0,
        diffusion_coefficient: 0.8,
        neighbourhood: Neighbourhood::Moore,
    };

    let seed = rand::random::<u32>();
    let uptake = Uptake::new(10.0, 0.75).expect("invalid uptake parameters");
    let mut run = SimulationRun::single_vertex_from_seed(config, seed, uptake, 0.1)?;
    let mut publisher = SnapshotPublisher::new(run.snapshot());
    let reader = publisher.reader();

    let initial = reader.latest();
    println!("Seed {}", initial.seed);
    print_snapshot(&initial);

    for _ in 0..10 {
        run.advance();
        publisher.publish(run.snapshot());
        print_snapshot(&reader.latest());
    }

    Ok(())
}

fn print_snapshot(snapshot: &SimulationSnapshot) {
    let vertex = snapshot.vertices.first().expect("starting vertex exists");
    let available = snapshot
        .field
        .get_resource_at(vertex.position)
        .expect("vertex position is in the field");

    println!(
        "Tick {} at time {:.1} | vertex {:?} at ({}, {}) | cell {:.3} | stored {:.3}",
        snapshot.tick,
        snapshot.simulation_time,
        vertex.id,
        vertex.position.x,
        vertex.position.y,
        available,
        vertex.stored_resource,
    );
}

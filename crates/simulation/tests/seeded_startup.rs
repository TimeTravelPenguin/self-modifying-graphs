use simulation::{
    field::Neighbourhood,
    graph::{HealthConfig, Uptake, VertexConfig},
    simulation::{Simulation, SimulationConfig, SimulationError},
};

fn config(width: u32, height: u32) -> SimulationConfig {
    SimulationConfig {
        width,
        height,
        spacing: 1.0,
        resource_noise_scale: 10.0,
        initial_resource: 100.0,
        diffusion_coefficient: 0.8,
        neighbourhood: Neighbourhood::Moore,
    }
}

#[test]
fn seed_reproduces_the_field_and_single_vertex() {
    let seed = 42;
    let vertex_config =
        VertexConfig::new(Uptake::new(5.0, 1.0).unwrap(), 0.0, HealthConfig::Disabled).unwrap();
    let empty = Simulation::from_seed(config(9, 7), seed);
    let mut simulation =
        Simulation::single_vertex_from_seed(config(9, 7), seed, vertex_config.clone()).unwrap();
    let replay =
        Simulation::single_vertex_from_seed(config(9, 7), seed, vertex_config.clone()).unwrap();

    let field = simulation.field();
    assert_eq!((field.width(), field.height()), (9, 7));
    assert_eq!(field, empty.field());
    assert_eq!(field, replay.field());
    assert!(field.resources().windows(2).any(|pair| pair[0] != pair[1]));
    assert_eq!(empty.graph().vertices().count(), 0);

    assert_eq!(simulation.graph().vertices().count(), 1);
    assert_eq!(simulation.graph().edges().count(), 0);
    let (vertex_id, vertex) = simulation.graph().vertices().next().unwrap();
    let (_, replay_vertex) = replay.graph().vertices().next().unwrap();
    assert_eq!(vertex.position(), replay_vertex.position());
    assert_eq!(simulation.vertex_at(vertex.position()), Some(vertex_id));

    let initial_field_total = field.resources().iter().sum::<f64>();

    for _ in 0..3 {
        simulation.step(0.1);
    }

    let final_field_total = simulation.field().resources().iter().sum::<f64>();
    let stored = simulation
        .graph()
        .vertex(vertex_id)
        .unwrap()
        .stored_resource();
    assert!((stored - 1.5).abs() < 1e-12);
    assert!((initial_field_total - final_field_total - stored).abs() < 1e-9);
}

#[test]
fn single_vertex_startup_rejects_either_empty_dimension() {
    let vertex_config =
        VertexConfig::new(Uptake::new(5.0, 1.0).unwrap(), 0.0, HealthConfig::Disabled).unwrap();

    for (width, height) in [(0, 7), (9, 0)] {
        assert!(matches!(
            Simulation::single_vertex_from_seed(config(width, height), 42, vertex_config.clone()),
            Err(SimulationError::EmptyGrid)
        ));
    }
}

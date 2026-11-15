use simulation::{
    field::Neighbourhood,
    graph::Uptake,
    simulation::{Simulation, SimulationConfig, SimulationError},
};

const SEED: u32 = 42;

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

    let mut simulation = Simulation::from_seed(config, SEED);

    let v1 = simulation.add_vertex(
        (5, 5),
        Uptake::new(10.0, 0.75).expect("Invalid uptake parameters"),
    )?;
    let v2 = simulation.add_vertex(
        (5, 6),
        Uptake::new(5.0, 0.5).expect("Invalid uptake parameters"),
    )?;
    let edge = simulation.add_edge(v1, v2)?;

    for _ in 0..10 {
        let res = simulation
            .field()
            .get_resource_at((5, 5))
            .expect("Position should be valid");
        println!("Resource at (5, 5): {}", res);

        let vert = simulation.graph().vertex(v1).expect("Vertex should exist");
        println!("{:?}", vert);

        let vert = simulation.graph().vertex(v2).expect("Vertex should exist");
        println!("{:?}", vert);

        let edge = simulation.graph().edge(edge).expect("Edge should exist");
        println!("{:?}\n", edge);

        simulation.step(0.1);
    }

    Ok(())
}

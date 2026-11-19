use simulation::{
    field::{Diffuser, Neighbourhood, ResourceField},
    graph::{HealthConfig, Uptake, Vertex, VertexConfig, VertexId},
    simulation::{Simulation, SimulationConfig, SimulationRun},
};

fn simulation_with_amounts(amounts: &[f64]) -> Simulation {
    let width = u32::try_from(amounts.len()).unwrap();
    let mut field = ResourceField::new_random(1, width, 0, 1.0, 0.0);
    field.resources_mut().copy_from_slice(amounts);

    Simulation::new(field, Diffuser::new(Neighbourhood::VonNeumann), 0.0, 1.0)
}

fn vertex(simulation: &Simulation, id: VertexId) -> &Vertex {
    simulation.graph().vertex(id).unwrap()
}

#[test]
fn vertices_pay_their_own_cost_from_stored_resources_after_uptake() {
    let mut simulation = simulation_with_amounts(&[8.0, 8.0, 8.0]);
    let uptake = Uptake::new(4.0, 0.5).unwrap();
    let low_cost = VertexConfig::new(uptake, 1.0, HealthConfig::default()).unwrap();
    let high_cost = VertexConfig::new(uptake, 3.0, HealthConfig::default()).unwrap();
    let free = simulation
        .add_vertex(
            (0, 0),
            VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap(),
        )
        .unwrap();
    let low = simulation.add_vertex((1, 0), low_cost).unwrap();
    let high = simulation.add_vertex((2, 0), high_cost).unwrap();

    simulation.step(1.0);

    assert_eq!(simulation.field().resources(), &[4.0, 4.0, 4.0]);
    assert_eq!(vertex(&simulation, free).living_cost(), 0.0);
    assert_eq!(vertex(&simulation, low).living_cost(), 1.0);
    assert_eq!(vertex(&simulation, high).living_cost(), 3.0);
    assert_eq!(vertex(&simulation, free).stored_resource(), 2.0);
    assert_eq!(vertex(&simulation, low).stored_resource(), 1.0);
    assert_eq!(vertex(&simulation, high).stored_resource(), 0.0);

    simulation.step(0.0);

    assert_eq!(simulation.field().resources(), &[4.0, 4.0, 4.0]);
    assert_eq!(vertex(&simulation, free).stored_resource(), 2.0);
    assert_eq!(vertex(&simulation, low).stored_resource(), 0.0);
    assert_eq!(vertex(&simulation, high).stored_resource(), 0.0);
}

#[test]
fn a_vertex_can_recover_after_it_cannot_pay_the_full_cost() {
    let mut simulation = simulation_with_amounts(&[10.0]);
    let uptake = Uptake::new(4.0, 0.5).unwrap();
    let config = VertexConfig::new(uptake, 1.0, HealthConfig::default()).unwrap();
    let id = simulation.add_vertex((0, 0), config).unwrap();

    simulation.step(0.25);

    assert_eq!(simulation.vertex_at((0, 0)), Some(id));
    assert_eq!(vertex(&simulation, id).stored_resource(), 0.0);

    simulation.step(1.0);

    assert_eq!(simulation.vertex_at((0, 0)), Some(id));
    assert_eq!(vertex(&simulation, id).stored_resource(), 1.0);
}

#[test]
fn living_cost_must_be_finite_and_nonnegative() {
    let uptake = Uptake::new(4.0, 0.5).unwrap();

    for amount in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(VertexConfig::new(uptake, amount, HealthConfig::default()).is_err());
    }

    assert!(VertexConfig::new(uptake, 0.0, HealthConfig::default()).is_ok());
}

#[test]
fn seeded_runs_apply_the_configured_cost() {
    let config = SimulationConfig {
        width: 1,
        height: 1,
        spacing: 1.0,
        initial_resource: 20.0,
        resource_noise_scale: 1.0,
        diffusion_coefficient: 0.0,
        neighbourhood: Neighbourhood::VonNeumann,
    };

    let uptake = Uptake::new(4.0, 1.0).unwrap();
    let vertex = VertexConfig::new(uptake, 1.5, HealthConfig::default()).unwrap();
    let mut run = SimulationRun::single_vertex_from_seed(config, 42, vertex, 0.5).unwrap();

    run.advance();

    let snapshot = run.snapshot();
    assert_eq!(snapshot.tick, 1);
    assert_eq!(snapshot.vertices[0].stored_resource, 0.5);
}

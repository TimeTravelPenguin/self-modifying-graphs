use simulation::{
    field::{DiffusionError, Neighbourhood},
    graph::Uptake,
    simulation::{SimulationConfig, SimulationError, SimulationRun},
};

fn config() -> SimulationConfig {
    SimulationConfig {
        width: 9,
        height: 7,
        spacing: 1.0,
        initial_resource: 100.0,
        resource_noise_scale: 10.0,
        diffusion_coefficient: 0.8,
        neighbourhood: Neighbourhood::Moore,
    }
}

#[test]
fn fixed_step_run_replays_completed_state() {
    let seed = 42;
    let dt = 0.125;
    let uptake = Uptake::new(5.0, 1.0).unwrap();
    let mut run = SimulationRun::single_vertex_from_seed(config(), seed, uptake, dt).unwrap();
    let mut replay = SimulationRun::single_vertex_from_seed(config(), seed, uptake, dt).unwrap();

    for view in [run.view(), replay.view()] {
        assert_eq!(view.seed, seed);
        assert_eq!(view.tick, 0);
        assert_eq!(view.simulation_time, 0.0);
        assert_eq!((view.field.width(), view.field.height()), (9, 7));
        assert_eq!(view.graph.vertices().count(), 1);
        assert_eq!(view.graph.edges().count(), 0);
    }

    assert_eq!(run.view().field, replay.view().field);

    for _ in 0..10 {
        run.advance();
        replay.advance();
    }

    let view = run.view();
    let replay_view = replay.view();
    assert_eq!((view.tick, replay_view.tick), (10, 10));
    assert_eq!(view.simulation_time, 10.0 * dt);
    assert_eq!(view.simulation_time, replay_view.simulation_time);
    assert_eq!(view.field, replay_view.field);

    let (_, vertex) = view.graph.vertices().next().unwrap();
    let (_, replay_vertex) = replay_view.graph.vertices().next().unwrap();
    assert_eq!(vertex.position(), replay_vertex.position());
    assert_eq!(vertex.stored_resource(), replay_vertex.stored_resource());
    assert!(vertex.stored_resource() > 0.0);
}

#[test]
fn run_rejects_invalid_timesteps_at_startup() {
    let uptake = Uptake::new(5.0, 1.0).unwrap();

    for dt in [0.0, -0.1, f64::INFINITY, f64::NAN] {
        assert!(matches!(
            SimulationRun::single_vertex_from_seed(config(), 42, uptake, dt),
            Err(SimulationError::Diffusion(DiffusionError::InvalidTimestep(rejected)))
                if rejected.to_bits() == dt.to_bits()
        ));
    }

    assert!(matches!(
        SimulationRun::single_vertex_from_seed(config(), 42, uptake, 1.0),
        Err(SimulationError::Diffusion(DiffusionError::UnstableTimestep {
            diffusion_number,
            limit,
        })) if diffusion_number == 0.8 && limit == 0.30
    ));
}

use simulation::{
    effects::{Effect, EffectInput, EffectTarget, EffectsConfig, ResponseCurve},
    field::{Diffuser, Neighbourhood, ResourceField},
    graph::{HealthConfig, Uptake, VertexConfig},
    simulation::{Simulation, SimulationConfig, SimulationError, SimulationRun},
};

fn model(amounts: &[f64]) -> Simulation {
    let mut field = ResourceField::new_random(1, amounts.len() as u32, 0, 1.0, 0.0);
    field.resources_mut().copy_from_slice(amounts);

    Simulation::new(field, Diffuser::new(Neighbourhood::VonNeumann), 0.0, 1.0)
}

fn capacity(multiplier: f64) -> EffectsConfig {
    EffectsConfig::new([Effect {
        input: EffectInput::Age,
        target: EffectTarget::UptakeCapacity,
        response: ResponseCurve::Constant { multiplier },
    }])
    .unwrap()
}

#[test]
fn partial_payment_damages_health_and_exact_payment_recovers_it() {
    let config = VertexConfig::new(
        Uptake::new(2.0, 1.0).unwrap(),
        1.0,
        HealthConfig::Enabled {
            max_health: 2.0,
            damage_per_unpaid_tick: 1.0,
            recovery_per_paid_tick: 0.25,
        },
    )
    .unwrap();
    let mut simulation = model(&[10.0]);
    let id = simulation.add_vertex((0, 0), config).unwrap();

    simulation.step(0.25);
    let vertex = simulation.graph().vertex(id).unwrap();
    assert_eq!(vertex.stored_resource(), 0.0);
    assert_eq!(vertex.health(), Some(1.5));
    assert_eq!(vertex.age(), 0.25);

    simulation.step(1.0);
    let vertex = simulation.graph().vertex(id).unwrap();
    assert_eq!(vertex.stored_resource(), 1.0);
    assert_eq!(vertex.health(), Some(1.75));

    simulation.step(0.0);
    let vertex = simulation.graph().vertex(id).unwrap();
    assert_eq!(vertex.stored_resource(), 0.0);
    assert_eq!(vertex.health(), Some(2.0));
    assert_eq!(vertex.age(), 1.25);
}

#[test]
fn extinction_clears_cells_incident_edges_and_stale_ids() {
    let mortal = VertexConfig::new(
        Uptake::new(0.0, 1.0).unwrap(),
        1.0,
        HealthConfig::Enabled {
            max_health: 1.0,
            damage_per_unpaid_tick: 1.0,
            recovery_per_paid_tick: 0.0,
        },
    )
    .unwrap();
    let mut simulation = model(&[0.0, 0.0]);
    let left = simulation.add_vertex((0, 0), mortal.clone()).unwrap();
    let right = simulation.add_vertex((1, 0), mortal.clone()).unwrap();
    simulation.add_edge(left, right).unwrap();
    simulation.add_edge(left, left).unwrap();

    simulation.step(0.0);

    assert_eq!(simulation.graph().vertices().count(), 0);
    assert_eq!(simulation.graph().edges().count(), 0);
    assert_eq!(simulation.vertex_at((0, 0)), None);
    assert_eq!(simulation.vertex_at((1, 0)), None);
    let replacement = simulation.add_vertex((0, 0), mortal).unwrap();
    assert_ne!(replacement, left);
    assert!(matches!(
        simulation.set_vertex_effects(left, capacity(0.0)),
        Err(SimulationError::UnknownVertex(id)) if id == left
    ));

    simulation.step(1.0);
    assert_eq!(simulation.graph().vertices().count(), 0);
}

#[test]
fn recovery_uses_beginning_of_tick_age_and_snapshot_previews_the_next_tick() {
    let config = SimulationConfig {
        width: 1,
        height: 1,
        spacing: 1.0,
        initial_resource: 100.0,
        resource_noise_scale: 1.0,
        diffusion_coefficient: 0.0,
        neighbourhood: Neighbourhood::VonNeumann,
    };
    let vertex = VertexConfig::new(Uptake::new(2.0, 1.0).unwrap(), 1.0, HealthConfig::default())
        .unwrap()
        .with_effects(capacity(0.0))
        .unwrap();
    let mut run = SimulationRun::single_vertex_from_seed(config, 42, vertex, 1.0).unwrap();

    run.advance();
    let initial = run.snapshot();
    let id = initial.vertices[0].id;
    assert_eq!(initial.vertices[0].health, Some(9.0));
    let recovery = EffectsConfig::new([Effect {
        input: EffectInput::Age,
        target: EffectTarget::RecoveryAmount,
        response: ResponseCurve::ExponentialDecay {
            half_life: 1.0,
            minimum: 0.0,
        },
    }])
    .unwrap();
    assert_eq!(run.set_vertex_effects(id, recovery).unwrap(), 1);

    run.advance();
    let next = run.snapshot();
    let vertex = &next.vertices[0];
    assert_eq!(vertex.age, 2.0);
    assert_eq!(vertex.health, Some(9.125));
    assert_eq!(vertex.effective_recovery, Some(0.0625));
    assert_eq!(vertex.effects_revision, 1);
    assert_eq!(initial.vertices[0].effects_revision, 0);
    assert_eq!(initial.vertices[0].health, Some(9.0));
    assert_eq!(initial.vertices[0].config.effects().effects().len(), 1);
}

#[test]
fn shared_effects_evaluate_independently_for_old_and_young_vertices() {
    let effects = EffectsConfig::new([Effect {
        input: EffectInput::Age,
        target: EffectTarget::UptakeCapacity,
        response: ResponseCurve::ExponentialDecay {
            half_life: 1.0,
            minimum: 0.0,
        },
    }])
    .unwrap();
    let config = VertexConfig::new(Uptake::new(4.0, 1.0).unwrap(), 0.0, HealthConfig::Disabled)
        .unwrap()
        .with_effects(effects)
        .unwrap();
    let mut simulation = model(&[100.0, 100.0]);
    let old = simulation.add_vertex((0, 0), config.clone()).unwrap();
    simulation.step(1.0);
    let young = simulation.add_vertex((1, 0), config).unwrap();

    let old_vertex = simulation.graph().vertex(old).unwrap();
    let young_vertex = simulation.graph().vertex(young).unwrap();
    assert!(std::ptr::eq(
        old_vertex.config().effects().effects().as_ptr(),
        young_vertex.config().effects().effects().as_ptr()
    ));
    assert_eq!(old_vertex.effective_effects().uptake_capacity, 0.5);
    assert_eq!(young_vertex.effective_effects().uptake_capacity, 1.0);

    simulation.step(1.0);

    assert_eq!(simulation.field().resources(), &[94.0, 96.0]);
    assert_eq!(simulation.graph().vertex(old).unwrap().health(), None);
    assert_eq!(simulation.graph().vertex(young).unwrap().age(), 1.0);
}

#[test]
fn rejected_amplification_preserves_configuration_revision_and_state() {
    let config = VertexConfig::new(
        Uptake::new(f64::MAX, 1.0).unwrap(),
        0.0,
        HealthConfig::default(),
    )
    .unwrap();
    let mut simulation = model(&[1.0]);
    let id = simulation.add_vertex((0, 0), config.clone()).unwrap();

    assert!(simulation.set_vertex_effects(id, capacity(2.0)).is_err());
    let vertex = simulation.graph().vertex(id).unwrap();
    assert_eq!(vertex.config(), &config);
    assert_eq!(vertex.effects_revision(), 0);
    assert_eq!(vertex.age(), 0.0);
    assert_eq!(vertex.health(), Some(10.0));

    simulation.set_vertex_effects(id, capacity(0.0)).unwrap();
    simulation.step(f64::MAX);
    let vertex = simulation.graph().vertex(id).unwrap();
    assert_eq!(vertex.stored_resource(), 0.0);
    assert_eq!(vertex.age(), f64::MAX);
    assert_eq!(simulation.field().resources(), &[1.0]);
}

#[test]
fn configured_seeded_runs_replay_health_age_and_effects() {
    fn run() -> SimulationRun {
        let config = SimulationConfig {
            width: 5,
            height: 3,
            spacing: 1.0,
            initial_resource: 100.0,
            resource_noise_scale: 2.0,
            diffusion_coefficient: 0.1,
            neighbourhood: Neighbourhood::Moore,
        };
        let vertex = VertexConfig::new(
            Uptake::saturating(2.0, 10.0, 0.75).unwrap(),
            1.0,
            HealthConfig::default(),
        )
        .unwrap()
        .with_effects(capacity(0.5))
        .unwrap();

        SimulationRun::single_vertex_from_seed(config, 42, vertex, 0.1).unwrap()
    }

    let mut first = run();
    let mut replay = run();

    for _ in 0..20 {
        first.advance();
        replay.advance();
        assert_eq!(first.snapshot(), replay.snapshot());
    }
}

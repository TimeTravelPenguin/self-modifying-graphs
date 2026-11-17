use simulation::{
    field::{Diffuser, Neighbourhood, ResourceField},
    graph::Uptake,
    simulation::Simulation,
};

fn simulation_with_amounts(amounts: &[f64]) -> Simulation {
    let width = u32::try_from(amounts.len()).unwrap();
    let mut field = ResourceField::new_random(1, width, 0, 1.0, 0.0);
    field.resources_mut().copy_from_slice(amounts);

    Simulation::new(field, Diffuser::new(Neighbourhood::VonNeumann), 0.0, 1.0)
}

#[test]
fn constructors_reject_invalid_rates_half_saturation_and_efficiency() {
    for rate in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Uptake::new(rate, 1.0).is_none());
        assert!(Uptake::saturating(rate, 10.0, 1.0).is_none());
    }

    for half_saturation in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(Uptake::saturating(8.0, half_saturation, 1.0).is_none());
    }

    for efficiency in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(Uptake::new(8.0, efficiency).is_none());
        assert!(Uptake::saturating(8.0, 10.0, efficiency).is_none());
    }

    assert!(Uptake::new(0.0, 0.0).is_some());
    assert!(Uptake::saturating(0.0, 10.0, 1.0).is_some());
}

#[test]
fn constant_and_saturating_vertices_use_the_same_resource_accounting() {
    let efficiency = 0.75;
    let saturating = Uptake::saturating(8.0, 10.0, efficiency).unwrap();
    let capped = Uptake::saturating(100.0, 10.0, efficiency).unwrap();
    let constant = Uptake::new(6.0, efficiency).unwrap();
    let amounts = [0.0, 2.0, 10.0, 150.0, 0.5, 10.0];
    let mut simulation = simulation_with_amounts(&amounts);
    let vertices: Vec<_> = [
        saturating, saturating, saturating, saturating, capped, constant,
    ]
    .into_iter()
    .enumerate()
    .map(|(x, uptake)| simulation.add_vertex((x, 0), uptake).unwrap())
    .collect();

    simulation.step(0.0);
    assert_eq!(simulation.field().resources(), &amounts);
    assert!(
        simulation
            .graph()
            .vertices()
            .all(|(_, vertex)| vertex.stored_resource() == 0.0)
    );

    simulation.step(0.5);

    let expected_withdrawals = [0.0, 2.0 / 3.0, 2.0, 3.75, 0.5, 3.0];

    for (x, (vertex_id, expected_withdrawal)) in
        vertices.into_iter().zip(expected_withdrawals).enumerate()
    {
        let remaining = simulation.field().get_resource_at((x, 0)).unwrap();
        let stored = simulation
            .graph()
            .vertex(vertex_id)
            .unwrap()
            .stored_resource();

        assert!((remaining - (amounts[x] - expected_withdrawal)).abs() < 1e-12);
        assert!((stored - efficiency * expected_withdrawal).abs() < 1e-12);
    }
}

#[test]
fn saturating_uptake_handles_a_sum_that_would_overflow() {
    let mut simulation = simulation_with_amounts(&[f64::MAX]);
    let uptake = Uptake::saturating(f64::MAX, f64::MAX, 1.0).unwrap();
    let vertex_id = simulation.add_vertex((0, 0), uptake).unwrap();

    simulation.step(1.0);

    let remaining = simulation.field().get_resource_at((0, 0)).unwrap();
    let stored = simulation
        .graph()
        .vertex(vertex_id)
        .unwrap()
        .stored_resource();
    assert_eq!(remaining, f64::MAX / 2.0);
    assert_eq!(stored, f64::MAX / 2.0);
}

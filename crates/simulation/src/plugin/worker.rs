//! Wall-clock pacing, commands, and shutdown for the simulation worker.

use std::{
    io,
    sync::{
        Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use bevy::prelude::Resource;
use thiserror::Error;

use crate::{
    effects::EffectsConfig,
    graph::VertexId,
    simulation::{SimulationError, SimulationRun, SnapshotPublisher, SnapshotReader},
};

enum SimulationCommand {
    ToggleNeighbourhood,
    ReseedField(u32),
    SetVertexEffects(EffectUpdateRequest),
    TickBoundary,
    Stop,
}

struct EffectUpdateRequest {
    vertex_id: VertexId,
    effects: EffectsConfig,
    reply: Sender<EffectUpdateOutcome>,
}

struct AppliedEffectUpdate {
    vertex_id: VertexId,
    revision: u64,
    reply: Sender<EffectUpdateOutcome>,
}

/// Failure to reach the worker or receive its response.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SimulationControlError {
    /// The worker stopped before accepting or answering a request.
    #[error("simulation worker has stopped")]
    WorkerDisconnected,
}

/// Result of replacing one vertex's complete effect set.
#[derive(Debug)]
pub enum EffectUpdateOutcome {
    /// The new configuration was installed and its first tick was published.
    Applied {
        /// Vertex whose configuration was replaced.
        vertex_id: VertexId,
        /// Revision assigned to the accepted effect set.
        revision: u64,
        /// First published tick that used the new effect set.
        tick: u64,
    },
    /// The model rejected the replacement without modifying the vertex.
    Rejected(SimulationError),
    /// A newer request for the same vertex replaced this request before the tick.
    Superseded,
}

/// A request-specific response that remains readable after the viewer skips ticks.
#[derive(Debug)]
pub struct EffectUpdateTicket {
    receiver: Mutex<Receiver<EffectUpdateOutcome>>,
    outcome: Option<EffectUpdateOutcome>,
}

impl EffectUpdateTicket {
    /// Polls the response without waiting and retains a completed result.
    ///
    /// Returns `Ok(None)` while the replacement is pending. An applied result is
    /// available only after the corresponding completed snapshot was published.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationControlError::WorkerDisconnected`] if the worker stops
    /// before responding. Completed results remain available after shutdown.
    pub fn try_result(&mut self) -> Result<Option<&EffectUpdateOutcome>, SimulationControlError> {
        if self.outcome.is_none() {
            // The mutex supplies Sync for Bevy storage. Exclusive ticket access
            // lets polling use the receiver directly without acquiring a lock.
            let receiver = self
                .receiver
                .get_mut()
                .expect("ticket receiver is never locked");

            match receiver.try_recv() {
                Ok(outcome) => self.outcome = Some(outcome),
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => {
                    return Err(SimulationControlError::WorkerDisconnected);
                }
            }
        }

        Ok(self.outcome.as_ref())
    }
}

/// Sends commands from the Bevy app to the worker that owns the simulation.
#[derive(Resource, Clone)]
pub struct SimulationControls {
    sender: Sender<SimulationCommand>,
}

impl SimulationControls {
    /// Queues a switch between diffusion stencils if diffusion is enabled.
    ///
    /// Returns whether the command reached the queue. The worker checks that
    /// the requested stencil is stable for the current timestep.
    pub fn toggle_neighbourhood(&self) -> bool {
        self.sender
            .send(SimulationCommand::ToggleNeighbourhood)
            .is_ok()
    }

    /// Queues resource field regeneration with the given seed.
    ///
    /// Returns whether the command reached the queue. The field appears in the
    /// next completed snapshot.
    pub fn reseed_field(&self, seed: u32) -> bool {
        self.sender
            .send(SimulationCommand::ReseedField(seed))
            .is_ok()
    }

    /// Queues a complete effect-set replacement for the next tick boundary.
    ///
    /// The latest queued replacement for a vertex wins. Earlier requests for
    /// that vertex report [`EffectUpdateOutcome::Superseded`]. The worker checks
    /// the vertex ID and its base quantities before installing the replacement.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationControlError::WorkerDisconnected`] if the worker has
    /// stopped. Rejections by the model are delivered through the returned ticket.
    pub fn set_vertex_effects(
        &self,
        vertex_id: VertexId,
        effects: EffectsConfig,
    ) -> Result<EffectUpdateTicket, SimulationControlError> {
        let (reply, receiver) = mpsc::channel();
        let request = EffectUpdateRequest {
            vertex_id,
            effects,
            reply,
        };

        self.sender
            .send(SimulationCommand::SetVertexEffects(request))
            .map_err(|_| SimulationControlError::WorkerDisconnected)?;

        Ok(EffectUpdateTicket {
            receiver: Mutex::new(receiver),
            outcome: None,
        })
    }
}

/// Owns the stepping thread and joins it when dropped.
#[derive(Resource)]
pub(super) struct SimulationWorker {
    reader: SnapshotReader,
    controls: SimulationControls,
    thread: Option<JoinHandle<()>>,
}

impl SimulationWorker {
    /// Publishes the current tick and starts advancing on a wall-clock interval.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero interval or if the worker thread cannot start.
    pub(super) fn start(
        mut run: SimulationRun,
        interval: Duration,
        perlin_scale: f64,
        max_resource: f64,
        diffusion_enabled: bool,
    ) -> io::Result<Self> {
        if interval.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "simulation worker interval must be positive",
            ));
        }

        let mut publisher = SnapshotPublisher::new(run.snapshot());
        let reader = publisher.reader();
        let (sender, receiver) = mpsc::channel();
        let boundary_sender = sender.clone();
        let thread = thread::Builder::new()
            .name("simulation".to_owned())
            .spawn(move || {
                let mut tick_started = Instant::now();
                let mut pending = Vec::new();
                let mut applied = Vec::new();

                loop {
                    let time_to_tick = interval.saturating_sub(tick_started.elapsed());

                    match receiver.recv_timeout(time_to_tick) {
                        Ok(command) => {
                            if !handle_command(
                                command,
                                &mut run,
                                &mut pending,
                                perlin_scale,
                                max_resource,
                                diffusion_enabled,
                            ) {
                                break;
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                    }

                    if tick_started.elapsed() >= interval {
                        boundary_sender
                            .send(SimulationCommand::TickBoundary)
                            .expect("worker command receiver is alive");

                        if !drain_to_boundary(
                            &receiver,
                            &mut run,
                            &mut pending,
                            perlin_scale,
                            max_resource,
                            diffusion_enabled,
                        ) {
                            return;
                        }

                        tick_started = Instant::now();
                        advance_and_publish(&mut run, &mut publisher, &mut pending, &mut applied);
                    }
                }
            })?;

        Ok(Self {
            reader,
            controls: SimulationControls { sender },
            thread: Some(thread),
        })
    }

    /// Returns a reader for the latest completed snapshot.
    pub(super) fn reader(&self) -> SnapshotReader {
        self.reader.clone()
    }

    /// Returns a sender for commands to this run.
    pub(super) fn controls(&self) -> SimulationControls {
        self.controls.clone()
    }

    fn stop_and_join(&mut self) -> thread::Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };

        let _ = self.controls.sender.send(SimulationCommand::Stop);
        thread.join()
    }
}

impl Drop for SimulationWorker {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

fn handle_command(
    command: SimulationCommand,
    run: &mut SimulationRun,
    pending: &mut Vec<EffectUpdateRequest>,
    perlin_scale: f64,
    max_resource: f64,
    diffusion_enabled: bool,
) -> bool {
    match command {
        SimulationCommand::Stop => return false,
        SimulationCommand::ToggleNeighbourhood if diffusion_enabled => {
            match run.toggle_neighbourhood() {
                Ok(neighbourhood) => println!("Switched to {neighbourhood:?}"),
                Err(error) => eprintln!("Could not switch diffusion stencil. {error}"),
            }
        }
        SimulationCommand::ReseedField(seed) => {
            run.reseed_field(seed, perlin_scale, max_resource);
            println!("Reset resource field with seed {seed}");
        }
        SimulationCommand::SetVertexEffects(request) => queue_effect_update(request, pending),
        SimulationCommand::ToggleNeighbourhood | SimulationCommand::TickBoundary => {}
    }

    true
}

fn drain_to_boundary(
    receiver: &Receiver<SimulationCommand>,
    run: &mut SimulationRun,
    pending: &mut Vec<EffectUpdateRequest>,
    perlin_scale: f64,
    max_resource: f64,
    diffusion_enabled: bool,
) -> bool {
    while let Ok(command) = receiver.recv() {
        if matches!(command, SimulationCommand::TickBoundary) {
            return true;
        }

        if !handle_command(
            command,
            run,
            pending,
            perlin_scale,
            max_resource,
            diffusion_enabled,
        ) {
            return false;
        }
    }

    false
}

fn queue_effect_update(request: EffectUpdateRequest, pending: &mut Vec<EffectUpdateRequest>) {
    if let Some(index) = pending
        .iter()
        .position(|previous| previous.vertex_id == request.vertex_id)
    {
        let previous = pending.remove(index);
        let _ = previous.reply.send(EffectUpdateOutcome::Superseded);
    }

    pending.push(request);
}

fn advance_and_publish(
    run: &mut SimulationRun,
    publisher: &mut SnapshotPublisher,
    pending: &mut Vec<EffectUpdateRequest>,
    applied: &mut Vec<AppliedEffectUpdate>,
) {
    for request in pending.drain(..) {
        match run.set_vertex_effects(request.vertex_id, request.effects) {
            Ok(revision) => applied.push(AppliedEffectUpdate {
                vertex_id: request.vertex_id,
                revision,
                reply: request.reply,
            }),
            Err(error) => {
                let _ = request.reply.send(EffectUpdateOutcome::Rejected(error));
            }
        }
    }

    run.advance();
    let snapshot = run.snapshot();
    let tick = snapshot.tick;
    publisher.publish(snapshot);

    for update in applied.drain(..) {
        let _ = update.reply.send(EffectUpdateOutcome::Applied {
            vertex_id: update.vertex_id,
            revision: update.revision,
            tick,
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        effects::{Effect, EffectInput, EffectTarget, ResponseCurve},
        field::Neighbourhood,
        graph::{HealthConfig, Uptake, VertexConfig},
        simulation::SimulationConfig,
    };

    use super::*;

    fn run(rate: f64) -> SimulationRun {
        let config = SimulationConfig {
            width: 3,
            height: 2,
            spacing: 1.0,
            initial_resource: 10.0,
            resource_noise_scale: 10.0,
            diffusion_coefficient: 0.1,
            neighbourhood: Neighbourhood::VonNeumann,
        };

        let uptake = Uptake::new(rate, 1.0).unwrap();
        let vertex = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap();
        SimulationRun::single_vertex_from_seed(config, 42, vertex, 0.1).unwrap()
    }

    fn effects(multiplier: f64) -> EffectsConfig {
        EffectsConfig::new([Effect {
            input: EffectInput::Age,
            target: EffectTarget::UptakeCapacity,
            response: ResponseCurve::Constant { multiplier },
        }])
        .unwrap()
    }

    fn request(
        vertex_id: VertexId,
        effects: EffectsConfig,
    ) -> (EffectUpdateRequest, EffectUpdateTicket) {
        let (reply, receiver) = mpsc::channel();
        let request = EffectUpdateRequest {
            vertex_id,
            effects,
            reply,
        };

        let ticket = EffectUpdateTicket {
            receiver: Mutex::new(receiver),
            outcome: None,
        };

        (request, ticket)
    }

    #[test]
    fn tickets_are_send_and_sync_for_bevy_storage() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<EffectUpdateTicket>();
    }

    #[test]
    fn wall_clock_pacing_preserves_the_state_at_each_completed_model_tick() {
        for interval in [Duration::from_millis(5), Duration::from_millis(15)] {
            let mut worker = SimulationWorker::start(run(1.0), interval, 10.0, 10.0, true).unwrap();
            let reader = worker.reader();
            let deadline = Instant::now() + Duration::from_secs(2);
            let snapshot = loop {
                let snapshot = reader.latest();

                if snapshot.tick > 0 {
                    break snapshot;
                }

                assert!(Instant::now() < deadline, "worker did not advance");
                thread::sleep(Duration::from_millis(1));
            };

            worker.stop_and_join().unwrap();
            let mut reference = run(1.0);

            for _ in 0..snapshot.tick {
                reference.advance();
            }

            assert_eq!(*snapshot, reference.snapshot());
        }
    }

    #[test]
    fn tick_boundary_defers_later_commands_even_when_the_queue_stays_populated() {
        let mut run = run(1.0);
        let id = run.snapshot().vertices[0].id;
        let mut publisher = SnapshotPublisher::new(run.snapshot());
        let (sender, receiver) = mpsc::channel();
        let (first, mut first_ticket) = request(id, effects(0.0));
        let (later, mut later_ticket) = request(id, effects(2.0));
        sender
            .send(SimulationCommand::SetVertexEffects(first))
            .unwrap();
        sender.send(SimulationCommand::TickBoundary).unwrap();
        sender
            .send(SimulationCommand::SetVertexEffects(later))
            .unwrap();
        sender.send(SimulationCommand::TickBoundary).unwrap();
        sender.send(SimulationCommand::Stop).unwrap();
        let mut pending = Vec::new();
        let mut applied = Vec::new();

        assert!(drain_to_boundary(
            &receiver,
            &mut run,
            &mut pending,
            10.0,
            10.0,
            true
        ));
        advance_and_publish(&mut run, &mut publisher, &mut pending, &mut applied);

        assert!(matches!(
            first_ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Applied {
                revision: 1,
                tick: 1,
                ..
            })
        ));
        assert!(later_ticket.try_result().unwrap().is_none());
        assert_eq!(
            publisher.reader().latest().vertices[0]
                .effective_effects
                .uptake_capacity,
            0.0
        );
        assert!(drain_to_boundary(
            &receiver,
            &mut run,
            &mut pending,
            10.0,
            10.0,
            true
        ));
        advance_and_publish(&mut run, &mut publisher, &mut pending, &mut applied);

        assert!(matches!(
            later_ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Applied {
                revision: 2,
                tick: 2,
                ..
            })
        ));
        assert_eq!(
            publisher.reader().latest().vertices[0]
                .effective_effects
                .uptake_capacity,
            2.0
        );
        assert!(!drain_to_boundary(
            &receiver,
            &mut run,
            &mut pending,
            10.0,
            10.0,
            true
        ));
        assert_eq!(run.tick(), 2);
    }

    #[test]
    fn initial_snapshot_is_available_before_the_worker_steps() {
        let mut worker =
            SimulationWorker::start(run(1.0), Duration::from_secs(3), 10.0, 10.0, true).unwrap();
        let reader = worker.reader();
        let initial = reader.latest();
        let controls = worker.controls();
        let id = initial.vertices[0].id;
        let mut ticket = controls.set_vertex_effects(id, effects(0.0)).unwrap();

        assert_eq!(initial.seed, 42);
        assert_eq!(initial.tick, 0);
        assert_eq!(initial.vertices.len(), 1);
        assert!(ticket.try_result().unwrap().is_none());
        assert!(controls.reseed_field(43));

        worker.stop_and_join().unwrap();

        assert_eq!(reader.latest().tick, 0);
        assert!(!controls.toggle_neighbourhood());
        assert_eq!(
            ticket.try_result().unwrap_err(),
            SimulationControlError::WorkerDisconnected
        );
        assert_eq!(
            controls.set_vertex_effects(id, effects(0.0)).unwrap_err(),
            SimulationControlError::WorkerDisconnected,
        );
    }

    #[test]
    fn replacement_acknowledges_its_published_tick_and_keeps_prior_snapshot() {
        let mut run = run(1.0);
        let initial = run.snapshot();
        let id = initial.vertices[0].id;
        let mut publisher = SnapshotPublisher::new(initial);
        let reader = publisher.reader();
        let retained = reader.latest();
        let (request, mut ticket) = request(id, effects(0.0));
        let mut pending = vec![request];
        let mut applied = Vec::new();
        assert!(ticket.try_result().unwrap().is_none());

        advance_and_publish(&mut run, &mut publisher, &mut pending, &mut applied);

        assert!(matches!(
            ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Applied { vertex_id, revision: 1, tick: 1 }) if *vertex_id == id
        ));
        let published = reader.latest();
        assert_eq!(published.tick, 1);
        assert_eq!(published.vertices[0].effects_revision, 1);
        assert_eq!(published.vertices[0].effective_effects.uptake_capacity, 0.0);
        assert_eq!(published.vertices[0].stored_resource, 0.0);
        assert_eq!(retained.tick, 0);
        assert_eq!(retained.vertices[0].effects_revision, 0);
        assert_eq!(retained.vertices[0].effective_effects.uptake_capacity, 1.0);
        assert!(ticket.try_result().unwrap().is_some());
    }

    #[test]
    fn incompatible_effects_leave_the_applied_configuration_and_revision_intact() {
        let mut run = run(f64::MAX);
        let initial = run.snapshot();
        let id = initial.vertices[0].id;
        let mut publisher = SnapshotPublisher::new(initial);
        let (request, mut ticket) = request(id, effects(2.0));
        let mut pending = vec![request];
        let mut applied = Vec::new();

        advance_and_publish(&mut run, &mut publisher, &mut pending, &mut applied);

        assert!(matches!(
            ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Rejected(
                SimulationError::VertexConfig(_)
            ))
        ));
        let published = publisher.reader().latest();
        assert_eq!(published.vertices[0].effects_revision, 0);
        assert_eq!(published.vertices[0].effective_effects.uptake_capacity, 1.0);
        assert!(published.vertices[0].config.effects().effects().is_empty());
    }

    #[test]
    fn worker_publishes_an_applied_request_before_returning_its_acknowledgement() {
        let mut worker =
            SimulationWorker::start(run(1.0), Duration::from_millis(10), 10.0, 10.0, true).unwrap();
        let reader = worker.reader();
        let id = reader.latest().vertices[0].id;
        let mut ticket = worker
            .controls()
            .set_vertex_effects(id, effects(0.0))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let tick = loop {
            if let Some(outcome) = ticket.try_result().unwrap() {
                let EffectUpdateOutcome::Applied {
                    tick, revision: 1, ..
                } = outcome
                else {
                    panic!("valid request was not applied");
                };

                break *tick;
            }

            assert!(
                Instant::now() < deadline,
                "worker did not answer the update"
            );
            thread::sleep(Duration::from_millis(1));
        };

        let snapshot = reader.latest();
        assert!(snapshot.tick >= tick);
        assert_eq!(snapshot.vertices[0].effects_revision, 1);
        assert_eq!(snapshot.vertices[0].effective_effects.uptake_capacity, 0.0);
        worker.stop_and_join().unwrap();
        assert!(ticket.try_result().unwrap().is_some());
    }

    #[test]
    fn last_request_wins_and_stale_vertex_is_rejected_without_changes() {
        let mut run = run(1.0);
        let id = run.snapshot().vertices[0].id;
        let mut publisher = SnapshotPublisher::new(run.snapshot());
        let (first, mut first_ticket) = request(id, effects(0.0));
        let (last, mut last_ticket) = request(id, effects(2.0));
        let (stale, mut stale_ticket) = request(VertexId::default(), effects(0.0));
        let mut pending = Vec::new();
        let mut applied = Vec::new();
        queue_effect_update(first, &mut pending);
        queue_effect_update(last, &mut pending);
        queue_effect_update(stale, &mut pending);

        assert!(matches!(
            first_ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Superseded)
        ));
        assert!(last_ticket.try_result().unwrap().is_none());

        advance_and_publish(&mut run, &mut publisher, &mut pending, &mut applied);

        assert!(matches!(
            last_ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Applied {
                revision: 1,
                tick: 1,
                ..
            })
        ));
        assert!(matches!(
            stale_ticket.try_result().unwrap(),
            Some(EffectUpdateOutcome::Rejected(_))
        ));
        let published = publisher.reader().latest();
        assert_eq!(published.vertices[0].effects_revision, 1);
        assert_eq!(published.vertices[0].effective_effects.uptake_capacity, 2.0);
    }
}

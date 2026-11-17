//! Wall-clock pacing, commands, and shutdown for the simulation worker.

use std::{
    io,
    sync::mpsc::{self, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use bevy::prelude::Resource;

use crate::simulation::{SimulationRun, SnapshotPublisher, SnapshotReader};

enum SimulationCommand {
    ToggleNeighbourhood,
    ReseedField(u32),
    Stop,
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
        let thread = thread::Builder::new()
            .name("simulation".to_owned())
            .spawn(move || {
                let mut tick_started = Instant::now();

                loop {
                    let time_to_tick = interval.saturating_sub(tick_started.elapsed());

                    match receiver.recv_timeout(time_to_tick) {
                        Ok(SimulationCommand::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                        Ok(SimulationCommand::ToggleNeighbourhood) if diffusion_enabled => {
                            match run.toggle_neighbourhood() {
                                Ok(neighbourhood) => println!("Switched to {neighbourhood:?}"),
                                Err(error) => {
                                    eprintln!("Could not switch diffusion stencil. {error}")
                                }
                            }
                        }
                        Ok(SimulationCommand::ReseedField(seed)) => {
                            run.reseed_field(seed, perlin_scale, max_resource);
                            println!("Reset resource field with seed {seed}");
                        }
                        Ok(SimulationCommand::ToggleNeighbourhood)
                        | Err(RecvTimeoutError::Timeout) => {}
                    }

                    if tick_started.elapsed() >= interval {
                        tick_started = Instant::now();
                        run.advance();
                        publisher.publish(run.snapshot());
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

#[cfg(test)]
mod tests {
    use crate::{field::Neighbourhood, graph::Uptake, simulation::SimulationConfig};

    use super::*;

    #[test]
    fn initial_snapshot_is_available_before_the_worker_steps() {
        let config = SimulationConfig {
            width: 3,
            height: 2,
            spacing: 1.0,
            initial_resource: 10.0,
            resource_noise_scale: 10.0,
            diffusion_coefficient: 0.1,
            neighbourhood: Neighbourhood::VonNeumann,
        };

        let seed = 42;
        let uptake = Uptake::new(1.0, 1.0).unwrap();
        let run = SimulationRun::single_vertex_from_seed(config, seed, uptake, 0.1).unwrap();
        let mut worker =
            SimulationWorker::start(run, Duration::from_secs(3), 10.0, 10.0, true).unwrap();
        let reader = worker.reader();
        let initial = reader.latest();
        let controls = worker.controls();

        assert_eq!(initial.seed, seed);
        assert_eq!(initial.tick, 0);
        assert_eq!(initial.vertices.len(), 1);
        assert!(controls.reseed_field(43));

        worker.stop_and_join().unwrap();

        assert_eq!(reader.latest().tick, 0);
        assert!(!controls.toggle_neighbourhood());
    }
}

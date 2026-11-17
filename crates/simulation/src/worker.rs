use std::{
    io,
    sync::mpsc::{self, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use simulation::simulation::{SimulationRun, SnapshotPublisher, SnapshotReader};

pub(super) struct SimulationWorker {
    reader: SnapshotReader,
    stop_sender: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl SimulationWorker {
    pub(super) fn start(mut run: SimulationRun, interval: Duration) -> io::Result<Self> {
        if interval.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "simulation worker interval must be positive",
            ));
        }

        let mut publisher = SnapshotPublisher::new(run.snapshot());
        let reader = publisher.reader();
        let (stop_sender, stop_receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("simulation".to_owned())
            .spawn(move || {
                let mut tick_started = Instant::now();

                while let Err(RecvTimeoutError::Timeout) =
                    stop_receiver.recv_timeout(interval.saturating_sub(tick_started.elapsed()))
                {
                    tick_started = Instant::now();
                    run.advance();
                    publisher.publish(run.snapshot());
                }
            })?;

        Ok(Self {
            reader,
            stop_sender,
            thread: Some(thread),
        })
    }

    pub(super) fn reader(&self) -> SnapshotReader {
        self.reader.clone()
    }

    pub(super) fn stop(mut self) -> thread::Result<()> {
        self.stop_and_join()
    }

    fn stop_and_join(&mut self) -> thread::Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };

        let _ = self.stop_sender.send(());
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
    use simulation::{field::Neighbourhood, graph::Uptake, simulation::SimulationConfig};

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
        let worker = SimulationWorker::start(run, Duration::from_secs(3)).unwrap();
        let reader = worker.reader();
        let initial = reader.latest();

        assert_eq!(initial.seed, seed);
        assert_eq!(initial.tick, 0);
        assert_eq!(initial.vertices.len(), 1);

        worker.stop().unwrap();
        assert_eq!(reader.latest().tick, 0);
    }
}

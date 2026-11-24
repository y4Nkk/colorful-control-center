use super::{experiment::ActiveLoad, stopped};
use anyhow::{Context, Result, bail, ensure};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const MAX_SAMPLE_AGE: Duration = Duration::from_secs(2);

struct LoadState {
    stop: AtomicBool,
    last_sample: Mutex<Instant>,
}

pub(super) struct Load {
    state: Arc<LoadState>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl Load {
    pub(super) fn start(count: usize, duty: u8, sampled_at: Instant) -> Result<Self> {
        ensure!(
            sampled_at.elapsed() < MAX_SAMPLE_AGE,
            "启动负载前的温度采样已过期"
        );
        let mut load = Self {
            state: Arc::new(LoadState {
                stop: AtomicBool::new(false),
                last_sample: Mutex::new(sampled_at),
            }),
            threads: Vec::new(),
        };
        for _ in 0..count {
            let state = load.state.clone();
            load.threads.push(
                thread::Builder::new()
                    .spawn(move || burn(&state, duty))
                    .context("启动探测负载线程失败")?,
            );
        }
        Ok(load)
    }
}

impl ActiveLoad for Load {
    fn refresh(&self, sampled_at: Instant) -> Result<()> {
        let mut last = self
            .state
            .last_sample
            .lock()
            .map_err(|_| anyhow::anyhow!("取样时钟异常"))?;
        if last.elapsed() >= MAX_SAMPLE_AGE || sampled_at.elapsed() >= MAX_SAMPLE_AGE {
            bail!("温度采样超过 2 秒未确认，已停止负载");
        }
        *last = sampled_at;
        Ok(())
    }
}

impl Drop for Load {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn burn(state: &LoadState, duty: u8) {
    let period = Duration::from_millis(100);
    let busy = Duration::from_millis(u64::from(duty));
    while !stopped() && !state.stop.load(Ordering::Relaxed) {
        let Ok((start, elapsed)) = state.last_sample.lock().map(|last| {
            let start = Instant::now();
            (start, start.duration_since(*last))
        }) else {
            break;
        };
        let Some(fresh_for) = MAX_SAMPLE_AGE.checked_sub(elapsed) else {
            break;
        };
        while start.elapsed() < busy.min(fresh_for)
            && !stopped()
            && !state.stop.load(Ordering::Relaxed)
        {
            std::hint::spin_loop();
        }
        if let Some(rest) = period.checked_sub(start.elapsed()) {
            thread::sleep(rest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_sampling_cannot_start_or_restart_load() {
        let expired = Instant::now() - MAX_SAMPLE_AGE;
        assert!(Load::start(0, 30, expired).is_err());
        let load = Load::start(0, 30, Instant::now()).unwrap();
        *load.state.last_sample.lock().unwrap() = expired;
        assert!(load.refresh(Instant::now()).is_err());
    }

    #[test]
    fn load_workers_exit_on_stale_data_without_waiting_for_the_source() {
        let load = Load::start(1, 20, Instant::now()).unwrap();
        *load.state.last_sample.lock().unwrap() = Instant::now() - MAX_SAMPLE_AGE;
        let start = Instant::now();
        while !load.threads[0].is_finished() && start.elapsed() < Duration::from_secs(1) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(load.threads[0].is_finished());
    }
}

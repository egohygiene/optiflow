use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use signal_hook::consts::{SIGINT, SIGTERM};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interruption {
    Interrupt,
    Terminate,
}

#[derive(Debug, Clone, Default)]
pub struct SignalState {
    signal: Arc<AtomicUsize>,
}

impl SignalState {
    pub fn install() -> Result<Self> {
        let state = Self::default();
        signal_hook::flag::register_usize(SIGINT, Arc::clone(&state.signal), SIGINT as usize)
            .context("failed to install SIGINT handler")?;
        signal_hook::flag::register_usize(SIGTERM, Arc::clone(&state.signal), SIGTERM as usize)
            .context("failed to install SIGTERM handler")?;
        Ok(state)
    }

    pub fn current(&self) -> Option<Interruption> {
        match self.signal.load(Ordering::Relaxed) as i32 {
            SIGINT => Some(Interruption::Interrupt),
            SIGTERM => Some(Interruption::Terminate),
            _ => None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.current().is_some()
    }

    #[cfg(test)]
    pub fn interrupted(interruption: Interruption) -> Self {
        let state = Self::default();
        let value = match interruption {
            Interruption::Interrupt => SIGINT,
            Interruption::Terminate => SIGTERM,
        };
        state.signal.store(value as usize, Ordering::Relaxed);
        state
    }
}

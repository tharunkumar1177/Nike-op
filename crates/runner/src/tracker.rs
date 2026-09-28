//! Live tracking of the active profile's processes for the quick flyout.
//!
//! Each matching process is pinned by an open handle, so its exit is seen
//! without re-enumerating and its PID cannot be recycled while tracked.
//! Tracking runs only while the flyout is visible.

use crate::flyout::{ProcessKey, ProcessRow};
use edge_optimizer_core::process::{
    enumerate_processes, resolve_targets, ProcessTarget, WatchedProcess,
};

/// Full re-enumerations happen every this many ticks to find newly started
/// instances; the ticks in between only check tracked handles.
const RESCAN_EVERY_TICKS: u32 = 3;

pub struct ProcessTracker {
    session_id: u32,
    names: Vec<String>,
    watched: Vec<WatchedProcess>,
    ticks: u32,
}

impl ProcessTracker {
    pub fn new(session_id: u32) -> Self {
        Self {
            session_id,
            names: Vec::new(),
            watched: Vec::new(),
            ticks: 0,
        }
    }

    pub fn set_names(&mut self, names: &[String]) {
        if self.names != names {
            self.names = names.to_vec();
            self.watched.clear();
        }
    }

    /// Release every handle; the tracker is idle until the next rescan.
    pub fn clear(&mut self) {
        self.watched.clear();
        self.ticks = 0;
    }

    /// Resolve the current names to live processes and start watching new ones.
    pub fn rescan(&mut self) {
        self.watched.retain(|process| !process.has_exited());
        if self.names.is_empty() {
            self.watched.clear();
            return;
        }
        let processes = match enumerate_processes() {
            Ok(processes) => processes,
            Err(error) => {
                tracing::warn!("cannot enumerate processes for the flyout: {:#}", error);
                return;
            }
        };
        let targets = resolve_targets(&processes, &self.names, self.session_id, std::process::id());
        self.watched
            .retain(|process| targets.contains(&process.target));
        for target in targets {
            let known = self.watched.iter().any(|process| process.target == target);
            if !known {
                if let Some(process) = WatchedProcess::open(target) {
                    self.watched.push(process);
                }
            }
        }
    }

    /// Advance one flyout tick; returns whether the tracked set changed.
    pub fn tick(&mut self) -> bool {
        let before: Vec<ProcessKey> = self.rows().into_iter().map(|row| row.key).collect();
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks.is_multiple_of(RESCAN_EVERY_TICKS) {
            self.rescan();
        } else {
            self.watched.retain(|process| !process.has_exited());
        }
        let after: Vec<ProcessKey> = self.rows().into_iter().map(|row| row.key).collect();
        before != after
    }

    pub fn rows(&self) -> Vec<ProcessRow> {
        self.watched
            .iter()
            .map(|process| ProcessRow {
                key: key_of(&process.target),
                image_name: process.target.image_name.clone(),
            })
            .collect()
    }

    /// Targets for one tracked process, or for all of them.
    pub fn targets(&self, only: Option<ProcessKey>) -> Vec<ProcessTarget> {
        self.watched
            .iter()
            .filter(|process| !process.has_exited())
            .filter(|process| only.is_none_or(|key| key_of(&process.target) == key))
            .map(|process| process.target.clone())
            .collect()
    }
}

fn key_of(target: &ProcessTarget) -> ProcessKey {
    ProcessKey {
        pid: target.pid,
        creation_time: target.creation_time,
    }
}

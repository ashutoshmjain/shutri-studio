//! In-process job registry. Long work never blocks a UI command:
//! submit -> job_id -> worker -> progress events (the Tauri layer emits).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub label: String,
    pub state: JobState,
    pub progress: f32, // 0.0 - 1.0
    pub message: String,
}

pub struct Jobs {
    next: AtomicU64,
    map: Mutex<HashMap<u64, Job>>,
}

impl Default for Jobs {
    fn default() -> Self {
        Self::new()
    }
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn submit(&self, label: impl Into<String>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        self.map.lock().unwrap().insert(
            id,
            Job {
                id,
                label: label.into(),
                state: JobState::Queued,
                progress: 0.0,
                message: String::new(),
            },
        );
        id
    }

    pub fn update(&self, id: u64, state: JobState, progress: f32, message: impl Into<String>) {
        if let Some(j) = self.map.lock().unwrap().get_mut(&id) {
            j.state = state;
            j.progress = progress.clamp(0.0, 1.0);
            j.message = message.into();
        }
    }

    pub fn get(&self, id: u64) -> Option<Job> {
        self.map.lock().unwrap().get(&id).cloned()
    }

    pub fn list(&self) -> Vec<Job> {
        let mut v: Vec<Job> = self.map.lock().unwrap().values().cloned().collect();
        v.sort_by_key(|j| j.id);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let jobs = Jobs::new();
        let id = jobs.submit("render");
        jobs.update(id, JobState::Done, 1.0, "ok");
        assert_eq!(jobs.get(id).unwrap().state, JobState::Done);
    }
}

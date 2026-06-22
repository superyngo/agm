use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// A repo update started
    UpdateRepoStart { name: String },
    /// A repo update finished
    UpdateRepoComplete {
        name: String,
        success: bool,
        message: String,
    },
    /// All updates finished
    UpdateAllDone {
        total: usize,
        updated: usize,
        new_skills: usize,
        new_agents: usize,
        new_commands: usize,
    },
    /// Generic operation result (for tool TUI link/unlink etc.)
    #[allow(dead_code)]
    OperationResult { message: String, success: bool },
    /// A line of git/clone progress while adding a source.
    CloneLine { line: String, is_err: bool },
    /// Adding a source finished. Counts are items *found* (not installed).
    AddDone {
        name: String,
        skill_count: usize,
        agent_count: usize,
        command_count: usize,
        success: bool,
        message: String,
    },
}

pub struct BackgroundTask {
    receiver: mpsc::Receiver<TaskEvent>,
    pub is_running: bool,
    pub progress: Option<String>,
}

impl BackgroundTask {
    pub fn new(receiver: mpsc::Receiver<TaskEvent>) -> Self {
        Self {
            receiver,
            is_running: true,
            progress: None,
        }
    }

    /// Non-blocking drain of all pending events. Returns collected events.
    pub fn poll(&mut self) -> Vec<TaskEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            match &event {
                TaskEvent::UpdateAllDone { .. } => self.is_running = false,
                TaskEvent::UpdateRepoStart { name } => {
                    self.progress = Some(format!("Updating {}...", name));
                }
                TaskEvent::UpdateRepoComplete { .. } => {}
                TaskEvent::OperationResult { .. } => {}
                TaskEvent::CloneLine { line, .. } => {
                    self.progress = Some(line.clone());
                }
                TaskEvent::AddDone { .. } => self.is_running = false,
            }
            events.push(event);
        }
        events
    }
}

/// Spawn a background update. Returns BackgroundTask.
/// NOTE: This function calls `crate::skills::update_all_with_progress` which doesn't exist yet
/// (Task 2.1). For now, create the function signature but use a placeholder that we'll wire up later.
///
/// The actual wiring will look like:
/// ```ignore
/// pub fn spawn_update(
///     skills_dir: PathBuf,
///     agents_dir: PathBuf,
///     source_dir: PathBuf,
/// ) -> BackgroundTask {
///     let (tx, rx) = mpsc::channel();
///     thread::spawn(move || {
///         crate::skills::update_all_with_progress(&skills_dir, &agents_dir, &source_dir, |progress| {
///             let event = match progress {
///                 UpdateProgress::RepoStart { name } => TaskEvent::UpdateRepoStart { name },
///                 UpdateProgress::RepoComplete { name, success, message } =>
///                     TaskEvent::UpdateRepoComplete { name, success, message },
///                 UpdateProgress::AllDone { total, updated, new_skills, new_agents } =>
///                     TaskEvent::UpdateAllDone { total, updated, new_skills, new_agents },
///             };
///             let _ = tx.send(event);
///         });
///     });
///     BackgroundTask::new(rx)
/// }
/// ```
///
/// For NOW, just implement it with a TODO comment and have it accept a generic closure:
#[allow(dead_code)]
pub fn spawn_with<F>(task_fn: F) -> BackgroundTask
where
    F: FnOnce(mpsc::Sender<TaskEvent>) + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        task_fn(tx);
    });
    BackgroundTask::new(rx)
}

/// Spawn a background update. Returns BackgroundTask.
pub fn spawn_update(
    skills_dir: PathBuf,
    agents_dir: PathBuf,
    commands_dir: PathBuf,
    source_dir: PathBuf,
) -> BackgroundTask {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        crate::skills::update_all_with_progress(
            &skills_dir,
            &agents_dir,
            &commands_dir,
            &source_dir,
            |progress| {
                let event = match progress {
                    crate::skills::UpdateProgress::RepoStart { name } => {
                        TaskEvent::UpdateRepoStart { name }
                    }
                    crate::skills::UpdateProgress::RepoComplete {
                        name,
                        success,
                        message,
                    } => TaskEvent::UpdateRepoComplete {
                        name,
                        success,
                        message,
                    },
                    crate::skills::UpdateProgress::AllDone {
                        total,
                        updated,
                        new_skills,
                        new_agents,
                        new_commands,
                    } => TaskEvent::UpdateAllDone {
                        total,
                        updated,
                        new_skills,
                        new_agents,
                        new_commands,
                    },
                };
                let _ = tx.send(event);
            },
        );
    });
    BackgroundTask::new(rx)
}

/// Spawn a background add. Clones/copies the source (no skills are installed).
/// Reports git progress as `CloneLine` events and finishes with `AddDone`
/// carrying the count of items *found* in the source.
pub fn spawn_add(source: String, is_url: bool, source_dir: PathBuf) -> BackgroundTask {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let forward = |evt: &crate::skills::CloneProgress, tx: &mpsc::Sender<TaskEvent>| {
            if let crate::skills::CloneProgress::GitLine { line, is_err } = evt {
                let _ = tx.send(TaskEvent::CloneLine {
                    line: line.clone(),
                    is_err: *is_err,
                });
            }
        };

        let result = if is_url {
            crate::skills::clone_or_pull(&source, &source_dir, None, |evt| forward(&evt, &tx))
        } else {
            let path = crate::paths::expand_tilde(&source);
            crate::skills::add_local_copy(&path, &source_dir, None, |evt| forward(&evt, &tx))
        };

        let event = match result {
            Ok((repo_path, found_skills)) => {
                let name = repo_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("source")
                    .to_string();
                let skill_count = found_skills.len();
                let agent_count = crate::skills::scan_agents(&repo_path).len();
                let command_count = crate::skills::scan_commands(&repo_path).len();
                TaskEvent::AddDone {
                    name,
                    skill_count,
                    agent_count,
                    command_count,
                    success: true,
                    message: "Added".to_string(),
                }
            }
            Err(e) => TaskEvent::AddDone {
                name: source.clone(),
                skill_count: 0,
                agent_count: 0,
                command_count: 0,
                success: false,
                message: e.to_string(),
            },
        };
        let _ = tx.send(event);
    });
    BackgroundTask::new(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_poll_drains_events() {
        let (tx, rx) = mpsc::channel();
        let mut task = BackgroundTask::new(rx);

        // Send multiple events
        tx.send(TaskEvent::UpdateRepoStart {
            name: "repo1".to_string(),
        })
        .unwrap();
        tx.send(TaskEvent::UpdateRepoComplete {
            name: "repo1".to_string(),
            success: true,
            message: "Success".to_string(),
        })
        .unwrap();
        tx.send(TaskEvent::OperationResult {
            message: "Operation complete".to_string(),
            success: true,
        })
        .unwrap();

        let events = task.poll();
        assert_eq!(events.len(), 3);

        // Verify the events are in order
        match &events[0] {
            TaskEvent::UpdateRepoStart { name } => assert_eq!(name, "repo1"),
            _ => panic!("Expected UpdateRepoStart"),
        }
        match &events[1] {
            TaskEvent::UpdateRepoComplete {
                name,
                success,
                message,
            } => {
                assert_eq!(name, "repo1");
                assert!(success);
                assert_eq!(message, "Success");
            }
            _ => panic!("Expected UpdateRepoComplete"),
        }
        match &events[2] {
            TaskEvent::OperationResult { message, success } => {
                assert_eq!(message, "Operation complete");
                assert!(success);
            }
            _ => panic!("Expected OperationResult"),
        }
    }

    #[test]
    fn test_poll_sets_not_running_on_done() {
        let (tx, rx) = mpsc::channel();
        let mut task = BackgroundTask::new(rx);

        assert!(task.is_running);

        tx.send(TaskEvent::UpdateAllDone {
            total: 5,
            updated: 3,
            new_skills: 2,
            new_agents: 1,
            new_commands: 0,
        })
        .unwrap();

        let events = task.poll();
        assert_eq!(events.len(), 1);
        assert!(!task.is_running);
    }

    #[test]
    fn test_poll_empty_channel() {
        let (_tx, rx) = mpsc::channel();
        let mut task = BackgroundTask::new(rx);

        let events = task.poll();
        assert!(events.is_empty());
        assert!(task.is_running);
    }

    #[test]
    fn test_poll_updates_progress() {
        let (tx, rx) = mpsc::channel();
        let mut task = BackgroundTask::new(rx);

        assert!(task.progress.is_none());

        tx.send(TaskEvent::UpdateRepoStart {
            name: "test-repo".to_string(),
        })
        .unwrap();

        let events = task.poll();
        assert_eq!(events.len(), 1);
        assert_eq!(task.progress, Some("Updating test-repo...".to_string()));
    }

    #[test]
    fn test_spawn_with() {
        let task = spawn_with(|tx| {
            tx.send(TaskEvent::OperationResult {
                message: "Test message".to_string(),
                success: true,
            })
            .unwrap();
        });

        // Wait briefly for the thread to execute
        std::thread::sleep(Duration::from_millis(10));

        // Poll to get the event
        let mut task = task;
        let events = task.poll();

        assert_eq!(events.len(), 1);
        match &events[0] {
            TaskEvent::OperationResult { message, success } => {
                assert_eq!(message, "Test message");
                assert!(success);
            }
            _ => panic!("Expected OperationResult"),
        }
    }
}

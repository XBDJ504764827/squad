use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{AgentError, AgentFileChanged, PathPolicy, WorkspaceRootConfig};

pub struct FileWatcher {
    path_policy: PathPolicy,
    receiver: Receiver<notify::Result<notify::Event>>,
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(path_policy: PathPolicy, roots: &[WorkspaceRootConfig]) -> Result<Self, AgentError> {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |result| {
                let _ = sender.send(result);
            },
            Config::default(),
        )?;

        for root in roots {
            watcher.watch(&root.local_root, RecursiveMode::Recursive)?;
        }

        Ok(Self {
            path_policy,
            receiver,
            _watcher: watcher,
        })
    }

    pub fn poll_changes(&mut self) -> Result<Vec<AgentFileChanged>, AgentError> {
        let mut changed_paths = BTreeSet::new();

        while let Ok(event) = self.receiver.try_recv() {
            let event = event?;
            if !should_emit_event(&event.kind) {
                continue;
            }

            for path in event.paths {
                if let Ok(logical_path) = self.path_policy.local_to_logical(&path) {
                    changed_paths.insert(logical_path);
                }
            }
        }

        Ok(changed_paths
            .into_iter()
            .map(|logical_path| AgentFileChanged { logical_path })
            .collect())
    }
}

fn should_emit_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

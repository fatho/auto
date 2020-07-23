use std::collections::{HashMap, HashSet};
use std::fmt::Display;

/// Unique ID of tasks to be run.
/// TODO: make this type more lightweight to clone and hash
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct TaskId(pub String);

impl Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for TaskId {
    fn from(id: &str) -> Self {
        TaskId(id.into())
    }
}

#[derive(Debug)]
pub struct Task<P> {
    pub id: TaskId,
    pub needs: Vec<TaskId>,
    pub payload: P,
}

#[derive(Debug)]
pub struct PlanQueue<P> {
    /// All tasks that cannot be executed yet
    blocked: HashMap<TaskId, TaskState<P>>,
    /// Tasks indexed by reverse dependecy relationship
    needed_by: HashMap<TaskId, Vec<TaskId>>,
    /// Set of tasks that can be run right now
    available: Vec<Task<P>>,
}

#[derive(Debug)]
struct TaskState<P> {
    task: Task<P>,
    remaining_needs: HashSet<TaskId>,
}

impl<P> PlanQueue<P> {
    pub fn new() -> Self {
        Self {
            blocked: HashMap::new(),
            needed_by: HashMap::new(),
            available: Vec::new(),
        }
    }

    /// Remove a task from the available set.
    pub fn pop_available(&mut self) -> Option<Task<P>> {
        self.available.pop()
    }

    /// Unblocks tasks that depended on the task that was done.
    pub fn mark_done(&mut self, task: &TaskId) {
        if let Some(dependents) = self.needed_by.remove(&task) {
            for dependent in dependents {
                let needs = &mut self
                    .blocked
                    .get_mut(&dependent)
                    .expect("We verified that this must exist at insertion time")
                    .remaining_needs;
                if needs.remove(task) && needs.is_empty() {
                    let state = self.blocked.remove(&dependent).expect("Known to be there");
                    self.available.push(state.task);
                }
            }
        }
    }

    /// Stop processing and return the remaining tasks.
    pub fn give_up(self) -> Vec<Task<P>> {
        self.blocked.into_iter().map(|(_, state)| state.task).collect()
    }

    pub fn insert(&mut self, task: Task<P>) {
        for need in &task.needs {
            self.needed_by
                .entry(need.clone())
                .or_default()
                .push(task.id.clone());
        }

        if task.needs.is_empty() {
            self.available.push(task);
        } else {
            self.blocked.insert(
                task.id.clone(),
                TaskState {
                    remaining_needs: task.needs.iter().cloned().collect(),
                    task,
                },
            );
        }
    }
}

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Display;

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
pub struct Task {
    pub id: TaskId,
    pub program: OsString,
    pub arguments: Vec<OsString>,
    pub needs: Vec<TaskId>,
}

#[derive(Debug)]
pub struct PlanQueue {
    /// All known tasks
    tasks: HashMap<TaskId, Task>,
    /// Tasks indexed by reverse dependecy relationship
    needed_by: HashMap<TaskId, Vec<TaskId>>,
    /// Set of tasks that can be run right now
    available: Vec<TaskId>,
}

impl PlanQueue {
    pub fn new() -> PlanQueue {
        Self {
            tasks: HashMap::new(),
            needed_by: HashMap::new(),
            available: Vec::new(),
        }
    }

    /// Remove a task from the available set.
    pub fn pop_available(&mut self) -> Option<TaskId> {
        self.available.pop()
    }

    /// Unblocks tasks that depended on the task that was done.
    pub fn mark_done(&mut self, task: &TaskId) {
        if let Some(dependents) = self.needed_by.remove(&task) {
            for dependent in dependents {
                let needs = &mut self
                    .tasks
                    .get_mut(&dependent)
                    .expect("We verified that this must exist at insertion time")
                    .needs;
                needs.retain(|x| x != task);
                // TODO: need to remember original needs?
                if needs.is_empty() {
                    self.available.push(dependent.clone());
                }
            }
        }
    }

    pub fn insert(&mut self, task: Task) -> bool {
        if self.tasks.contains_key(&task.id) {
            return false;
        }

        for need in &task.needs {
            if !self.tasks.contains_key(need) {
                return false;
            }
            self.needed_by
                .entry(need.clone())
                .or_default()
                .push(task.id.clone());
        }

        if task.needs.is_empty() {
            self.available.push(task.id.clone());
        }
        self.tasks.insert(task.id.clone(), task);

        true
    }
}

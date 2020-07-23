use snafu::Snafu;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

/// Unique ID of tasks to be run.
/// TODO: make this type more lightweight to clone and hash
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

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
pub struct TaskQueue<P> {
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

impl<P> Default for TaskQueue<P> {
    fn default() -> Self {
        Self {
            blocked: HashMap::new(),
            needed_by: HashMap::new(),
            available: Vec::new(),
        }
    }
}

impl<P> TaskQueue<P> {
    pub fn new<I: IntoIterator<Item = Task<P>>>(tasks: I) -> Result<Self> {
        QueuePlanner::new(tasks).plan()
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
        self.blocked
            .into_iter()
            .map(|(_, state)| state.task)
            .collect()
    }

    fn insert(&mut self, task: Task<P>) {
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

/// Compute the dependency queue using topological sorting based on depth-first search.
struct QueuePlanner<P> {
    taskmap: HashMap<TaskId, Task<P>>,
    visited: HashSet<TaskId>,
    visiting: HashSet<TaskId>,
    plan: TaskQueue<P>,
    stack: Vec<TaskId>,
}

impl<P> QueuePlanner<P> {
    pub fn new<I: IntoIterator<Item = Task<P>>>(tasks: I) -> Self {
        let taskmap = tasks.into_iter().map(|t| (t.id.clone(), t)).collect();
        Self {
            taskmap,
            visited: HashSet::new(),
            visiting: HashSet::new(),
            plan: TaskQueue::default(),
            stack: Vec::new(),
        }
    }

    fn plan(mut self) -> Result<TaskQueue<P>> {
        while let Some(key) = self.taskmap.keys().next().cloned() {
            self.topo(&key)?;
        }
        Ok(self.plan)
    }

    fn topo(&mut self, current: &TaskId) -> Result<()> {
        if self.visited.contains(current) {
            return Ok(());
        } else if !self.visiting.insert(current.clone()) {
            let mut chain = vec![current.to_owned()];
            for prev in self.stack.iter().rev() {
                chain.push((*prev).to_owned());
                if prev == current {
                    break;
                }
            }
            chain.reverse();

            // We arrived at the same node `current` while already visiting `current`
            return Err(Error::CircularDependency { chain });
        }
        self.stack.push(current.clone());

        let task = self
            .taskmap
            .remove(current)
            .ok_or_else(|| Error::UnknownReference {
                dependency: current.to_owned(),
                dependent: self
                    .stack
                    .iter()
                    .nth_back(1)
                    .expect("Must have a parent, otherwise it would exist")
                    .to_owned(),
            })?;

        // Insert all dependencies first
        for needed in task.needs.iter() {
            self.topo(needed)?;
        }
        // Then insert current
        self.plan.insert(task);

        self.stack.pop();
        self.visited.insert(current.clone());
        Ok(())
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Circular dependency chain: {}", DisplayChain { chain }))]
    CircularDependency { chain: Vec<TaskId> },

    #[snafu(display("Dependency {:?} of task {:?} is not known", dependency.as_str(), dependent.as_str()))]
    UnknownReference {
        dependency: TaskId,
        dependent: TaskId,
    },
}

struct DisplayChain<'a> {
    chain: &'a [TaskId],
}

impl<'a> Display for DisplayChain<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ids = self.chain.iter();
        if let Some(first) = ids.next() {
            write!(f, "{:?}", first.as_str())?;
        } else {
            write!(f, "()")?;
        }
        for next in ids {
            write!(f, " -> {:?}", next.as_str())?;
        }
        Ok(())
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

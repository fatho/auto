use std::collections::HashSet;

use snafu::{ResultExt, Snafu};

mod autofile;
mod planner;

fn main() {
    if let Err(err) = run() {
        println!("{}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let example = r#"
    [tasks.build]
    program = "/usr/bin/echo"
    arguments = ["-e", "building\nreally\nhard"]

    [tasks.lint]
    program = "/usr/bin/echo"
    arguments = ["-e", "some linting"]

    [tasks.test]
    program = "/usr/bin/true"
    needs = ["build"]

    [tasks.ship]
    program = "/usr/bin/echo"
    arguments = ["-e", "shipping now\nand done"]

    needs = ["test", "lint"]
    "#;
    let autofile: autofile::AutoFile = toml::from_str(&example).context(LoadConfig)?;

    let mut plan = TopoPlanner::new(&autofile).plan()?;

    println!("{:?}", plan);

    while let Some(task) = plan.pop_available() {
        println!("running {} ... done", task);
        plan.mark_done(&task);
    }

    Ok(())
}

struct TopoPlanner<'a> {
    autofile: &'a autofile::AutoFile,
    visited: HashSet<&'a str>,
    visiting: HashSet<&'a str>,
    plan: planner::PlanQueue,
    stack: Vec<&'a str>,
}

impl<'a> TopoPlanner<'a> {
    pub fn new(autofile: &'a autofile::AutoFile) -> TopoPlanner {
        Self {
            autofile,
            visited: HashSet::new(),
            visiting: HashSet::new(),
            plan: planner::PlanQueue::new(),
            stack: Vec::new(),
        }
    }

    pub fn plan(mut self) -> Result<planner::PlanQueue> {
        for id in self.autofile.tasks.keys() {
            self.topo(&id)?;
        }
        Ok(self.plan)
    }

    fn topo(&mut self, current: &'a str) -> Result<()> {
        use planner::TaskId;

        if self.visited.contains(current) {
            return Ok(());
        } else if !self.visiting.insert(current) {
            let mut chain = vec![current.to_owned()];
            for prev in self.stack.iter().rev() {
                chain.push((*prev).to_owned());
                if *prev == current {
                    break;
                }
            }
            chain.reverse();

            // We arrived at the same node `current` while already visiting `current`
            return Err(Error::CircularDependency { chain });
        }
        self.stack.push(current);

        let task = self
            .autofile
            .tasks
            .get(current)
            .ok_or_else(|| Error::UnknownReference {
                id: current.to_owned(),
            })?;

        // Insert all dependencies first
        for needed in task.needs.iter() {
            self.topo(needed)?;
        }
        // Then insert current
        let success = self.plan.insert(planner::Task {
            id: TaskId(current.to_owned()),
            program: (&task.program).into(),
            arguments: task.arguments.iter().map(|s| s.into()).collect(),
            needs: task.needs.iter().map(|id| TaskId(id.to_owned())).collect(),
        });
        assert!(
            success,
            "Insertion should have succeeded because all invariants were validated"
        );

        self.stack.pop();
        self.visited.insert(current);
        Ok(())
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not load config: {}", source))]
    LoadConfig { source: toml::de::Error },

    #[snafu(display("Circular dependency chain: {:?}", chain))]
    CircularDependency { chain: Vec<String> },

    #[snafu(display("Referenced task {} is not known", id))]
    UnknownReference { id: String },
}

type Result<T, E = Error> = std::result::Result<T, E>;

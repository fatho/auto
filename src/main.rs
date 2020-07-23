use std::{ffi::OsString, collections::HashSet};
use ansi_term::Color;
use snafu::{ResultExt, Snafu};

mod autofile;
mod planner;

fn main() {
    if let Err(err) = run() {
        eprintln!("{}{}{}", Color::Red.prefix(), err, Color::Red.suffix());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let example = r#"
    [tasks.build]
    program = "echo"
    arguments = ["-e", "building\nreally\nhard"]

    [tasks.lint]
    program = "echo"
    arguments = ["-e", "some linting"]

    [tasks.test]
    program = "false"
    needs = ["build"]

    [tasks.ship]
    program = "echo"
    arguments = ["-e", "shipping now\nand done"]

    needs = ["test", "lint"]
    "#;
    let autofile: autofile::AutoFile = toml::from_str(&example).context(LoadConfig)?;

    let mut plan = TopoPlanner::new(&autofile).plan()?;

    eprintln!("{:?}", plan);

    while let Some(task) = plan.pop_available() {
        eprintln!("{} ... {}", Color::Blue.bold().paint("running"), task.id);

        let mut cmd = std::process::Command::new(&task.payload.program)
            .args(&task.payload.arguments)
            .spawn()
            .context(TaskStart { id: task.id.clone() })?;

        let status = cmd.wait().context(TaskWait { id: task.id.clone() })?;

        if status.success() {
            plan.mark_done(&task.id);
            eprintln!("{} {}", Color::Green.bold().paint("success"), task.id);
        } else {
            eprintln!("{} {}", Color::Red.bold().paint("failed"), task.id);
        }
    }

    for remaining in plan.give_up() {
        eprintln!("{} {}", Color::Red.bold().paint("not running"), remaining.id);
    }

    Ok(())
}

struct TopoPlanner<'a> {
    autofile: &'a autofile::AutoFile,
    visited: HashSet<&'a str>,
    visiting: HashSet<&'a str>,
    plan: planner::PlanQueue<Cmd>,
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

    pub fn plan(mut self) -> Result<planner::PlanQueue<Cmd>> {
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
                dependency: current.to_owned(),
                dependent: self
                    .stack
                    .iter()
                    .copied()
                    .nth_back(1)
                    .expect("Must have a parent, otherwise it would exist")
                    .to_owned(),
            })?;

        // Insert all dependencies first
        for needed in task.needs.iter() {
            self.topo(needed)?;
        }
        // Then insert current
        self.plan.insert(planner::Task {
            id: TaskId(current.to_owned()),
            needs: task.needs.iter().map(|id| TaskId(id.to_owned())).collect(),
            payload: Cmd {
                program: (&task.program).into(),
                arguments: task.arguments.iter().map(|s| s.into()).collect(),
            },
        });

        self.stack.pop();
        self.visited.insert(current);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Cmd {
    pub program: OsString,
    pub arguments: Vec<OsString>,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not load config: {}", source))]
    LoadConfig { source: toml::de::Error },

    #[snafu(display("Circular dependency chain: {:?}", chain))]
    CircularDependency { chain: Vec<String> },

    #[snafu(display("Dependency {:?} of task {:?} is not known", dependency, dependent))]
    UnknownReference {
        dependency: String,
        dependent: String,
    },

    #[snafu(display("Failed to spawn {:?}: {}", id, source))]
    TaskStart { id: planner::TaskId,  source: std::io::Error },

    #[snafu(display("Failed to wait for {:?}: {}", id, source))]
    TaskWait { id: planner::TaskId,  source: std::io::Error },
}

type Result<T, E = Error> = std::result::Result<T, E>;

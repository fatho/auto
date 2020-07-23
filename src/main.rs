use ansi_term::Color;
use snafu::{ResultExt, Snafu};
use std::ffi::OsString;

mod autofile;
mod queue;

fn main() {
    if let Err(err) = run() {
        eprintln!("{}{}{}", Color::Red.prefix(), err, Color::Red.suffix());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let example = r#"
    [tasks.build]
    program = "bash"
    arguments = ["-c", "echo 'building' && sleep 3 && echo 'done'"]

    [tasks.lint]
    program = "bash"
    arguments = ["-c", "echo 'lint' && sleep 1 && echo 'done'"]

    [tasks.test]
    program = "bash"
    arguments = ["-c", "echo 'testing' && sleep 2 && echo 'oh no' && false"]
    needs = ["build"]

    [tasks.other-test]
    program = "bash"
    arguments = ["-c", "echo 'more testing' && sleep 2 && echo 'more testing successful'"]
    needs = ["build"]

    [tasks.ship]
    program = "bash"
    arguments = ["-c", "echo 'shipping...' && sleep 1 && echo 'Aaand it's gone.'"]

    needs = ["test", "lint"]
    "#;
    let autofile: autofile::AutoFile = toml::from_str(&example).context(LoadConfig)?;

    let mut plan = queue::TaskQueue::new(autofile.tasks.iter().map(|(id, task)| {
        queue::Task {
            id: queue::TaskId(id.clone()),
            needs: task
                .needs
                .iter()
                .map(|id| queue::TaskId(id.to_owned()))
                .collect(),
            payload: Cmd {
                program: (&task.program).into(),
                arguments: task.arguments.iter().map(|s| s.into()).collect(),
            },
        }
    }))
    .context(Planner)?;

    eprintln!("{:?}", plan);

    while let Some(task) = plan.pop_available() {
        eprintln!("{} ... {}", Color::Blue.bold().paint("running"), task.id);

        let mut cmd = std::process::Command::new(&task.payload.program)
            .args(&task.payload.arguments)
            .spawn()
            .context(TaskStart {
                id: task.id.clone(),
            })?;

        let status = cmd.wait().context(TaskWait {
            id: task.id.clone(),
        })?;

        if status.success() {
            plan.mark_done(&task.id);
            eprintln!("{} {}", Color::Green.bold().paint("success"), task.id);
        } else {
            eprintln!("{} {}", Color::Red.bold().paint("failed"), task.id);
        }
    }

    for remaining in plan.give_up() {
        eprintln!(
            "{} {}",
            Color::Red.bold().paint("not running"),
            remaining.id
        );
    }

    Ok(())
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

    #[snafu(display("Failed to compute execution plan: {}", source))]
    Planner { source: queue::Error },

    #[snafu(display("Failed to spawn {:?}: {}", id, source))]
    TaskStart {
        id: queue::TaskId,
        source: std::io::Error,
    },

    #[snafu(display("Failed to wait for {:?}: {}", id, source))]
    TaskWait {
        id: queue::TaskId,
        source: std::io::Error,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

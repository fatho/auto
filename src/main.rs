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

    eprintln!("Generated plan for {} tasks", autofile.tasks.len());

    let outdir = tempfile::tempdir().context(Temp)?;
    eprintln!("Logging output to {}", outdir.path().display());

    let mut successful = Vec::new();
    let mut failed = Vec::new();

    while let Some(task) = plan.pop_available() {
        eprintln!("{} {}", Color::Blue.bold().paint("Running"), task.id);

        // Create files for redirecting output
        let task_stdout_path = outdir.path().join(&task.id.as_str()).with_extension("stdout");
        let task_stderr_path = outdir.path().join(&task.id.as_str()).with_extension("stderr");
        let task_stdout = std::fs::File::create(task_stdout_path).context(Temp)?;
        let task_stderr = std::fs::File::create(task_stderr_path).context(Temp)?;

        let start_time = std::time::Instant::now();
        let mut cmd = std::process::Command::new(&task.payload.program)
            .args(&task.payload.arguments)
            .stdout(task_stdout)
            .stderr(task_stderr)
            .spawn()
            .context(TaskStart {
                id: task.id.clone(),
            })?;

        let status = cmd.wait().context(TaskWait {
            id: task.id.clone(),
        })?;
        let duration = start_time.elapsed();

        let msg = if status.success() {
            Color::Green.bold().paint("Finished")
        } else {
            Color::Red.bold().paint("Failed")
        };
        eprintln!("{} {} (took {:.2}s)", msg, task.id, duration.as_secs_f64());

        if status.success() {
            plan.mark_done(&task.id);
            successful.push(task);
        } else {
            failed.push(task);
        };
    }

    let not_started = plan.give_up();
    for remaining in &not_started {
        eprintln!(
            "{} {}",
            Color::Red.bold().paint("not running"),
            remaining.id
        );
    }

    eprintln!("{} successful, {} failed, {} not started", successful.len(), failed.len(), not_started.len());

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

    #[snafu(display("Failed to create temporary output: {}", source))]
    Temp { source: std::io::Error },

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

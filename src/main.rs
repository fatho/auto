use ansi_term::Color;
use snafu::{ResultExt, Snafu};
use std::ffi::OsString;
use std::path::Path;
use std::sync::mpsc;

mod autofile;
mod queue;
use std::{collections::HashSet, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "auto",
    about = "A tool for automatically running task in the right order."
)]
struct Opt {
    /// Input file
    #[structopt(parse(from_os_str), default_value = "Autofile.toml")]
    autofile: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    if let Err(err) = run(opt) {
        eprintln!("{}{}{}", Color::Red.prefix(), err, Color::Red.suffix());
        std::process::exit(1);
    }
}

fn run(opt: Opt) -> Result<()> {
    let source = std::fs::read_to_string(&opt.autofile).context(LoadConfig {
        path: opt.autofile.clone(),
    })?;
    let autofile: autofile::AutoFile =
        toml::from_str(&source).context(ParseConfig { path: opt.autofile })?;

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

    let (sender, receiver) = mpsc::channel::<TaskMessage>();

    let mut running = HashSet::new();

    loop {
        while let Some(task) = plan.pop_available() {
            eprintln!("{} {}", Color::Blue.bold().paint("Running"), task.id);
            running.insert(task.id.clone());

            std::thread::spawn({
                let sender = sender.clone();
                let outdir_path = outdir.path().to_path_buf();
                move || {
                    let start_time = std::time::Instant::now();

                    let outcome = match run_task(&outdir_path, &task) {
                        Ok(true) => TaskOutcome::Success,
                        Ok(false) => TaskOutcome::Failed,
                        Err(err) => TaskOutcome::Error(err),
                    };

                    let duration = start_time.elapsed();
                    sender.send(TaskMessage {
                        task,
                        outcome,
                        duration,
                    })
                }
            });
        }

        if running.is_empty() {
            break;
        } else {
            match receiver.recv() {
                Ok(result) => {
                    running.remove(&result.task.id);
                    match result.outcome {
                        TaskOutcome::Success => {
                            eprintln!(
                                "{} {} (duration {:.2}s)",
                                Color::Green.bold().paint("Success"),
                                result.task.id,
                                result.duration.as_secs_f64()
                            );
                            plan.mark_done(&result.task.id);
                            successful.push(result.task);
                        }
                        TaskOutcome::Failed => {
                            eprintln!(
                                "{} {} (duration {:.2}s)",
                                Color::Red.bold().paint("Failure"),
                                result.task.id,
                                result.duration.as_secs_f64()
                            );
                            failed.push(result.task);
                        }
                        TaskOutcome::Error(err) => {
                            eprintln!(
                                "{} {}: {} (duration {:.2}s)",
                                Color::Red.bold().paint("  Error"),
                                result.task.id,
                                err,
                                result.duration.as_secs_f64()
                            );
                            failed.push(result.task);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("{}: {}", Color::Red.bold().paint("Thread Failure"), err);
                    break;
                }
            }
        }
    }

    let not_started = plan.give_up();
    for remaining in &not_started {
        eprintln!(
            "{} {}",
            Color::Red.bold().paint("not running"),
            remaining.id
        );
    }

    eprintln!(
        "{} successful, {} failed, {} not started",
        successful.len(),
        failed.len(),
        not_started.len()
    );

    Ok(())
}

fn run_task(outdir: &Path, task: &queue::Task<Cmd>) -> Result<bool> {
    // Create files for redirecting output
    let task_stdout_path = outdir.join(&task.id.as_str()).with_extension("stdout");
    let task_stderr_path = outdir.join(&task.id.as_str()).with_extension("stderr");
    let task_stdout = std::fs::File::create(task_stdout_path).context(Temp)?;
    let task_stderr = std::fs::File::create(task_stderr_path).context(Temp)?;

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

    Ok(status.success())
}

struct TaskMessage {
    task: queue::Task<Cmd>,
    outcome: TaskOutcome,
    duration: std::time::Duration,
}

enum TaskOutcome {
    Success,
    Failed,
    Error(Error),
}

#[derive(Debug)]
pub struct Cmd {
    pub program: OsString,
    pub arguments: Vec<OsString>,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not load config {}: {}", path.display(), source))]
    LoadConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Could not parse config {}: {}", path.display(), source))]
    ParseConfig {
        path: PathBuf,
        source: toml::de::Error,
    },

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

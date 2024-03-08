use std::{
    fs::{read_to_string, File, OpenOptions},
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context};

use futures::future::join_all;
use serde::Deserialize;
use tokio::{process::Command, time::sleep};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, Deserialize)]
struct Task {
    command: String,
    duration: String,
}

#[derive(Debug, Deserialize)]
struct Tasks {
    entries: Vec<Task>,
}

fn parse_time(input: &str) -> Duration {
    let mut input = input.split(':');
    let hours: u64 = input.next().unwrap().parse().unwrap();
    let mins: u64 = input.next().unwrap().parse().unwrap();
    let secs: u64 = input.next().unwrap().parse().unwrap();
    assert!(input.next().is_none(), "bad format");
    Duration::from_secs(hours * 3600 + mins * 60 + secs)
}

async fn task_loop(Tasks { entries }: Tasks) {
    let mut handles = vec![];
    for Task { command, duration } in entries {
        let duration = parse_time(&duration);
        let t = tokio::spawn(async move {
            loop {
                _ = Command::new("pwsh.exe")
                    .arg("-Command")
                    .arg(&command)
                    .spawn()
                    .unwrap()
                    .wait()
                    .await;
                sleep(duration).await;
            }
        });
        handles.push(t);
    }
    join_all(handles.into_iter()).await;
}

async fn cron_loop(base: &Path) {
    let file = read_to_string(base).unwrap();
    let tasks: Tasks = toml::from_str(&file).unwrap();

    task_loop(tasks).await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let home = homedir::get_my_home()?.context("No homedir")?;
    let mut base = home.join(".crond.toml");
    if !base.exists() {
        base = home.join(".config").join("crond.toml");
    }

    if !base.exists() {
        Err(anyhow!(
            "Error: Expected file {:?} or {:?}",
            home.join(".crond"),
            base
        ))?
    }
    if base.is_dir() {
        Err(anyhow!("Error: {base:?} is a dir",))?
    }

    let stdout_log = tracing_subscriber::fmt::layer().pretty();
    let subscriber = tracing_subscriber::Registry::default().with(stdout_log);

    let logfile = base.parent().unwrap().join("crond.log");
    let logfile = if logfile.exists() {
        OpenOptions::new().append(true).open(logfile)
    } else {
        File::create(logfile)
    }?;

    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(Arc::new(logfile));
    let subscriber = subscriber.with(layer);
    tracing::subscriber::set_global_default(subscriber)?;

    cron_loop(&base).await;
    Ok(())
}

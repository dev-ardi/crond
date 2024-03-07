// #[derive(Parser)]
// struct Cli {}

use std::{
    fs::{File, OpenOptions},
    path::Path,
    process::Output,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context};

use futures::future::join_all;
use tokio::{process::Command, time::sleep};
use tracing::{error, info, instrument, trace, Level};
use tracing_subscriber::layer::SubscriberExt;

struct Config {}

#[instrument]
async fn loop_folder(base: &Path, folder_name: &str, duration: Duration) -> () {
    let name = base.join(folder_name);
    loop {
        let span = tracing::span!(Level::INFO, "loop_folder", frequency = folder_name);
        let guard = span.enter();
        trace!("reading {folder_name}");
        let files = walkdir::WalkDir::new(&name).into_iter();

        let mut futs = vec![];
        for file in files {
            let file = match file {
                Ok(e) => e,
                Err(e) => {
                    error!("error reading file: {e}");
                    continue;
                }
            };
            if !file.file_type().is_file() {
                continue;
            }
            let file = file.path().to_owned();

            let cmd;
            cfg_if::cfg_if! {
                if #[cfg(windows)] {
                    cmd = Command::new("pwsh.exe")
                        .arg("-File")
                        .arg(file.as_os_str())
                        .spawn();
                    trace!("{cmd:?}");
                } else {
                    todo!("untested on other platforms, should respect shebang");
                     cmd = Command::new(&file).spawn();
                }
            };

            let child = match cmd {
                Ok(e) => e,
                Err(e) => {
                    error!("error executing {:?}: {e}", file);
                    continue;
                }
            };

            futs.push(async move { (child.wait_with_output().await, file) });
        }
        drop(guard);

        let futs = join_all(futs).await;
        for (res, path) in futs {
            let path = path.as_os_str().to_str().unwrap_or("");
            let span = tracing::span!(Level::INFO, "loop_folder", file_name = path);
            _ = span.enter();

            let Output {
                status,
                stdout,
                stderr,
            } = match res {
                Ok(e) => e,
                Err(e) => {
                    error!("error executing {path:?}: {e}");
                    continue;
                }
            };
            let stdout = String::from_utf8_lossy(&stdout);
            let stderr = String::from_utf8_lossy(&stderr);
            if status.code().unwrap_or(1) == 0 {
                info!("{stdout}");
                info!("{stderr}");
            } else {
                error!("{stdout}");
                error!("{stderr}");
            }
        }

        sleep(duration).await;
    }
}

async fn cron_loop(base: &Path) {
    let proms = vec![
        loop_folder(base, "second", Duration::from_secs(1)),
        loop_folder(base, "minute", Duration::from_secs(60)),
        loop_folder(base, "hour", Duration::from_secs(60 * 60)),
        loop_folder(base, "day", Duration::from_secs(60 * 60 * 24)),
        loop_folder(base, "week", Duration::from_secs(60 * 60 * 24 * 7)),
    ];
    join_all(proms).await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let home = homedir::get_my_home()?.context("No homedir")?;
    let mut base = home.join(".crond");
    if !base.exists() {
        base = home.join(".config").join("crond");
    }

    if !base.exists() {
        Err(anyhow!(
            "Error: Expected folder {:?} or {:?}",
            home.join(".crond"),
            base
        ))?
    }
    if base.is_file() {
        Err(anyhow!("Error: {base:?} is a file",))?
    }

    let stdout_log = tracing_subscriber::fmt::layer().pretty();
    let subscriber = tracing_subscriber::Registry::default().with(stdout_log);

    let logfile = base.join("logfile.txt");
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

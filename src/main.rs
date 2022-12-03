mod args;
mod report;

use std::{
    io::{self, Write},
    process::ExitCode,
};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use tracing::{debug, error};
use tracing_subscriber::EnvFilter;

use crate::{args::Arguments, report::Report};

fn run() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::try_new("pkgdu=warn")?);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_writer(io::stderr)
        .try_init()
        .map_err(|err| anyhow!("{err:#}"))
        .context("Failed to initialize tracing subscriber")?;

    let arguments = Arguments::parse();
    debug!("Run with {:?}", arguments);

    let mut report = Report::new(
        arguments.pkgname_regex,
        arguments.sort,
        arguments.description,
        arguments.total,
        arguments.quiet,
    );
    report
        .build()
        .context("Failed to calculate sum of file sizes for each installed packages")?;

    let mut stdout = io::BufWriter::new(io::stdout().lock());
    if let Err(err) = writeln!(stdout, "{report}").context("Could not write report to STDOUT") {
        if let Some(io_err) = err.downcast_ref::<io::Error>() {
            match io_err.kind() {
                io::ErrorKind::BrokenPipe => return Ok(()),
                _ => bail!("{err:#}"),
            }
        } else {
            bail!("{err:#}");
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    if let Err(err) = run() {
        error!("{err:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

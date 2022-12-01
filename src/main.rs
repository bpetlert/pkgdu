use std::{
    cmp::Reverse,
    io::{self, Write},
};

use alpm::Alpm;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use humansize::{format_size_i, FormatSizeOptions, DECIMAL};
use pacmanconf::Config;
use tabled::{
    object::{Columns, Rows},
    Alignment, Disable, Modify, Style, Table, Tabled,
};
use tracing::debug;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Change the default column to sort on
    #[arg(
        long,
        value_enum,
        ignore_case = true,
        default_value_t = SortColumn::InstalledSizeDescending
    )]
    sort: SortColumn,

    #[arg(long)]
    show_description: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum SortColumn {
    #[value(name = "name-asc")]
    NameAscending,

    #[value(name = "name-desc")]
    NameDescending,

    #[value(name = "installed-size-asc")]
    InstalledSizeAscending,

    #[value(name = "installed-size-desc")]
    InstalledSizeDescending,
}

#[derive(Debug)]
struct Report {
    pkgs: Vec<PkgDiskUsage>,
    sort: SortColumn,
    show_description: bool,
}

#[derive(Debug, Tabled)]
struct PkgDiskUsage {
    #[tabled(rename = "Name", order = 1)]
    name: String,

    #[tabled(rename = "Installed Size", order = 0)]
    installed_size: FileSize,

    #[tabled(rename = "Description", order = 2)]
    description: String,
}

#[derive(Debug)]
struct FileSize(i64);

impl Report {
    pub fn new(sort: SortColumn, show_description: bool) -> Self {
        Self {
            pkgs: Vec::new(),
            sort,
            show_description,
        }
    }

    pub fn build(&mut self) -> Result<()> {
        let alpm = {
            let pacman_conf = Config::new().context("Failed to load `pacman.conf`")?;
            Alpm::new(pacman_conf.root_dir, pacman_conf.db_path).context("Could not access ALPM")?
        };

        self.pkgs = alpm
            .localdb()
            .pkgs()
            .iter()
            .map(|pkg| PkgDiskUsage {
                installed_size: FileSize(pkg.isize()),
                name: pkg.name().to_owned(),
                description: pkg.desc().unwrap_or("").to_owned(),
            })
            .collect();

        match self.sort {
            SortColumn::NameAscending => self.pkgs.sort_by_key(|k| k.name.clone()),
            SortColumn::NameDescending => self.pkgs.sort_by_key(|k| Reverse(k.name.clone())),
            SortColumn::InstalledSizeAscending => self.pkgs.sort_by_key(|k| k.installed_size.0),
            SortColumn::InstalledSizeDescending => {
                self.pkgs.sort_by_key(|k| Reverse(k.installed_size.0))
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut table = Table::new(&self.pkgs);
        table
            .with(Style::blank())
            .with(Disable::row(Rows::first())) // No headers
            .with(Modify::new(Columns::new(..)).with(Alignment::left()));

        if !self.show_description {
            // FIXME: Why using ByColumnName with #[tabled(rename = "Description", order = 2)] does not work?
            // table.with(Disable::column(ByColumnName::new("Description")));
            table.with(Disable::column(Columns::single(2)));
        }

        write!(f, "{table}")
    }
}

impl std::fmt::Display for FileSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let fmt = FormatSizeOptions::from(DECIMAL).space_after_value(true);
        write!(f, "{}", format_size_i(self.0, fmt))
    }
}

fn main() -> Result<()> {
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

    let mut report = Report::new(arguments.sort, arguments.show_description);
    report.build()?;

    let mut stdout = io::BufWriter::new(io::stdout().lock());
    writeln!(stdout, "{report}")?;

    Ok(())
}

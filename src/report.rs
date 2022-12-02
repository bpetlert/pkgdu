use std::cmp::Reverse;

use alpm::Alpm;
use anyhow::{Context, Result};
use humansize::{format_size_i, FormatSizeOptions, DECIMAL};
use pacmanconf::Config;
use tabled::{
    object::{Columns, Rows},
    Alignment, Disable, Modify, Style, Table, Tabled,
};

use crate::args::SortColumn;

#[derive(Debug)]
pub struct Report {
    pkgs: Vec<PkgDiskUsage>,
    sort: SortColumn,
    show_description: bool,
}

#[derive(Debug, Tabled)]
pub struct PkgDiskUsage {
    #[tabled(rename = "Name", order = 1)]
    name: String,

    #[tabled(rename = "Installed Size", order = 0)]
    installed_size: FileSize,

    #[tabled(rename = "Description", order = 2)]
    description: String,
}

#[derive(Debug)]
pub struct FileSize(i64);

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
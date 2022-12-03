use std::cmp::Reverse;

use alpm::{Alpm, Package};
use anyhow::{anyhow, Context, Result};
use humansize::{format_size_i, FormatSizeOptions, DECIMAL};
use pacmanconf::Config;
use regex::{Regex, RegexSet};
use tabled::{
    object::{Columns, Rows},
    Alignment, Disable, Modify, Style, Table, Tabled,
};

use crate::args::SortColumn;

#[derive(Debug)]
pub struct Report {
    pkgs: Vec<PkgDiskUsage>,

    pkgname_pattern: Option<String>,
    exclude_pattern: Option<Vec<String>>,
    sort: SortColumn,
    description: bool,
    total: bool,
    quiet: bool,
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
    pub fn new(
        pkgname_pattern: Option<String>,
        exclude_pattern: Option<Vec<String>>,
        sort: SortColumn,
        description: bool,
        total: bool,
        quiet: bool,
    ) -> Self {
        Self {
            pkgname_pattern,
            exclude_pattern,
            pkgs: Vec::new(),
            sort,
            description,
            total,
            quiet,
        }
    }

    pub fn build(&mut self) -> Result<()> {
        let alpm = {
            let pacman_conf = Config::new().context("Failed to load `pacman.conf`")?;
            Alpm::new(pacman_conf.root_dir, pacman_conf.db_path).context("Could not access ALPM")?
        };

        // Apply PKGNAME_PATTERN
        let mut installed_pkgs: Vec<Package> = match &self.pkgname_pattern {
            Some(pkgname_regex) => {
                let pkgname_filter: &Regex = {
                    static RE: once_cell::sync::OnceCell<regex::Regex> =
                        once_cell::sync::OnceCell::new();
                    RE.get_or_try_init(|| regex::Regex::new(pkgname_regex))
                        .map_err(|err| anyhow!("{err:#?}"))
                        .context("Failed to crate package name filter")?
                };

                alpm.localdb()
                    .pkgs()
                    .iter()
                    .filter(|pkg| pkgname_filter.is_match(pkg.name()))
                    .collect()
            }
            None => alpm.localdb().pkgs().iter().collect(),
        };

        // Apply EXCLUDE_PATTERN
        if let Some(exclude_regex) = &self.exclude_pattern {
            let exclude_filter_set: &RegexSet = {
                static RE: once_cell::sync::OnceCell<regex::RegexSet> =
                    once_cell::sync::OnceCell::new();
                RE.get_or_try_init(|| regex::RegexSet::new(exclude_regex))
                    .map_err(|err| anyhow!("{err:#?}"))
                    .context("Failed to crate exclude filter")?
            };

            installed_pkgs.retain(|pkg| !exclude_filter_set.is_match(pkg.name()));
        }

        // Load installed packages' info
        self.pkgs = installed_pkgs
            .iter()
            .map(|pkg| PkgDiskUsage {
                installed_size: FileSize(pkg.isize()),
                name: pkg.name().to_owned(),
                description: pkg.desc().unwrap_or("").to_owned(),
            })
            .collect();

        let total_size: i64 = self.pkgs.iter().map(|pkg| pkg.installed_size.0).sum();

        // Sort report
        match self.sort {
            SortColumn::NameAscending => self.pkgs.sort_by_key(|k| k.name.clone()),
            SortColumn::NameDescending => self.pkgs.sort_by_key(|k| Reverse(k.name.clone())),
            SortColumn::InstalledSizeAscending => self.pkgs.sort_by_key(|k| k.installed_size.0),
            SortColumn::InstalledSizeDescending => {
                self.pkgs.sort_by_key(|k| Reverse(k.installed_size.0))
            }
        }

        // Quiet option
        if self.quiet {
            self.pkgs = Vec::new();
        }

        // Add a grand total
        if self.total {
            self.pkgs.push(PkgDiskUsage {
                name: "(TOTAL)".to_string(),
                installed_size: FileSize(total_size),
                description: "".to_string(),
            });
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

        if !self.description {
            // RESEARCH: Why using ByColumnName with #[tabled(rename = "Description", order = 2)] does not work?
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

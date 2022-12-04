use std::{cmp::Reverse, collections::HashMap};

use alpm::{Alpm, Dep};
use anyhow::{anyhow, bail, Context, Result};
use humansize::{format_size_i, FormatSizeOptions, DECIMAL};
use pacmanconf::Config;
use regex::{Regex, RegexSet};
use tabled::{
    object::{Columns, Rows},
    Alignment, Disable, Modify, Style, Table, Tabled,
};
use tracing::{debug, warn};

use crate::args::SortColumn;

#[derive(Debug)]
pub struct Report {
    pkgs: Vec<PkgDiskUsage>,

    pkgname_pattern: Option<String>,
    exclude_pattern: Option<Vec<String>>,
    recursive_depends_on: bool,
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
        recursive_depends_on: bool,
        sort: SortColumn,
        description: bool,
        total: bool,
        quiet: bool,
    ) -> Self {
        Self {
            pkgname_pattern,
            exclude_pattern,
            recursive_depends_on,
            pkgs: Vec::new(),
            sort,
            description,
            total: quiet || total,
            quiet,
        }
    }

    pub fn build(&mut self) -> Result<()> {
        let alpm = {
            let pacman_conf = Config::new().context("Failed to load `pacman.conf`")?;
            Alpm::new(pacman_conf.root_dir, pacman_conf.db_path).context("Could not access ALPM")?
        };

        // Apply PKGNAME_PATTERN
        let mut installed_pkgs: Vec<String> = match &self.pkgname_pattern {
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
                    .map(|pkg| pkg.name().to_owned())
                    .collect()
            }
            None => alpm
                .localdb()
                .pkgs()
                .iter()
                .map(|pkg| pkg.name().to_owned())
                .collect(),
        };

        // Include all package dependencies required by the matching packages.
        if self.recursive_depends_on && self.pkgname_pattern.is_some() {
            let mut visited_deps: HashMap<String, bool> = installed_pkgs
                .iter()
                .map(|name| (name.clone(), false))
                .collect();

            loop {
                if visited_deps.iter().all(|(_, &visited)| visited) {
                    // All deps are checked
                    break;
                }

                let mut new_deps = visited_deps.clone();
                for (name, visited) in &visited_deps {
                    if *visited {
                        continue;
                    }

                    let pkg = match alpm.localdb().pkg(&**name) {
                        Ok(pkg) => pkg,
                        Err(err) => {
                            warn!("Failed to get info of package `{name}`: {err:#}");
                            continue;
                        }
                    };

                    // Solve all deps in "Depends On" field
                    let deps = pkg.depends();
                    for dep in deps {
                        match self.solve_dep(&alpm, &dep) {
                            Ok(pkg_name) => match new_deps.entry(pkg_name) {
                                std::collections::hash_map::Entry::Occupied(mut e) => {
                                    e.insert(true);
                                }
                                std::collections::hash_map::Entry::Vacant(e) => {
                                    e.insert(false);
                                }
                            },
                            Err(err) => {
                                warn!("Failed to solve dependency of `{}`: {err:#}", dep.name())
                            }
                        }
                    }

                    // Mark as visited
                    new_deps.insert(pkg.name().to_string(), true);
                }
                visited_deps = new_deps;
            }

            installed_pkgs = visited_deps.into_iter().map(|(name, _)| name).collect();
        }

        // Apply EXCLUDE_PATTERN
        if let Some(exclude_regex) = &self.exclude_pattern {
            let exclude_filter_set: &RegexSet = {
                static RE: once_cell::sync::OnceCell<regex::RegexSet> =
                    once_cell::sync::OnceCell::new();
                RE.get_or_try_init(|| regex::RegexSet::new(exclude_regex))
                    .map_err(|err| anyhow!("{err:#?}"))
                    .context("Failed to crate exclude filter")?
            };

            installed_pkgs.retain(|name| !exclude_filter_set.is_match(name));
        }

        // Load installed packages' info
        self.pkgs = installed_pkgs
            .into_iter()
            .filter_map(|name| alpm.localdb().pkg(name).ok())
            .map(|pkg| {
                let description = if !self.description {
                    "".to_string()
                } else {
                    pkg.desc().unwrap_or("").to_owned()
                };

                PkgDiskUsage {
                    installed_size: FileSize(pkg.isize()),
                    name: pkg.name().to_owned(),
                    description,
                }
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

    fn solve_dep(&self, alpm: &Alpm, dep: &Dep) -> Result<String> {
        debug!("Try to solve dependency of `{dep:?}` using `Provides` field's name hash");
        let name_hash = dep.name_hash();
        if let Some(pkg) = alpm
            .localdb()
            .pkgs()
            .iter()
            .find(|pkg| pkg.provides().iter().any(|d| d.name_hash() == name_hash))
        {
            debug!("Found dependency package => `{}`", pkg.name());
            return Ok(pkg.name().to_string());
        } else {
            debug!("Cannot find dependency in `Provides` field");
        }

        debug!("Try to solve dependency of `{dep:?}` using `Name` field");
        match alpm.localdb().pkg(dep.name()) {
            Ok(pkg) => {
                debug!("Found dependency package => `{}`", pkg.name());
                return Ok(pkg.name().to_string());
            }
            Err(err) => debug!("Cannot find dependency in `Name` field: {err:#}"),
        };

        bail!("Cannot solve dependency of `{}`", dep.name());
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

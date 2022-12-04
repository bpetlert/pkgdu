use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Specify the regular expression for matching package name.
    #[arg()]
    pub pkgname_pattern: Option<String>,

    /// Specify the regular expression for excluding package name.
    #[arg(long = "exclude", value_name = "PATTERN")]
    pub exclude_pattern: Option<Vec<String>>,

    /// Include all package dependencies required by the matching packages.
    #[arg(long)]
    pub recursive_depends_on: bool,

    /// Change the default column to sort on
    #[arg(
        long,
        value_name = "COLUMN-[ASC|DESC]",
        value_enum,
        ignore_case = true,
        default_value_t = SortColumn::InstalledSizeDescending
    )]
    pub sort: SortColumn,

    /// Show package's description in report.
    #[arg(long)]
    pub description: bool,

    /// Display a grand total
    #[arg(long, default_value_if("quiet", "true", "true"))]
    pub total: bool,

    /// Show only a grand total. Do not show package's size report.
    #[arg(long, conflicts_with_all = ["sort", "description", "total"])]
    pub quiet: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SortColumn {
    #[value(name = "name-asc")]
    NameAscending,

    #[value(name = "name-desc")]
    NameDescending,

    #[value(name = "installed-size-asc")]
    InstalledSizeAscending,

    #[value(name = "installed-size-desc")]
    InstalledSizeDescending,
}

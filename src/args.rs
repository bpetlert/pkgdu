use clap::Parser;

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
    pub sort: SortColumn,

    #[arg(long)]
    pub show_description: bool,

    /// Display a grand total
    #[arg(long)]
    pub total: bool,
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

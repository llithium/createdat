use clap::Parser;

/// Rename images with the date they were created
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Optional prefix for renamed images
    pub name: Option<String>,

    /// Choose which files to rename based on file extension
    #[arg(short, long)]
    pub extension: bool,

    /// Put date in front of filename
    #[arg(short, long)]
    pub front: bool,

    /// Remove original filename
    #[arg(short, long)]
    pub no_name: bool,

    /// Use 12-hour time format instead of 24-hour
    #[arg(short, long)]
    pub twelve: bool,

    /// Date without time
    #[arg(short, long)]
    pub date: bool,

    /// Use spaces instead of underscore under scores in name
    #[arg(long)]
    pub space: bool,

    /// Set custom date format to use ('%a %b %e %Y' = "Wed Jul 17 2024")
    #[arg(long, value_name = "Format")]
    pub format: Option<String>,

    /// Set the source folder for images
    #[arg(short = 'S', long, value_name = "Path")]
    pub source: Option<String>,

    /// Set the target folder for renamed images (default: renamed)
    #[arg(short = 'T', long, value_name = "Path")]
    pub target: Option<String>,

    /// Put custom name after the date
    #[arg(short, long)]
    pub suffix: bool,

    /// Preview the name format of renamed files
    #[arg(short, long)]
    pub preview: bool,

    /// Rename all files, not just images
    #[arg(short, long)]
    pub all: bool,
}

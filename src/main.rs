mod cli;
mod test;

use std::{
    env::current_dir,
    ffi::OsStr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local};
use clap::Parser;
use cli::Cli;
use inquire::{
    ui::{RenderConfig, Styled},
    MultiSelect,
};
use mime_guess::Mime;
use owo_colors::OwoColorize;
use tokio::{
    fs::{self, create_dir_all, read_dir, remove_dir, remove_dir_all, DirEntry, ReadDir},
    sync::{Mutex, Semaphore},
    task::JoinHandle,
};

static PERMITS: Semaphore = Semaphore::const_new(15);

struct CurrentFile {
    user_added_name: String,
    original_name: String,
}

#[derive(Debug, Clone, Copy)]
struct FileCount {
    renamed: u32,
    total: u32,
    duplicate: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    inquire::set_global_render_config(get_render_config());
    let cli = Arc::new(Cli::parse());
    let source_folder: PathBuf = if let Some(name) = cli.source.as_deref() {
        PathBuf::from(name.trim())
    } else {
        current_dir()?
    };
    let renamed_folder: PathBuf = if let Some(name) = cli.folder.as_deref() {
        PathBuf::from(name.trim())
    } else {
        PathBuf::from("renamed")
    };
    let source_folder = Arc::new(source_folder);

    let mut files = match read_dir(source_folder.as_ref()).await {
        Ok(files) => files,
        Err(err) => {
            eprintln!(
                "{} {}",
                " ERROR READING DIRECTORY ".black().on_red(),
                err.on_red()
            );
            return Err(err.into());
        }
    };
    let extension_selections = if cli.extension {
        if let Ok(selections) = get_extensions(&mut files).await {
            selections
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let start_time = SystemTime::now();

    if !cli.preview {
        if renamed_folder.exists() && renamed_folder.read_dir()?.next().is_some() {
            {
                eprintln!("Folder {:?} already exists", renamed_folder);
                return Ok(());
            }
        }
        if let Err(err) = create_dir_all(renamed_folder.clone()).await {
            eprintln!(
                "{} {}",
                " ERROR CREATING DIRECTORY ".black().on_red(),
                err.red()
            );
            return Err(err.into());
        }
    }
    let renamed_folder = Arc::new(renamed_folder);

    let file_count = match copy_files(
        files,
        cli.clone(),
        renamed_folder.clone(),
        extension_selections,
    )
    .await
    {
        Ok(file_count) => match file_count.duplicate {
            0 => file_count,
            count if count > 1 => {
                eprintln!(
                        "{} Duplicate creation times found. Files would be overwritten with current options",
                        " ERROR ".black().on_red(),
                    );
                remove_dir_all(renamed_folder.as_ref()).await?;
                return Ok(());
            }
            1 => {
                eprintln!(
                        "{} Duplicate creation time found. Files would be overwritten with current options",
                        " ERROR ".black().on_red(),
                    );
                remove_dir_all(renamed_folder.as_ref()).await?;
                return Ok(());
            }
            _ => return Ok(()),
        },
        Err(err) => return Err(err),
    };
    if cli.preview {
        if file_count.duplicate > 0 {
            println!(
                "{} Files would be overwritten with the current options.",
                " ERROR ".black().on_red(),
            );
        }
        return Ok(());
    }

    print_summary(start_time, file_count, renamed_folder, cli).await
}

async fn copy_files(
    mut files: ReadDir,
    cli: Arc<Cli>,
    renamed_folder: Arc<PathBuf>,
    extension_selections: Vec<String>,
) -> Result<FileCount> {
    let file_count = Arc::new(Mutex::new(FileCount {
        renamed: 0,
        total: 0,
        duplicate: 0,
    }));
    let mut tasks: Vec<JoinHandle<()>> = vec![];

    while let Ok(Some(file)) = files.next_entry().await {
        let extension_selections = extension_selections.clone();
        let current_file = CurrentFile {
            user_added_name: String::new(),
            original_name: String::new(),
        };
        let file_count = file_count.clone();
        let renamed_folder = renamed_folder.clone();
        let cli = cli.clone();
        let task = tokio::task::spawn(async move {
            let _permit = PERMITS.acquire().await.unwrap();
            let file_path = file.path();

            if file.metadata().await.unwrap().is_dir() {
                return;
            }

            let image_destination = if let Ok(img) = get_image_destination(
                cli.clone(),
                &file,
                current_file,
                extension_selections,
                &file_path,
                &mut *file_count.lock().await,
                renamed_folder,
            )
            .await
            {
                img
            } else {
                return;
            };
            if cli.preview {
                println!("{image_destination:?}");
                return;
            }

            if Path::new(&image_destination).exists() {
                file_count.lock().await.duplicate += 1;
                return;
            }
            if file_count.lock().await.duplicate > 0 {
                return;
            }

            let max_retries: u8 = 3;
            let retry_delay_ms: u64 = 100;
            let mut attempt: u8 = 0;

            loop {
                let copy_result = fs::copy(file.path(), image_destination.clone()).await;
                if copy_result.is_ok() {
                    file_count.lock().await.renamed += 1;
                    break;
                } else {
                    attempt += 1;
                    if attempt >= max_retries {
                        eprintln!(
                            "{}{}{:?}",
                            " ERROR ".black().on_red(),
                            " Max retries reached. Skipping file: ".red(),
                            file.path().red()
                        );
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                }
            }
        });
        tasks.push(task);
    }
    for task in tasks {
        task.await?;
    }
    let file_count = *file_count.lock().await;
    Ok(file_count)
}

async fn print_summary(
    start_time: SystemTime,
    file_count: FileCount,
    renamed_folder: Arc<PathBuf>,
    cli: Arc<Cli>,
) -> Result<()> {
    if file_count.renamed == 0 {
        remove_dir(renamed_folder.as_ref()).await?;
        if cli.extension {
            eprintln!("No files selected");
            return Ok(());
        }
        if cli.all {
            eprintln!("No files found");
        } else {
            eprintln!(
                "No images found. (Use '{}' or '{}' to rename any files found)",
                "-a".yellow(),
                "--all".yellow()
            );
        }
        return Ok(());
    }

    let end_time = start_time.elapsed().unwrap_or_else(|err| {
        eprintln!("Error calculating time{err}");
        std::time::Duration::default()
    });
    if cli.all || cli.extension {
        if file_count.renamed == file_count.total {
            Ok(println!(
                "{}{}{}{}{}{}{:?}",
                " ".on_green(),
                file_count.renamed.black().on_green(),
                "/".black().on_green(),
                file_count.total.black().on_green(),
                " ".on_green(),
                " Files renamed in ".green(),
                end_time.green()
            ))
        } else {
            Ok(println!(
                "{}/{} Files renamed in {:?}",
                file_count.renamed, file_count.total, end_time
            ))
        }
    } else if file_count.renamed == file_count.total {
        Ok(println!(
            "{}{}{}{}{}{}{:?}",
            " ".on_green(),
            file_count.renamed.black().on_green(),
            "/".black().on_green(),
            file_count.total.black().on_green(),
            " ".on_green(),
            " Images renamed in ".green(),
            end_time.green()
        ))
    } else {
        Ok(println!(
            "{}/{} Images renamed in {:?}",
            file_count.renamed, file_count.total, end_time
        ))
    }
}

async fn get_extensions(files: &mut ReadDir) -> Result<Vec<String>> {
    let mut file_extension_options: Vec<String> = vec![];
    while let Ok(Some(file)) = files.next_entry().await {
        let file_path = file.path();

        if file.metadata().await?.is_dir() {
            continue;
        }

        let Ok(file_name) = file.file_name().into_string() else {
            eprintln!(
                "{} Failed converting file name to string {:?}. File skipped",
                " ERROR ".black().on_red(),
                file_path
            );
            continue;
        };

        let file_extension = if file_name.starts_with('.') {
            if let Some(extension) = file_name.strip_prefix('.') {
                extension
            } else {
                eprintln!(
                    "{} Failed to get file extension from {}. File skipped",
                    " ERROR ".black().on_red(),
                    file_name
                );
                continue;
            }
        } else {
            Path::new(&file_path)
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or_default()
        };
        if file_extension_options.contains(&file_extension.to_owned()) {
            continue;
        } else {
            file_extension_options.push(String::from(file_extension));
        }
    }

    match MultiSelect::new("Select file types to rename:", file_extension_options).prompt() {
        Ok(selections) => Ok(selections),
        Err(err) => Err(err.into()),
    }
}

async fn format_time(cli: Arc<Cli>, file: &DirEntry) -> Result<String> {
    let file_modified_at_system_time = file.metadata().await?.modified()?;
    let file_modified_at_date_time: DateTime<Local> = file_modified_at_system_time.into();
    if let Some(format) = &cli.format {
        Ok(sanitize_filename::sanitize(
            file_modified_at_date_time.format(format).to_string(),
        ))
    } else if cli.date {
        Ok(file_modified_at_date_time.format("%Y-%m-%d").to_string())
    } else if cli.twelve {
        Ok(file_modified_at_date_time
            .format("%Y-%m-%d %I_%M_%S %p")
            .to_string())
    } else {
        Ok(file_modified_at_date_time
            .format("%Y-%m-%d %H_%M_%S")
            .to_string())
    }
}

async fn get_image_destination(
    cli: Arc<Cli>,
    file: &DirEntry,
    mut current_file: CurrentFile,
    extension_selections: Vec<String>,
    file_path: &Path,
    file_count: &mut FileCount,
    renamed_folder: Arc<PathBuf>,
) -> Result<PathBuf> {
    let Ok(file_name_with_extension) = file.file_name().into_string() else {
        return Err(anyhow!(
            "{}{}{:?}",
            " ERROR ".black().on_red(),
            " converting file name to string ".red(),
            file_path.red(),
        ));
    };
    let mut dotfile = false;
    let file_extension = if file_name_with_extension.starts_with('.') {
        if let Some(extension) = file_name_with_extension.strip_prefix('.') {
            dotfile = true;
            extension
        } else {
            {
                return Err(anyhow!(
                    "{}{}{}",
                    "Error getting file extension from ".red(),
                    file_name_with_extension.red(),
                    ". File skipped".red()
                ));
            }
        }
    } else {
        Path::new(&file_path)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
    };
    if cli.extension && !extension_selections.contains(&file_extension.to_owned()) {
        bail!("");
    }
    if !cli.all
        && !cli.extension
        && !mime_guess::from_path(file_path)
            .first()
            .unwrap_or(Mime::from_str("unkown/unknown")?)
            .to_string()
            .starts_with("image")
    {
        bail!("")
    }
    file_count.total += 1;

    if let Some(entered_prefix) = cli.name.as_deref() {
        current_file.user_added_name = if cli.front {
            " ".to_owned()
                + &sanitize_filename::sanitize(String::from(entered_prefix).trim()).clone()
        } else if cli.suffix {
            " ".to_owned() + &sanitize_filename::sanitize(String::from(entered_prefix).trim())
        } else {
            sanitize_filename::sanitize(String::from(entered_prefix).trim()).clone() + " "
        }
    }
    if !cli.no_name {
        current_file.original_name = if cli.front && dotfile {
            file_name_with_extension
                .strip_suffix(&format!(".{file_extension}"))
                .unwrap_or_default()
                .to_string()
        } else if cli.front {
            " ".to_owned()
                + file_name_with_extension
                    .strip_suffix(&format!(".{file_extension}"))
                    .unwrap_or_default()
        } else if dotfile {
            file_name_with_extension
                .strip_suffix(&format!(".{file_extension}"))
                .unwrap_or_default()
                .to_string()
        } else {
            file_name_with_extension
                .strip_suffix(&format!(".{file_extension}"))
                .unwrap_or_default()
                .to_string()
                + " "
        }
    }
    let image_modified_at_time = format_time(cli.clone(), file).await?;
    let image_destination = if cli.suffix {
        if cli.front {
            renamed_folder.join(format!(
                "{}{}{}.{}",
                image_modified_at_time,
                current_file.original_name,
                current_file.user_added_name.trim_end(),
                file_extension
            ))
        } else {
            renamed_folder.join(format!(
                "{}{}{}.{}",
                current_file.original_name,
                image_modified_at_time,
                current_file.user_added_name.trim_end(),
                file_extension
            ))
        }
    } else if cli.front {
        renamed_folder.join(format!(
            "{}{}{}.{}",
            image_modified_at_time,
            current_file.user_added_name,
            current_file.original_name,
            file_extension
        ))
    } else {
        renamed_folder.join(format!(
            "{}{}{}.{}",
            current_file.user_added_name,
            current_file.original_name,
            image_modified_at_time,
            file_extension
        ))
    };
    Ok(image_destination)
}

fn get_render_config() -> RenderConfig<'static> {
    RenderConfig::<'_> {
        unselected_checkbox: Styled::new("○"),
        selected_checkbox: Styled::new("●"),
        ..Default::default()
    }
}

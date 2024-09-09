# Createdat

Rename images and other files based on their creation date

## Installation

1. Install Rust and Cargo if you don't have them already. You can find installation instructions [here](https://www.rust-lang.org/tools/install).
2. Clone the repository using the following command:

```shell

git clone https://github.com/llithium/createdat.git

```

3. Navigate to the project directory:

```shell

cd createdat

```

4. Install with Cargo:

```shell

cargo install --path .

```

## Usage

```shell

createdat

```

Available options

```

Arguments:
  [NAME]  Optional prefix for renamed images

Options:
  -e, --extension        Choose which files to rename based on file extension
  -f, --front            Put date in front of filename
  -n, --no-name          Remove original filename
  -t, --twelve           Use 12-hour time format instead of 24-hour
  -d, --date             Date without time
      --format <Format>  Set custom date format to use ('%a %b %e %Y' = "Wed Jul 17 2024")
  -S, --source <Path>    Set the source folder for images
  -F, --folder <Path>    Set the target folder for renamed images (default: renamed)
  -s, --suffix           Put custom name after the date
  -p, --preview          Preview the name format of renamed files
  -a, --all              Rename all files, not just images
  -h, --help             Print help
  -V, --version          Print version


```

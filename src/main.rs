use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::ffi::OsString;
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use std::{env, io};

use chrono::{Datelike, NaiveDate};

/*
TODO: RSS
https://www.rssboard.org/rss-specification
https://www.tutorialspoint.com/rss/rss2.0-tag-syntax.htm
https://www.ietf.org/rfc/rfc4287.txt
*/

struct Argv {
    /* Folder to read content files from */
    input_content_path: PathBuf,
    /* Output will be stored in ./content/output_content_root_path if it's not None. */
    output_content_path: PathBuf,
}

fn argv() -> io::Result<Argv> {
    let argv: Vec<String> = env::args().collect();
    if argv.len() <= 1 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "specify content path in first argument.",
        ));
    }

    let input_content_path = PathBuf::from(&argv[1]);
    let output_content_root_path = PathBuf::from(&argv[2].trim_end_matches('/'));

    let argv = Argv {
        input_content_path,
        output_content_path: output_content_root_path,
    };

    let input_exists = argv.input_content_path.exists();
    let input_is_dir = argv.input_content_path.is_dir();

    if !input_exists {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "input content path (argv[0]) does not exist.",
        ));
    }
    if !input_is_dir {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "input content path is not a directory.",
        ));
    }

    Ok(argv)
}

fn content_files_dir_entries(content_path: &Path) -> io::Result<Vec<DirEntry>> {
    let content_files: Vec<Result<DirEntry, io::Error>> = fs::read_dir(content_path)?.collect();

    let errors: Vec<io::Error> = content_files
        .iter()
        .filter_map(|result| result.as_ref().err())
        .map(|err| io::Error::new(err.kind(), err.to_string()))
        .collect();

    if !errors.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{:#?}", errors),
        ));
    }

    let dir_entries: Vec<DirEntry> = content_files
        .into_iter()
        .filter_map(|result| result.ok())
        .collect();

    Ok(dir_entries)
}

fn content_file_pathbufs(input_content_path: PathBuf) -> io::Result<Vec<PathBuf>> {
    if !input_content_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "path to content does not exist.",
        ));
    }

    let content_file_dir_entries = content_files_dir_entries(&input_content_path)?;

    let content_file_pathbufs = content_file_dir_entries
        .into_iter()
        .filter(|dir_entry| !dir_entry.path().is_dir())
        .map(|dir_entry| dir_entry.path())
        .collect();

    Ok(content_file_pathbufs)
}

fn osstr_to_str_err() -> io::Error {
    io::Error::new(io::ErrorKind::Other, "error turning OsStr into str")
}

fn pathbuf_filename_get_err() -> io::Error {
    io::Error::new(io::ErrorKind::Other, "error getting PathBuf filename")
}

fn parse_content_meta_data_err(path: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        format!(
            "error parsing content file meta data for {}: splitn 2 '_' did not return len==2.",
            path
        ),
    )
}

fn content_metadata(path: &PathBuf) -> Result<(NaiveDate, String), Box<dyn Error>> {
    let path = match path.file_name() {
        Some(path) => match path.to_str() {
            Some(path) => path,
            None => return Err(Box::new(osstr_to_str_err())),
        },
        None => return Err(Box::new(pathbuf_filename_get_err())),
    };

    let split: Vec<&str> = path.splitn(2, '_').collect();
    if split.len() != 2 {
        return Err(Box::new(parse_content_meta_data_err(path)));
    }

    let date_str = split[0];
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let name = split[1];

    Ok((date, name.to_string()))
}

fn files_maps(
    content_file_paths: Vec<PathBuf>,
) -> Result<
    (
        BTreeMap<PathBuf, (NaiveDate, String)>,
        BTreeMap<i32, BTreeMap<u32, HashSet<u32>>>,
    ),
    Box<dyn Error>,
> {
    let mut content_files_meta_data: BTreeMap<PathBuf, (NaiveDate, String)> = BTreeMap::new();
    let mut content_file_system_structure: BTreeMap<i32, BTreeMap<u32, HashSet<u32>>> =
        BTreeMap::new();

    for path_to_content_file in content_file_paths {
        let (date, name) = content_metadata(&path_to_content_file)?;

        if !content_files_meta_data
            .insert(path_to_content_file, (date, name))
            .is_none()
        {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "unexpected duplicate content file.",
            )));
        }

        let _ = content_file_system_structure
            .entry(date.year())
            .or_insert(BTreeMap::new())
            .entry(date.month())
            .or_insert(HashSet::with_capacity(8))
            .insert(date.day());
    }

    Ok((content_files_meta_data, content_file_system_structure))
}

fn friendly_filename(name: &str) -> String {
    let mut result = Vec::with_capacity(name.len());
    for c in name.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if c == ' ' {
            result.push('_');
        }
    }
    result.iter().collect()
}

fn contents(title: String, content_file_path: &Path) -> io::Result<String> {
    let contents = fs::read_to_string(content_file_path)?;
    let contents = format!(
        ":base-path: ../../../..

include::{{base-path}}/head.adoc[]

== {}

{}",
        title, contents
    );

    Ok(contents)
}

fn construct_filesystem_time_structure(
    content_file_system_structure: BTreeMap<i32, BTreeMap<u32, HashSet<u32>>>,
    output_content_path: &PathBuf,
) -> io::Result<()> {
    for (year, months) in content_file_system_structure {
        for (month, days) in months {
            for day in days {
                let year_month_day = format!("{}/{:02}/{:02}", year, month, day);
                let path_to_content_dir =
                    format!("{}/{}", output_content_path.display(), year_month_day);
                fs::create_dir_all(path_to_content_dir)?;
            }
        }
    }
    Ok(())
}

fn construct_content_files(
    content_files_meta_data: BTreeMap<PathBuf, (NaiveDate, String)>,
    output_content_path: PathBuf,
) -> io::Result<()> {
    for (path, (date, name)) in content_files_meta_data {
        let (year, month, day) = (date.year_ce().1, date.month(), date.day());
        let year_month_day = format!("{}/{:02}/{:02}", year, month, day);
        let path_to_content_dir = format!("{}/{}", output_content_path.display(), year_month_day);
        let friendly_name = friendly_filename(&name);
        let content_file_output_path = format!("{}/{}.adoc", path_to_content_dir, friendly_name);
        let contents = contents(name, path.as_path())?;
        fs::write(content_file_output_path, contents)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let argv = argv()?;
    let content_file_paths = content_file_pathbufs(argv.input_content_path)?;
    let (content_files_meta_data, content_file_system_structure) = files_maps(content_file_paths)?;

    construct_filesystem_time_structure(content_file_system_structure, &argv.output_content_path)?;
    construct_content_files(content_files_meta_data, argv.output_content_path)?;

    Ok(())
}

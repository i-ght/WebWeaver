use std::collections::{BTreeMap, HashSet};
use std::error::Error;
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

fn argv() -> io::Result<Vec<String>> {
    let argv: Vec<String> = env::args().collect();
    if argv.len() <= 1 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "specify content path in first argument.",
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


fn content_file_pathbufs(argv: Vec<String>) -> io::Result<Vec<PathBuf>> {
    let content_path = &argv[1];
    let content_path = Path::new(content_path);
    if !content_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "path to content does not exist.",
        ));
    }

    let content_file_dir_entries = content_files_dir_entries(&content_path)?;

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

fn main() -> Result<(), Box<dyn Error>> {
    let argv = argv()?;
    let content_file_paths = content_file_pathbufs(argv)?;

    let mut content_files_meta_data: BTreeMap<PathBuf, (NaiveDate, String)> = BTreeMap::new();
    let mut content_file_system_structure: BTreeMap<i32, BTreeMap<u32, HashSet<u32>>> =
        BTreeMap::new();

    for path_to_content_file in content_file_paths {
        let (date, name) = content_metadata(&path_to_content_file)?;

        assert!(content_files_meta_data
            .insert(path_to_content_file, (date, name))
            .is_none());

        let _ = content_file_system_structure
            .entry(date.year())
            .or_insert(BTreeMap::new())
            .entry(date.month())
            .or_insert(HashSet::with_capacity(8))
            .insert(date.day());
    }

    /* for path_to_content_file in content_file_paths {
        let (date, name) = parse_content_metadata(&path_to_content_file)?;
        let file_contents = fs::read_to_string(path_to_content_file)?;

        let year_str = date.year().to_string();
        let year_dir = Path::new(&year_str);
        if !year_dir.exists() {
            fs::create_dir(year_dir)?;
        }


    } */
    dbg!(content_files_meta_data);

    for (year, months) in &content_file_system_structure {
        for (month, days) in months {
            for day in days {
                let year_month_day = format!("{}/{:02}/{:02}", year, month, day);
                dbg!(year_month_day);
            }
        }
    }

    Ok(())
}

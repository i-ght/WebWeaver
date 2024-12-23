use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs::{self, DirEntry};
use std::path::{Component, Path, PathBuf};
use std::{env, io};

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, Utc};
use rss::{Category, Channel, ChannelBuilder, Image, Item, ItemBuilder};

#[derive(Clone, Debug)]
struct ContentMetaUnit {
    date: NaiveDate,
    name: String,
    filesystem_friendly_name: String,
    file_ext: String,
    categories: Vec<String>,
    path: String,
}

struct ContentUnit {
    meta: ContentMetaUnit,
    contents: String,
}

struct Cfg {
    input_content_root_path: PathBuf,
    output_content_root_path: PathBuf,
    _author: Option<String>,
    category: String,
}

fn cfg() -> io::Result<Cfg> {
    let argv: Vec<String> = env::args().collect();
    if argv.len() <= 1 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "specify content path in first argument.",
        ));
    }

    let input_content_root_path = PathBuf::from(&argv[1]);

    let components: Vec<Component> = input_content_root_path.components().collect();

    let (output_content_root_path, author, category) =
        if let Some(content_index) = components.iter().position(|c| c.as_os_str() == ".content") {
            let after_content = &components[content_index + 1..];

            let author = after_content
                .get(0)
                .filter(|&c| c.as_os_str().to_str().expect("os str couldn't change to str.").starts_with('.'))
                .map(|c| {
                    c.as_os_str()
                        .to_string_lossy()
                        .into_owned()
                        .trim_start_matches('.')
                        .to_string()
                });

            let categories: Vec<String> = after_content
                .iter()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();

            let after_path = PathBuf::from_iter(after_content.iter().map(|c| c.as_os_str()));
            (after_path, author, categories.join("/"))
        } else {
            return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "error: did not find root content directory named .content which is a prerequisite.",
        ));
        };

    let cfg = Cfg {
        input_content_root_path,
        output_content_root_path,
        _author: author,
        category,
    };

    let input_exists = cfg.input_content_root_path.exists();

    if !input_exists {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "input content path (argv[0]) does not exist.",
        ));
    }

    let input_is_dir = cfg.input_content_root_path.is_dir();

    if !input_is_dir {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "input content path is not a directory.",
        ));
    }

    Ok(cfg)
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

fn content_file_pathbufs(input_content_path: &Path) -> io::Result<Vec<PathBuf>> {
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

fn content_file_metadata(
    path: &Path,
    content_output_root_path: &Path,
) -> Result<ContentMetaUnit, Box<dyn Error>> {
    let file_stem = match path.file_stem() {
        Some(path) => match path.to_str() {
            Some(path) => path,
            None => return Err(Box::new(osstr_to_str_err())),
        },
        None => return Err(Box::new(pathbuf_filename_get_err())),
    };

    let file_ext = match path.extension() {
        Some(file_ext) => match file_ext.to_str() {
            Some(file_ext) => file_ext,
            None => return Err(Box::new(osstr_to_str_err())),
        },
        None => return Err(Box::new(pathbuf_filename_get_err())),
    };

    let split: Vec<&str> = file_stem.splitn(2, '_').collect();
    if split.len() != 2 {
        return Err(Box::new(parse_content_meta_data_err(file_stem)));
    }

    let date_str = split[0];
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let name = split[1];

    let content_categories_path = content_output_root_path.to_string_lossy().into_owned();

    let categories: Vec<String> = content_categories_path
        .split('/')
        .map(String::from)
        .collect();

    let (year, month, day) = (date.year_ce().1, date.month(), date.day());
    let year_month_day = format!("{}/{:02}/{:02}", year, month, day);

    let categories_and_date_stamped_content_path =
        format!("{}/{}", content_categories_path, year_month_day);

    let filesystem_friendly_name = friendly_filename(name);

    let unit = ContentMetaUnit {
        date,
        name: name.to_string(),
        filesystem_friendly_name,
        file_ext: file_ext.to_string(),
        categories,
        path: categories_and_date_stamped_content_path,
    };

    Ok(unit)
}

fn files_map(
    content_file_paths: Vec<PathBuf>,
    content_output_root_path: &Path,
) -> Result<BTreeMap<PathBuf, ContentMetaUnit>, Box<dyn Error>> {
    let mut content_files_meta_data: BTreeMap<PathBuf, ContentMetaUnit> = BTreeMap::new();

    for path_to_content_file in content_file_paths {
        let meta = content_file_metadata(&path_to_content_file, content_output_root_path)?;

        if let None = content_files_meta_data 
            .insert(path_to_content_file, meta)
        {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "unexpected duplicate content file.",
            )));
        }
    }

    Ok(content_files_meta_data)
}

fn entries_map(
    content_files_meta_data: BTreeMap<PathBuf, ContentMetaUnit>,
) -> BTreeMap<u32, Vec<ContentMetaUnit>> {
    let mut map: BTreeMap<u32, Vec<ContentMetaUnit>> = BTreeMap::new();

    for (_, meta) in content_files_meta_data {
        let units = map
            .entry(meta.date.year_ce().1)
            .or_insert(Vec::with_capacity(8));
        units.push(meta);
    }

    for (_, units) in map.iter_mut() {
        units.sort_by(|a, b| b.date.cmp(&a.date));
    }

    map
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

fn content_unit_contents(title: &str, content_file_path: &Path) -> io::Result<String> {
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

fn title_case(input: &str) -> String {
    input
        .split_whitespace() // Split the string into words
        .map(|word| {
            let mut chars = word.chars(); // Get the characters of the word
            match chars.next() {
                // Take the first character
                Some(first) => first.to_uppercase().chain(chars).collect(), // Capitalize it and append the rest
                None => String::new(),                                      // Handle empty words
            }
        })
        .collect::<Vec<String>>() // Collect the capitalized words into a vector
        .join(" ") // Join them back into a single string with spaces
}

fn index_contents(
    category: &str,
    content_files_meta_data: BTreeMap<u32, Vec<ContentMetaUnit>>,
) -> String {
    let mut index = String::with_capacity(8192);

    let category = title_case(category);
    index.push_str(&format!("== \u{1F4D3} {} Index\n", category));
    index.push_str("\n");

    for (year, content_meta_units) in content_files_meta_data.iter().rev() {
        index.push_str(&format!("=== {}\n", year));
        index.push_str("\n");

        for unit in content_meta_units {
            index.push_str(&format!(
                "==== xref:{}/{}.{}[{}] — {}\n",
                unit.path,
                unit.filesystem_friendly_name,
                unit.file_ext,
                unit.name,
                unit.date.format("%B %d, %Y")
            ));
            index.push_str("\n");
        }
    }

    index
}

fn construct_content_filesystem(
    content_files_meta_data: &BTreeMap<PathBuf, ContentMetaUnit>,
) -> io::Result<Vec<ContentUnit>> {
    let mut content: Vec<ContentUnit> = Vec::with_capacity(content_files_meta_data.len());

    for (input_content_file_path, meta) in content_files_meta_data {
        let content_file_output_path = format!(
            "{}/{}.{}",
            meta.path, meta.filesystem_friendly_name, meta.file_ext
        );
        let contents = content_unit_contents(&meta.name, input_content_file_path)?;
        let dir = format!("content/{}", meta.path);
        let path = format!("content/{}", content_file_output_path);

        fs::create_dir_all(dir)?;
        fs::write(path, &contents)?;

        content.push(ContentUnit {
            meta: meta.clone(),
            contents,
        });
    }
    Ok(content)
}

fn rss_channel(
    link: &str,
    description: &str,
    title: &str,
    language: Option<String>,
    copyright: Option<String>,
    webmaster: Option<String>,
    categories: &[Category],
    image: Option<Image>,
    content: Vec<ContentUnit>,
) -> Channel {
    let now: DateTime<Utc> = Utc::now();
    let rfc_2822_date = now.to_rfc2822();

    let mut items: Vec<Item> = Vec::with_capacity(content.len());

    for unit in content {
        let (date, name, _categories, path, contents) = (
            unit.meta.date,
            unit.meta.name,
            unit.meta.categories,
            unit.meta.path,
            unit.contents,
        );

        let pub_date = date
            .and_time(NaiveTime::default())
            .and_local_timezone(Local)
            .unwrap();

        let item = ItemBuilder::default()
            .title(name.clone())
            /* .categories(categories) TODO: Each content item it's own category */
            .description(name)
            .content(contents)
            .pub_date(pub_date.to_rfc2822())
            .link(path) /* TODO: full URI */
            .build();

        items.push(item);
    }

    let channel = ChannelBuilder::default()
        .categories(categories)
        .description(description)
        .generator(Some(String::from("WebWeaver")))
        .items(items)
        .language(language)
        .last_build_date(rfc_2822_date)
        .copyright(copyright)
        .image(image)
        .link(link)
        .pub_date(now.to_rfc2822())
        .title(title)
        .webmaster(webmaster)
        .build();
    channel
}

fn _galginkomiker() {}

fn main() -> Result<(), Box<dyn Error>> {
    let cfg = cfg()?;
    let content_file_paths = content_file_pathbufs(&cfg.input_content_root_path)?;
    let content_files_meta_data: BTreeMap<PathBuf, ContentMetaUnit> =
        files_map(content_file_paths, &cfg.output_content_root_path)?;
    let content: Vec<ContentUnit> = construct_content_filesystem(&content_files_meta_data)?;

    let _rss_channel = rss_channel(
        "/",
        "galgenkomiker",
        "galkenkomiker",
        Some(String::from("en-us")),
        None,
        None,
        &vec![],
        None,
        content,
    );

    let entries = entries_map(content_files_meta_data);
    let index_contents = index_contents(&cfg.category, entries);

    println!("{}", index_contents);

    Ok(())
}

use crate::{errors::*, RenameMode::*, *};
use clap::{App, Arg};
use std::{env, fs, path::PathBuf};

fn app<'a, 'b>() -> App<'a, 'b> {
    App::new("Massive Renamer")
        .version("0.1.0")
        .author("Alsein <https://github.com/AlseinX>")
        .about("Massively rename the file names within contents.")
        .arg(
            Arg::with_name("silence")
                .help("Specify to mute the outputs.")
                .short("s")
                .long("silence"),
        )
        .arg(
            Arg::with_name("regex")
                .help("Specify to use regex matching instead of raw string matching.")
                .short("r")
                .long("regex"),
        )
        .arg(
            Arg::with_name("name only")
                .help("Specify to replace the name only.")
                .short("N")
                .long("name-only")
                .conflicts_with("content only"),
        )
        .arg(
            Arg::with_name("content only")
                .help("Specify to replace the content only.")
                .short("C")
                .long("content-only")
                .conflicts_with("name only"),
        )
        .arg(
            Arg::with_name("entry dir")
                .help("Specify a directory path instead of the current directory")
                .short("d")
                .long("dir")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("FIND")
                .help("The string for finding that defaults to string matching.")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("REPLACE")
                .help("The string that would be replaced on each matchings.")
                .required(true)
                .index(2),
        )
}

pub fn run() {
    let matches = app().get_matches();

    let entry_dir = fs::canonicalize(match matches.value_of("entry dir") {
        Some(value) => PathBuf::from(value),
        None => env::current_dir().expect_display(),
    })
    .expect_display();

    let find = matches.value_of("FIND").unwrap();
    let replace = matches.value_of("REPLACE").unwrap();
    let use_regex = matches.is_present("regex");
    let is_silence = matches.is_present("silence");

    let mode = if matches.is_present("name only") {
        NameOnly
    } else if matches.is_present("content only") {
        ContentOnly
    } else {
        Both
    };

    rename(entry_dir, find, replace, use_regex, mode, is_silence);
}

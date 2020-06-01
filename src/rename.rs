use crate::*;
use chardet::{charset2encoding, detect};
use encoding::{self, label::encoding_from_whatwg_label, DecoderTrap, EncoderTrap};
use fs::OpenOptions;
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use regex::*;
use std::sync::mpsc::{channel, Sender};
use std::{
    ffi::CStr,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::raw::c_char,
    path::{Path, PathBuf},
};

#[derive(Debug, Copy, Clone)]
pub enum RenameMode {
    Both = 0,
    NameOnly = 1,
    ContentOnly = 2,
}

impl RenameMode {
    pub fn is_replace_name(&self) -> bool {
        match self {
            RenameMode::ContentOnly => false,
            _ => true,
        }
    }

    pub fn is_replace_content(&self) -> bool {
        match self {
            RenameMode::NameOnly => false,
            _ => true,
        }
    }
}

#[derive(Debug, Clone)]
struct RenameWork<'a> {
    pub path: PathBuf,
    pub find: &'a str,
    pub replace: &'a str,
    pub use_regex: bool,
    pub mode: RenameMode,
    pub sender: Sender<RenameWork<'a>>,
    pub is_root: bool,
    pub silence: bool,
}

#[no_mangle]
pub extern "system" fn mrn(
    entry_dir: *const c_char,
    find: *const c_char,
    replace: *const c_char,
    use_regex: bool,
    replace_name: bool,
    replace_content: bool,
    silence: bool,
) {
    rename(
        PathBuf::from(to_str(entry_dir)),
        to_str(find),
        to_str(replace),
        use_regex,
        match (replace_name, replace_content) {
            (true, true) => RenameMode::Both,
            (true, false) => RenameMode::NameOnly,
            (false, true) => RenameMode::ContentOnly,
            _ => panic!("Must specify at least one of name and content to replace."),
        },
        silence,
    )
}

fn to_str<'a>(ptr: *const c_char) -> &'a str {
    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .expect("Invalid argument string.")
    }
}

pub fn rename(
    entry_dir: PathBuf,
    find: &str,
    replace: &str,
    use_regex: bool,
    mode: RenameMode,
    silence: bool,
) {
    let rx = {
        let (tx, rx) = channel();
        let work = RenameWork {
            path: entry_dir,
            find,
            replace,
            use_regex,
            mode,
            sender: tx.clone(),
            is_root: true,
            silence,
        };
        tx.send(work).expect_display();
        rx
    };
    rx.into_iter().par_bridge().for_each(handle_work)
}

struct CountingReplacer<'a> {
    value: &'a str,
    count: i32,
}

impl Replacer for &mut CountingReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures, dst: &mut String) {
        self.count += 1;
        self.value.replace_append(caps, dst)
    }
}

fn replace(
    source: impl AsRef<str>,
    find: &str,
    replace: &str,
    use_regex: bool,
) -> Result<(String, i32), String> {
    let source = source.as_ref();
    if use_regex {
        let reg = expect!(
            Regex::new(find),
            format!("\"{}\" is not a valid regular expression.", find)
        );
        let mut replacer = CountingReplacer {
            value: replace,
            count: 0,
        };
        let result = reg.replace_all(source, &mut replacer);
        Ok((result.to_string(), replacer.count))
    } else {
        let mut last_end = 0;
        let mut num = 0;
        let mut result = String::new();
        for (start, part) in source.match_indices(find) {
            result.push_str(unsafe { source.get_unchecked(last_end..start) });
            result.push_str(replace);
            last_end = start + part.len();
            num += 1;
        }
        result.push_str(unsafe { source.get_unchecked(last_end..source.len()) });
        Ok((result, num))
    }
}

fn handle_work<'a>(work: RenameWork<'a>) {
    let mut work = work;
    let mut output = format!("{}:\n", work.path.as_humanized_string());
    let mut modified = false;
    if !work.is_root && work.mode.is_replace_name() {
        if process_file_name(&mut work.path, work.find, work.replace, work.use_regex)
            .expect_display()
        {
            output.push_str(
                format!("   ·Renamed to \"{}\".\n", &work.path.as_humanized_string()).as_ref(),
            );
            modified = true;
        }
    }
    if fs::metadata(&work.path).expect_display().is_file() {
        if work.mode.is_replace_content() {
            match process_file_content(work.path.as_ref(), work.find, work.replace, work.use_regex)
            {
                Ok(num) if num > 0 => {
                    output.push_str(format!("   ·Replaced {} matches.\n", num).as_ref());
                    modified = true;
                }
                Err(err) => {
                    output.push_str(err.as_ref());
                    output.push_str("   ·Not a regular text file, skipped.\n");
                    modified = true;
                }
                _ => {}
            }
        }
    } else {
        for sub in fs::read_dir(&work.path).expect_display() {
            let sub = sub.expect_display();

            let new_work = RenameWork {
                path: sub.path(),
                is_root: false,
                ..work.clone()
            };

            work.sender.send(new_work).expect_display();
        }
    }
    if !work.silence && modified {
        print!("{}", output);
    }
}

fn process_file_name(
    path: &mut PathBuf,
    find: &str,
    repl: &str,
    use_regex: bool,
) -> Result<bool, String> {
    let (new_name, num) = expect!(replace(
        expect!(
            expect!(path.file_name(), None => format!("Failed to get a file name."))
                .convert_to_utf_8(),
            |err| format!("File \"{}\" has an invalid name.", err)
        ),
        find,
        repl,
        use_regex,
    ));
    if num > 0 {
        let to_path = path.parent().unwrap().join(new_name);
        expect!(
            fs::rename(&path, &to_path),
            format!(
                "Failed on renaming \"{}\" to \"{}\"",
                path.as_humanized_string(),
                to_path.as_humanized_string()
            )
        );
        *path = to_path;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn process_file_content(
    path: &Path,
    find: &str,
    repl: &str,
    use_regex: bool,
) -> Result<i32, String> {
    let mut file = expect!(
        OpenOptions::new().read(true).write(true).open(path),
        |err| format!(
            "Failed on openning \"{}\":\n{}",
            path.as_humanized_string(),
            err
        )
    );
    let mut buffer = Vec::new();
    expect!(file.read_to_end(&mut buffer), |err| format!(
        "Failed on reading \"{}\":\n{}",
        path.as_humanized_string(),
        err
    ));
    let (encode, _confidence, _language) = detect(&mut buffer);
    let coder = expect!(
        encoding_from_whatwg_label(charset2encoding(&encode)),
        None => format!(
            "Failed on detecting the encoding of \"{}\".",
            path.as_humanized_string()
        )
    );
    let content = expect!(coder.decode(&buffer, DecoderTrap::Ignore), |err| format!(
        "Failed on decoding \"{}\":\n{}",
        path.as_humanized_string(),
        err
    ));
    let (content, num) = expect!(replace(content, find, repl, use_regex));
    if num > 0 {
        buffer = expect!(
            coder.encode(content.as_ref(), EncoderTrap::Ignore),
            |err| format!(
                "Failed on re-encoding \"{}\":\n{}",
                path.as_humanized_string(),
                err
            )
        );
        expect!(
            file.set_len(0),
            format!("Failed on writing \"{}\".", path.as_humanized_string())
        );
        expect!(
            file.seek(SeekFrom::Start(0)),
            format!("Failed on writing \"{}\".", path.as_humanized_string())
        );
        expect!(
            file.write_all(&buffer),
            format!("Failed on writing \"{}\".", path.as_humanized_string())
        );
    }
    Ok(num)
}

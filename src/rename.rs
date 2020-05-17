use crate::*;
use chardet::{charset2encoding, detect};
use encoding::{self, label::encoding_from_whatwg_label, DecoderTrap, EncoderTrap};
use fs::OpenOptions;
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use std::sync::mpsc::{channel, Sender};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
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
pub extern "stdcall" fn rename(
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

fn replace(source: impl AsRef<str>, find: &str, replace: &str, _use_regex: bool) -> (String, i32) {
    let source = source.as_ref();
    let mut result = String::new();
    let mut last_end = 0;
    let mut num = 0;
    for (start, part) in source.match_indices(find) {
        result.push_str(unsafe { source.get_unchecked(last_end..start) });
        result.push_str(replace);
        last_end = start + part.len();
        num += 1;
    }
    result.push_str(unsafe { source.get_unchecked(last_end..source.len()) });
    (result, num)
}

fn handle_work<'a>(work: RenameWork<'a>) {
    let mut work = work;
    let mut output = format!("{}:\n", work.path.as_humanized_string());
    let mut modified = false;
    if !work.is_root && work.mode.is_replace_name() {
        let (new_name, num) = replace(
            work.path
                .file_name()
                .expect_display()
                .convert_to_utf_8()
                .expect_display(),
            work.find,
            work.replace,
            work.use_regex,
        );
        if num > 0 {
            let to_path = work.path.parent().unwrap().join(new_name);
            fs::rename(&work.path, &to_path).expect(
                format!(
                    "Failed on renaming \"{}\" to \"{}\".",
                    &work.path.as_humanized_string(),
                    &to_path.as_humanized_string()
                )
                .as_ref(),
            );
            work.path = to_path;
            output.push_str(
                format!("   ·Renamed to \"{}\".\n", &work.path.as_humanized_string()).as_ref(),
            );
            modified = true;
        }
    }
    if fs::metadata(&work.path).expect_display().is_file() {
        if work.mode.is_replace_content() {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&work.path)
                .expect_display();
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).expect_display();
            let (encode, _confidence, _language) = detect(&mut buffer);
            let coder = encoding_from_whatwg_label(charset2encoding(&encode)).expect_display();
            let content = coder.decode(&buffer, DecoderTrap::Ignore).expect_display();
            let (content, num) = replace(content, work.find, work.replace, work.use_regex);
            if num > 0 {
                buffer = coder
                    .encode(content.as_ref(), EncoderTrap::Ignore)
                    .expect_fn(|err| err);
                file.set_len(0).expect_display();
                file.seek(SeekFrom::Start(0)).expect_display();
                file.write_all(&buffer).expect_display();
                output.push_str(format!("   ·Replaced {} matches.\n", num).as_ref());
                modified = true;
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

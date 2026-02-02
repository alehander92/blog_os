use crate::vga_buffer::WRITER;
use crate::{print, println};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::{
    stream::{Stream, StreamExt},
    task::AtomicWaker,
};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler
///
/// Must not block or allocate.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        // fast path
        if let Ok(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tag(String);

#[derive(Debug, Clone)]
pub enum FileKind {
    Text,
    Log,
}

#[derive(Debug, Clone)]
pub struct File {
    pub name: String,
    pub tags: Vec<Tag>,
    pub kind: FileKind,
    pub raw: String,
    pub size: usize,
}

impl File {
    pub fn new(filename: &str, raw: &str, tags: &[Tag]) -> File {
        let (name, kind) = File::parse_filename(filename);
        File {
            name,
            kind,
            tags: tags.to_vec(),
            raw: raw.to_string(),
            size: raw.len(),
        }
    }

    pub fn parse_filename(filename: &str) -> (String, FileKind) {
        let tokens: Vec<&str> = filename.split(".").collect();
        if tokens.len() != 2 {
            (filename.to_string(), FileKind::Text)
        } else {
            let name = tokens[0].to_string();
            let raw_kind = tokens[1];
            let kind = match raw_kind {
                "txt" | "text" => FileKind::Text,
                "log" => FileKind::Log,
                _ => FileKind::Text,
            };
            (name, kind)
        }
    }
}

#[derive(Debug, Clone)]
pub struct OsDb {
    pub tags: Vec<Tag>,
    pub current_tags: Vec<Tag>,
    pub files: BTreeMap<String, File>,
}

#[derive(Debug, Clone)]
pub enum TaskError {
    DbError(String),
    Other(String),
}

impl OsDb {
    pub fn new() -> OsDb {
        let first = Tag("first".to_string());
        OsDb {
            tags: vec![first.clone()],
            current_tags: vec![first],
            files: BTreeMap::new(),
        }
    }

    pub fn write_file(
        &mut self,
        filename: &str,
        content: &str,
        tags: &[Tag],
    ) -> Result<(), TaskError> {
        if !self.files.contains_key(filename) {
            self.files
                .insert(filename.to_string(), File::new(filename, content, tags));
            Ok(())
        } else {
            Err(TaskError::DbError("file already exists".to_string()))
        }
    }

    pub fn files_for_tags(&self, tags: &[Tag]) -> Vec<File> {
        // println!("files all: {:?} tags: {:?}", self.files, tags);
        self.files
            .iter()
            .filter(|(_, file)| file.tags.iter().any(|t| tags.contains(t)))
            .map(|(_, file)| file.clone())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Shell {
    pub db: OsDb,
}

#[derive(Debug, Clone)]
pub enum ShellResult {
    List(Vec<ShellResult>),
    Raw(String),
    File(File),
    Error(String),
}

impl Shell {
    pub fn new() -> Shell {
        Shell { db: OsDb::new() }
    }

    pub fn command_current_tags(&mut self) -> ShellResult {
        let results: Vec<ShellResult> = self
            .db
            .current_tags
            .iter()
            .map(|t| ShellResult::Raw(format!("@{}", t.0)))
            .collect();
        ShellResult::List(results)
    }

    pub fn command_write(&mut self, filename: &str, content: &str) -> ShellResult {
        let current_tags = self.db.current_tags.clone();
        match self.db.write_file(filename, content, &current_tags) {
            Ok(_) => ShellResult::Raw("ok: file written".to_string()),
            Err(e) => ShellResult::Error(format!("file write error: {e:?}")),
        }
    }

    pub fn command_list_files(&mut self, tags: &[Tag]) -> ShellResult {
        let files = self.db.files_for_tags(tags);
        ShellResult::List(
            files
                .iter()
                .map(|file| ShellResult::File(file.clone()))
                .collect(),
        )
    }

    pub async fn run(&mut self, line: &str) -> ShellResult {
        let tokens: Vec<&str> = line.split(" ").collect();
        if tokens.len() == 0 {
            return ShellResult::Error("no command".to_string());
        }
        let command = tokens[0];
        match command {
            "pwd" => self.command_current_tags(),
            "write" => {
                if tokens.len() < 3 {
                    return ShellResult::Error(
                        "expected at least two arguments for `write`".to_string(),
                    );
                }
                self.command_write(tokens[1], tokens[2])
            }
            "ls" => {
                let tags: Vec<Tag> = if tokens.len() == 1 {
                    self.db.current_tags.clone()
                } else {
                    tokens[1..].iter().map(|raw| Tag(raw.to_string())).collect()
                };
                self.command_list_files(&tags)
            }
            _ => ShellResult::Error("not recognized".to_string()),
        }
    }

    pub fn render(&self, res: &ShellResult, inline: bool) -> String {
        match res {
            ShellResult::List(results) => {
                let mut text = "".to_string();
                if !inline {
                    text.push('\n');
                }
                for (i, res) in results.iter().enumerate() {
                    let sub_inline = true;
                    if inline {
                        text.push_str(&format!("{}", self.render(res, sub_inline)));
                        if i < results.len() - 1 {
                            text.push_str(", ");
                        }
                    } else {
                        text.push_str(&format!("-------\n{}\n", self.render(res, sub_inline)));
                    }
                }
                text
            }
            ShellResult::Raw(text) => text.to_string(),
            ShellResult::File(file) => {
                format!("| {} | {:?} | {} |", file.name, file.kind, file.size)
            }
            ShellResult::Error(e) => {
                format!("error: {e}")
            }
        }
    }
}

pub async fn shell() {
    let mut shell = Shell::new();
    let mut scancodes = ScancodeStream::new();
    println!("os> welcome");
    loop {
        print!("user> ");
        let line = read_line(&mut scancodes).await;
        let res = shell.run(&line).await;
        let inline = false;
        println!("os> {}", shell.render(&res, inline));
    }
}

const BACKSPACE_ASCII_CODE: u8 = 8;

pub async fn read_line(scancodes: &mut ScancodeStream) -> String {
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);
    let mut result = String::from("");
    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        if character == '\n' {
                            print!("{}", character);
                            break;
                        } else if character as u8 == BACKSPACE_ASCII_CODE {
                            let mut writer = WRITER.lock();
                            writer.clear_last_n_characters(1);
                            result.pop();
                        } else {
                            // print!("{}", character as u8);
                            print!("{}", character);
                            result.push(character);
                        }
                    }
                    DecodedKey::RawKey(_key) => {
                        print!("[RAW {:?}]", key)
                    }
                }
            }
        }
    }
    return result;
}

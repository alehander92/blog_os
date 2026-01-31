use crate::vga_buffer::WRITER;
use crate::{print, println};
use alloc::string::{String, ToString};
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

#[derive(Debug, Clone)]
pub struct Shell {}

#[derive(Debug, Clone)]
pub enum ShellResult {
    Raw(String),
    Error(String),
}

impl Shell {
    pub async fn run(&mut self, line: &str) -> ShellResult {
        if line == "pwd" {
            ShellResult::Raw("/".to_string())
        } else {
            ShellResult::Error("not recognized".to_string())
        }
    }
}
pub async fn shell() {
    let mut shell = Shell {};
    let mut scancodes = ScancodeStream::new();
    println!("os> welcome");
    loop {
        print!("user> ");
        let line = read_line(&mut scancodes).await;
        let res = shell.run(&line).await;
        println!("os> {res:?}");
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

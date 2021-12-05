use std::time::Instant;
use std::cell::RefCell;
use pancurses::noecho;
use std::thread::JoinHandle;
use pancurses::Window;
use std::time::Duration;
use pancurses::{endwin, initscr, Input};
use crossbeam::channel::{unbounded as unbound_channel, Receiver};
pub struct Progress {
    title: String,
    progress: f64,
    y: i32
}

impl Progress {
    pub fn new<T: AsRef<str>>(title: T, y: i32) -> Self{
        Progress{title: title.as_ref().to_owned(), progress: 0.0, y}
    }

    pub fn progress(&mut self, amount: f64) -> Message {
        // println!("{}: {} += {}", self.title, self.progress, amount);
        self.progress += amount;

        if self.progress >= 100.0 {
            Message::Finished{title: self.title.clone(), y: self.y}
        } else {
            Message::Progress{title: self.title.clone(), progress: self.progress, y: self.y}
        }
    }
}

pub trait WindowItem {
    fn bg_y(&self) -> i32;
    fn len(&self) -> usize;
    fn poll(&mut self, win: &Window) -> bool;
    fn is_done(&self) -> bool;
}

struct ProgressBar {
    items: Vec<String>,
    handles: Option<Vec<JoinHandle<()>>>,
    rx: Option<Receiver<Message>>,
    start: i32,
    width: usize,
    c: usize
}

impl ProgressBar {
    pub fn new(items: Vec<String>, start: i32, width: usize) -> Self {
        Self{items, handles: None, start, rx: None, c: 0, width}
    }

    pub fn start(& mut self){
        let (tx, rx) = unbound_channel();
        let handles = self.items.iter().cloned().enumerate().map(|(i, s)| {
            let t = tx.clone();
            let ys = self.start;
            std::thread::spawn(move || {
                let tx = t;
                let mut progressbar = Progress::new(s, ys + i as i32);
                let mut flag = false;
                loop {
                    let msg = progressbar.progress(rand::random::<f64>() * 4.0);
                    match &msg {
                        Message::Finished{title: _, y: _} => flag = true,
                        _ => ()
                    }
                    tx.send(msg).unwrap();
                    if flag {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(200))
                }
            })
        }).collect::<Vec<JoinHandle<()>>>();
        self.handles = Some(handles);
        self.rx = Some(rx);
    }

    fn join(&mut self) {
        if let Some(handles) = self.handles.take() {
            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    fn draw(&self, win: &Window, title: String, progress: f64, y: i32) {
        win.mv(y as i32, win.get_beg_x());
        let f = 100.0 / self.width as f64;
        let s = if progress == 100.0 {
            "=".repeat(self.width)
        } else {
            let x = (progress/f).floor() as usize;
            ["=".repeat(x), ">".to_owned(), " ".repeat(self.width - 1 - x)].join("")
        };
        win.printw(format!("{}: [{}] {:.2} %%",title, s, progress ));
        win.refresh();
    }
}

impl WindowItem for ProgressBar {
    fn bg_y(&self) -> i32 {
        self.start
    }

    fn len(&self) -> usize {
        self.items.len()
    }
    fn poll(&mut self, win: &Window) -> bool {
        if self.is_done() {return true;}
        if let Ok(msg) = self.rx.as_ref().unwrap().try_recv() {
            match msg {
                Message::Finished{title, y} => {
                    self.c += 1;
                    self.draw(&win, title, 100.0, y)
                },
                Message::Progress{title, progress, y} => {
                    self.draw(&win, title, progress, y);
                }
            }
        }
        if self.is_done() {
            self.join();
        }
        self.is_done()
    }

    fn is_done(&self) -> bool {
        self.c >= self.len()
    }
}
pub enum Message {
    Progress{title: String, progress: f64, y: i32},
    Finished{title: String, y: i32}
}

struct ScrollingMsg {
    msg: String,
    width: usize,
    y: i32,
    x: usize,
    last: Instant,
    speed: usize
}
impl ScrollingMsg {
    pub fn new(msg: String, width: usize, y: i32, speed: usize) -> Self {
        Self{msg, width, y, speed, last: Instant::now(),x: 0}
    }
}

impl WindowItem for ScrollingMsg {

    fn bg_y(&self) -> i32 {self.y}
    fn len(&self) -> usize { 1 }
    fn poll(&mut self, win: &pancurses::Window) -> bool {
        let dx = ((Instant::now() - self.last).as_secs_f32() * self.speed as f32).floor() as usize;
        if dx != 0 {
            self.last = Instant::now();
        }
        self.x = (self.x + dx) % self.width;
        win.mv(self.bg_y(), win.get_beg_x());
        if self.x + self.msg.len() > self.width {
            let s = self.width- self.x ;
            win.printw(format!("[{}{}{}]", &self.msg[s..], " ".repeat(self.width - self.msg.len()), &self.msg[..s]));
        } else {
            win.printw(format!("[{}{}{}]", " ".repeat(self.x), self.msg, " ".repeat(self.width - self.msg.len() - self.x)));
        }
        false
    }
    fn is_done(&self) -> bool { false }
}
fn main() {
    let files = vec!("File1.pdf", "File2.bmp", "File3.xml", "File4.mov", "File5.db");
    let window = initscr();
    window.nodelay(true);
    window.keypad(true);
    noecho();
    window.printw("Downloading Files (not Actually)\n");
    let mut ys = window.get_cur_y();

    let mut progressbar = ProgressBar::new(files.into_iter().map(|s| s.to_owned()).collect(), ys, 50);
    progressbar.start();

    ys += progressbar.len() as i32;

    let mut items: Vec<Box<RefCell<dyn WindowItem>>> = vec!(Box::new(RefCell::new(progressbar)));
    items.push(Box::new(RefCell::new(ScrollingMsg::new("Hello there!".to_owned(), 36, ys, 4))));

    ys += 1;
    progressbar = ProgressBar::new(vec!("Folder.xfp", "My Photos", "Friends Season 1").into_iter().map(|s| s.to_owned()).collect(), ys, 40);
    progressbar.start();
    ys += progressbar.len() as i32;
    items.push(Box::new(RefCell::new(progressbar)));

    items.push(Box::new(RefCell::new(ScrollingMsg::new("YAAAAAAAAAAAAS!".to_owned(), 36, ys, 4))));
    loop {
        match window.getch() {
            Some(Input::KeyDC) => {
                break;
            },
            _ => (),
        }
        for item in &items {
            item.borrow_mut().poll(&window);
        }
    }
    endwin();
}

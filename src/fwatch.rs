use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

use inotify::{
    EventMask,
    Inotify,
    WatchDescriptor,
    WatchMask,
    Event,
};

use regex::Regex;

use std::ffi::{OsStr, OsString};

use std::collections::HashMap;

use std::path::{PathBuf};

use std::process::{Child, Command};
use std::{thread};

use std::sync::mpsc::{
    channel,
    Sender,
};

type WatchMap = HashMap<WatchDescriptor, PathBuf>;
type NextCommand = PathBuf;


/// FWatch runtime info
pub struct Runtime {
    map: WatchMap,
    inotify: Inotify,
    template: Vec<String>,
    extension: Option<String>,
    regex: Option<Regex>,
}

impl Runtime {
    /// Setup the runtime.
    pub fn new(
        template: Vec<String>,
        extension: Option<String>,
        regex: Option<Regex>,
    ) -> Result<Runtime, String> {
        let (inotify, map) = watch_directories()?;
        Ok(Runtime {
            inotify,
            map,
            template: template,
            extension,
            regex,
        })
    }

    fn get_path(&self, wd: &WatchDescriptor, n: &OsStr) -> Result<PathBuf, String> {
        match self.map.get(&wd) {
            None => Err("Could not find WD!".to_string()),
            Some(p) => Ok(p.join(n)),
        }
    }

    fn filter_event(&self, event: &Event<&OsStr>) -> Option<PathBuf> {
        if event.mask.contains(EventMask::CLOSE_WRITE) {
            return None;
        }

        if let Some(p) = event.name {
            if let Ok(real_file) = self.get_path(&event.wd, &p) {
                if let Some(ext_matcher) = &self.extension {
                    if let Some(actual_ext) = real_file.extension() {
                        if *ext_matcher == actual_ext.to_string_lossy() {
                            return Some(real_file.to_path_buf());
                        }
                    }
                };

                if let Some(regex_matcher) = &self.regex {
                    if regex_matcher.is_match(&real_file.to_string_lossy()) {
                        return Some(real_file.to_path_buf());
                    }
                }
            }
        }
        None
    }


    pub fn run(&mut self) -> Result<(), String> {
        let mut buffer = [0u8; 4096];
        let monitor = monitor_child(&self.template);
        loop {
            let events = self
                .inotify
                .read_events_blocking(&mut buffer)
                .expect("Failed to read inotify events");
            /*
             * TODO: watch new directories...
               if event.mask.contains(EventMask::CREATE) {
               if event.mask.contains(EventMask::ISDIR) {
           */
            for e in events.filter_map(|e| self.filter_event(&e)) {
                monitor.send(e.to_path_buf())
                    .or_else(|e| Err(format!("Couldn't send message to monitor thread: {}", e)))?;
            }
        }
    }
}

fn monitor_child(template: &Vec<String>) -> Sender<NextCommand> {
    let template = template.iter().map(|e| e.clone()).collect::<Vec<String>>();
    let (tx, rx) = channel::<PathBuf>();
    thread::spawn(move || {
        let mut running: Option<Child> = None;
        loop {
            match rx.recv() {
                Ok(next) => {
                    if let Some(mut c) = running {
                        match c.try_wait() {
                            Ok(None) => {
                                match c.kill() { _ => {} };
                                match c.wait() { _ => {} };
                            }
                            _ => {
                            }
                        }
                    }

                    let command = template.get(0).unwrap();
                    let args: Vec<OsString> = template.iter()
                        .skip(1)
                        .map(|e| OsString::from(e.replace("{}", &next.to_string_lossy())))
                        .collect();

                    running = Some(
                        Command::new(&command).args(&args).spawn().unwrap()
                    );
                }
                _ => { }
            }
        }
    });

    tx
}

fn watch_directories() -> Result<(Inotify, WatchMap), String> {
    let mask=  WatchMask::CLOSE_WRITE | WatchMask::MOVE;
    let mut inotify = Inotify::init()
        .or_else(|e| Err(format!("Could not startup inotify: {}", e.to_string())))?;

    let mut wd_map = HashMap::new();

    let mut override_builder = OverrideBuilder::new("./");
    override_builder
        .add("!.git")
        .expect("Couldn't parse ignore file");

    let mut builder = WalkBuilder::new("./");
    builder
        .hidden(false)
        .overrides(override_builder.build().expect("Couldn't build overrides"));

    for entry in builder.build() {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_dir() {
                    let wd = inotify.add_watch(path, mask).or_else(|e| {
                        Err(format!(
                            "Failed to add watch to {:?}: {}",
                            path,
                            e.to_string()
                        ))
                    })?;
                    wd_map.insert(wd, path.to_owned());
                }
            }
            Err(err) => println!("E: {}", err),
        }
    }

    Ok((inotify, wd_map))
}

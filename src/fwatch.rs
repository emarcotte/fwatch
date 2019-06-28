use bytes::BytesMut;
use bytes::buf::BufMut;
use futures::Stream;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use inotify::{Event, Inotify, WatchDescriptor, WatchMask};
use crate::pager::{ Pager, };
use regex::Regex;
use shared_child::SharedChild;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

type WatchMap = HashMap<WatchDescriptor, PathBuf>;

/// FWatch runtime info
pub struct Runtime {
    dirs: Vec<String>,
    extension: Option<String>,
    map: WatchMap,
    pager: Option<Pager>,
    regex: Option<Regex>,
    template: Vec<String>,
}

impl Runtime {
    /// Setup the runtime.
    pub fn new(template: Vec<String>, extension: Option<String>, regex: Option<Regex>, dirs: Vec<String>, pager: bool) -> Runtime {
        Runtime {
            map: WatchMap::new(),
            template,
            extension,
            regex,
            dirs,
            pager: match pager {
                true  => Some(Pager::new()),
                false => None,
            },
        }
    }

    /// Find the path for a `WatchDescriptor`.
    fn get_path(&self, wd: &WatchDescriptor, n: &OsStr) -> Result<PathBuf, String> {
        match self.map.get(&wd) {
            Some(p) => Ok(p.join(n)),
            None => Err("Could not find WD!".to_string()),
        }
    }

    /// Prune down to the events that are something we should invoke the command
    fn is_executable_event(&self, event: &Event<OsString>) -> Option<PathBuf> {
        if let Some(p) = &event.name {
            if let Ok(real_file) = self.get_path(&event.wd, &p) {
                if let Some(ext_matcher) = &self.extension {
                    if let Some(actual_ext) = real_file.extension() {
                        if *ext_matcher == actual_ext.to_string_lossy() {
                            return Some(real_file.to_path_buf());
                        }
                    }
                }

                if let Some(regex_matcher) = &self.regex {
                    if regex_matcher.is_match(&real_file.to_string_lossy()) {
                        return Some(real_file.to_path_buf());
                    }
                }

                if self.regex.is_none() && self.extension.is_none() {
                    return Some(real_file.to_path_buf());
                }
            }
        }
        None
    }

    /// Kick off the event loop.
    pub fn run(mut self) -> Result<(), String> {
        // TODO: Remove all expect/unwrap calls.
        /* TODO: watch new directories...
         if event.mask.contains(EventMask::CREATE) {
         if event.mask.contains(EventMask::ISDIR) {
        */

        let mut inotify = Inotify::init().expect("Couldn't start up inotify");

        // TODO: http://man7.org/linux/man-pages/man7/inotify.7.html
        // sizeof(struct inotify_event) + NAME_MAX + 1
        let mut buf = BytesMut::with_capacity(4096);
        buf.put_slice(&[0u8; 4096]);

        // TODO: Move out of here.
        for dir in self.dirs.clone() {
            self.watch_directories(&mut inotify, &dir)
                .expect("Error watching directories");
        }

        let stream = inotify.event_stream(buf);

        let mut running: Option<Arc<SharedChild>> = None;
        for event in stream.wait() {
            let event = event.unwrap();
            if let Some(path) = self.is_executable_event(&event) {

                if let Some(pager) = &mut self.pager {
                    pager.stop();
                }

                let child = self.start(&path)?;
                let started = Arc::new(child);

                let wait_clone = started.clone();
                std::thread::spawn(move || {
                    wait_clone.wait().unwrap();
                });

                if let Some(running) = running {
                    running.kill().unwrap();
                }
                running = Some(started);
            }
        }

        Ok(())
    }

    /// Add the given path to the runtime.
    fn watch_directories(&mut self, inotify: &mut Inotify, path: &str) -> Result<(), String> {
        let mask = WatchMask::CLOSE_WRITE | WatchMask::MOVE;

        let overrides = OverrideBuilder::new(path)
            .add("!.git")
            .map_err(|e| format!("Error building overrides: {}", e.to_string()))?
            .build()
            .expect("Couldn't parse ignore file");

        let mut builder = WalkBuilder::new("./");
        builder.hidden(false).overrides(overrides);

        for entry in builder.build() {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_dir() {
                        // TODO: Prevent adding multiple watches to the same directory.
                        let wd = inotify.add_watch(path, mask).or_else(|e| {
                            Err(format!(
                                "Failed to add watch to {:?}: {}",
                                path,
                                e.to_string()
                            ))
                        })?;
                        self.map.insert(wd, path.to_owned());
                    }
                }
                Err(err) => {
                    println!("Warning, couldn't walk directory: {}", err);
                }
            }
        }

        Ok(())
    }

    /// Given a file name, build and start a child process.
    fn start(&mut self, next: &PathBuf) -> Result<SharedChild, String> {
        let command = &self.template[0];
        let args: Vec<OsString> = self
            .template
            .iter()
            .skip(1)
            .map(|e| OsString::from(e.replace("{}", &next.to_string_lossy())))
            .collect();

        let mut c = Command::new(&command);
        c.args(&args);

        if let Some(pager) = &mut self.pager {
            let stream = pager.start()?;
            c.stdout(stream.try_clone().unwrap());
            c.stderr(stream);
        }

        SharedChild::spawn(&mut c)
            .map_err(|e| format!("Spawn error: {}", e.to_string()))
    }
}

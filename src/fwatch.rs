use bytes::BytesMut;
use bytes::buf::BufMut;

use futures::prelude::*;
//use futures::stream::Stream;

use futures::{
    select,
    stream::{
        StreamExt,
    },
};

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use inotify::{Event, EventMask, EventStream, Inotify, WatchDescriptor, WatchMask};
use os_pipe::{PipeWriter, pipe};
use regex::Regex;
use shared_child::SharedChild;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::path::{ Path, PathBuf, };
use std::process::Command;
use std::io::{ BufRead, BufReader, };
use std::sync::Arc;
use super::pager::{ Pager };
use futures::channel::oneshot::{channel, Receiver};

type WatchMap = HashMap<WatchDescriptor, PathBuf>;

/// FWatch runtime info
pub struct Runtime {
    extension: Option<String>,
    inotify: Inotify,
    map: WatchMap,
    pager: Option<Arc<Pager>>,
    regex: Option<Regex>,
    running: Option<Arc<SharedChild>>,
    command: OsString,
    args: Vec<String>,
}

impl Runtime {
    /// Setup the runtime.
    pub fn new(template: Vec<String>) -> Result<Runtime, String> {
        if template.is_empty() {
            return Err("Empty template string!".to_string());
        }

        Ok(Runtime {
            extension: None,
            inotify: Inotify::init()
                .map_err(|e| format!("Error starting up inotify: {}", e))?,
            map: WatchMap::new(),
            pager: None,
            regex: None,
            running: None,
            command: OsString::from(&template[0]),
            args: template.into_iter().skip(1).collect(),
        })
    }

    pub fn use_pager(&mut self, should_page: bool) -> Result<&mut Runtime, Box<dyn Error>> {
        self.pager = match should_page {
            true  => Some(Arc::new(Pager::new()?)),
            false => None,
        };
        Ok(self)
    }

    pub fn set_extension(&mut self, ext: String) -> &mut Runtime {
        self.extension = Some(ext);
        self
    }

    pub fn set_regex(&mut self, regex: Regex) -> &mut Runtime {
        self.regex = Some(regex);
        self
    }

    /// Find the path for a `WatchDescriptor`.
    fn get_path(&self, wd: &WatchDescriptor, n: &OsStr) -> Option<PathBuf> {
        self.map.get(&wd)
            .map(|p| p.join(n))
    }

    /// Get the path for an event if one exists.
    fn get_event_path(&self, event: &Event<OsString>) -> Option<PathBuf> {
        match &event.name {
            Some(p) => self.get_path(&event.wd, &p),
            _ => None,
        }
    }

    /// Prune down to the events that are something we should invoke the command
    fn is_executable_event(&self, event: &Event<OsString>) -> Option<PathBuf> {
        if event.mask.contains(EventMask::ISDIR) || !event.mask.contains(EventMask::CLOSE_WRITE) {
            return None;
        }

        if let Some(real_file) = self.get_event_path(&event) {
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
        None
    }

    fn is_watchable_dir(&self, event: &Event<OsString>) -> Option<PathBuf> {
        if event.mask.contains(EventMask::ISDIR)
            && (event.mask.contains(EventMask::CREATE)
                || event.mask.contains(EventMask::MOVED_TO)) {
            return self.get_event_path(&event);
        }
        None
    }

    // TODO: http://man7.org/linux/man-pages/man7/inotify.7.html
    // sizeof(struct inotify_event) + NAME_MAX + 1
    fn make_buffer(&self,) -> BytesMut {
        let mut buf = BytesMut::with_capacity(4096);
        buf.put_slice(&[0u8; 4096]);
        buf
    }

    /// Kick off the event loop.
    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        // TODO: Remove all expect/unwrap calls.
        let fs_stream = self.get_stream()?;

        if let Some((pager, pager_stream)) = self.start_pager() {
            let (reader, writer) = pipe()?;
            std::thread::spawn(move || {
                let mut buffer = BufReader::new(reader);
                loop {
                    let mut buf = String::new();
                    match buffer.read_line(&mut buf) {
                        Ok(0)  => { pager.add("ZzzEOF"); break; },
                        Ok(_)  => { pager.add(&buf); },
                        Err(e) => { pager.add(&format!("Error: {}", e)); break; },
                    };
                };
            });

            let mut fused_fs_stream = fs_stream.fuse();
            let mut fused_recv = pager_stream.fuse();

            loop {
                select!(
                    fs_event = fused_fs_stream.next() => {
                        match fs_event {
                            Some(Ok(fs_event)) => {
                                let process_output_writer = writer.try_clone()?;
                                self.process_event(&fs_event, Some(process_output_writer));
                            },
                            Some(Err(e)) => {
                                eprintln!("Something crashed in fs stream: {}", e);
                                break;
                            },
                            None => unreachable!(),
                        };
                    },
                    _ = fused_recv => break,
                    complete => break,
                    default  => unreachable!(),
                );
            }
        }
        else {
            unreachable!();
            /*
            for event in fs_stream.wait() {
                self.process_event(&event.unwrap(), None);
            }
            */
        }

        Ok(())
    }

    fn start_pager(&self) -> Option<(Arc<Pager>, Receiver<()>)> {
        if let Some(running_pager) = &self.pager {
            let (tx, rx) = channel();
            let exit_monitor = running_pager.clone();
            std::thread::spawn(move || {
                exit_monitor.run();
                tokio::spawn(async move {
                    if let Err(e) = tx.send(()) {
                        eprintln!("Error sending process completion: {:?}", e);
                    }
                });
            });
            Some((running_pager.clone(), rx))
        }
        else {
            None
        }
    }

    fn get_stream(&mut self) -> std::io::Result<EventStream<BytesMut>> {
        self.inotify.event_stream(self.make_buffer())
    }

    // TODO: Replace output with customized pagers.
    fn process_event(&mut self, event: &Event<OsString>, output: Option<PipeWriter>) {
        if let Some(path) = self.is_watchable_dir(&event) {
            match self.watch_directories(&path) {
                Err(e) => println!("Warning, could not watch {}: {}", path.to_string_lossy(), e),
                _ => (),
            }
        }
        if let Some(path) = self.is_executable_event(&event) {
            if let Some(pager) = &self.pager {
                pager.reset();
            }

            match self.start(&path, output) {
                Err(e)    => println!("Error starting command: {}", e),
                Ok(child) => {
                    if let Some(running) = &self.running {
                        running.kill().unwrap();
                    }
                    self.running = Some(child);
                }
            }
        }
    }

    /// Add the given path to the runtime.
    pub fn watch_directories(&mut self, path: &dyn AsRef<Path>) -> Result<(), String> {
        let mask = WatchMask::CLOSE_WRITE | WatchMask::MOVE | WatchMask::CREATE;

        let overrides = OverrideBuilder::new(path)
            .add("!.git")
            .map_err(|e| format!("Error building overrides: {}", e.to_string()))?
            .build()
            .expect("Couldn't parse ignore file");

        let mut builder = WalkBuilder::new(path);
        builder.hidden(false)
            .overrides(overrides);

        for entry in builder.build() {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_dir() {
                        // TODO: Prevent adding multiple watches to the same directory.
                        let wd = self.inotify.add_watch(path, mask).or_else(|e| {
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

    /// Construct a `Command` for the given input.
    fn get_command(&mut self, next: &dyn AsRef<Path>, output: Option<PipeWriter>) -> Result<Command, String> {
        let mut c = Command::new(&self.command);
        c.args(self.args
            .iter()
            .map(|e| e.replace("{}", &next.as_ref().to_string_lossy()))
            .collect::<Vec<String>>());

        if let Some(writer) = output {
            c.stdout(writer.try_clone().unwrap());
            c.stderr(writer.try_clone().unwrap());
        }

        Ok(c)
    }

    /// Given a file name, build and start a child process.
    fn start(&mut self, next: &dyn AsRef<Path>, output: Option<PipeWriter>) -> Result<Arc<SharedChild>, String> {
        let mut command = self.get_command(next, output)?;
        let child = SharedChild::spawn(&mut command)
            .map_err(|e| format!("Spawn error: {}", e.to_string()))?;

        let started = Arc::new(child);

        let wait_clone = started.clone();

        std::thread::spawn(move || wait_clone.wait().unwrap());

        Ok(started)
    }
}

#[cfg(test)]
mod test {
    // use tempfile::{ tempdir, TempDir };
    use std::error::Error;
    use os_pipe::{pipe};
    use std::io::Read;
    // use std::fs::File;
    // use inotify::{EventStream};
    //use futures::stream::Stream;
    // use std::io::{ BufReader, BufRead, Write };

    #[test]
    fn construction() {
        assert_eq!(super::Runtime::new(vec!()).err().unwrap(), "Empty template string!");
        assert!(super::Runtime::new(vec!("echo", "{}").into_iter().map(str::to_string).collect()).is_ok());
    }

    #[test]
    fn command_spawning() -> Result<(), Box<dyn Error>> {
        let (mut reader, writer) = pipe()?;
        let mut runtime = super::Runtime::new(vec!("echo",  "Test", "{}").into_iter().map(str::to_string).collect())?;
        let tracker = runtime.start(&"Hello.txt", Some(writer))?;
        let mut output = String::new();
        reader.read_to_string(&mut output)?;
        assert!(tracker.wait()?.success());
        assert_eq!("Test Hello.txt\n", output);
        Ok(())
    }
/*
    #[test]
    fn watch_directories() -> Result<(), Box<dyn Error>> {
        let (reader, writer) = pipe()?;
        let mut reader = BufReader::new(reader);
        let dir: TempDir = tempdir().unwrap();
        let mut runtime = super::Runtime::new(vec!("echo", "Test", "{}").into_iter().map(str::to_string).collect())?;
        let mut stream: EventStream<_> = runtime.get_stream()?;
        runtime.watch_directories(&dir)?;
        let tmp_path = dir.path().join("Fake.txt");

        // A file create isn't special, it should be an ignored event.
        File::create(&tmp_path)
            .expect("Failed to create temp file");

        let event = stream.wait().next().unwrap().unwrap();
        let path = runtime.get_event_path(&event);
        assert_eq!(tmp_path, path.unwrap());

        assert!(runtime.is_executable_event(&event).is_none());
        assert!(runtime.is_watchable_dir(&event).is_none());

        // A file close_write is special, should trigger runs.
        let mut temp_file = std::fs::OpenOptions::new()
            .write(true)
            .open(&tmp_path)?;
        temp_file.write_all(&vec!(0u8))?;
        temp_file.sync_all()?;

        let event = stream.by_ref().wait().next().unwrap().unwrap();
        let path = runtime.get_event_path(&event);

        assert_eq!(tmp_path, path.unwrap());
        assert!(runtime.is_executable_event(&event).is_some());
        assert!(runtime.is_watchable_dir(&event).is_none());

        runtime.process_event(&event, Some(writer));
        let mut output = String::new();
        reader.read_line(&mut output)?;
        assert_eq!(format!("Test {}\n", tmp_path.display()), output);
        Ok(())
    }
    */
}

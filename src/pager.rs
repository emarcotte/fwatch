use shared_child::SharedChild;
use std::sync::Arc;
use std::process::{ Command, };
use os_pipe::{ pipe, PipeWriter, };

pub struct Pager {
    running: Option<Arc<SharedChild>>,
}

impl Pager {
    pub fn new() -> Pager {
        Pager {
            running: None,
        }
    }

    pub fn start(&mut self) -> Result<PipeWriter, String> {
        let (input, output) = pipe()
            .map_err(|e| format!("Pipe creation error: {}", e.to_string()))?;

        let mut c = Command::new("less");
        c.args(vec!("-SRXKc~", "--mouse"));
        c.stdin(input);
        let proc = SharedChild::spawn(&mut c).unwrap();
        let started = Arc::new(proc);
        let wait_clone = started.clone();
        self.running = Some(started);
        std::thread::spawn(move || {
            wait_clone.wait().unwrap();
        });
        Ok(output)
    }

    pub fn stop(&mut self) {
        if let Some(running) = &self.running {
            running.kill().unwrap();
        }

        self.running = None;
    }
}

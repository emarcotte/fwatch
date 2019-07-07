/*
use pager2::Pager2;
use std::sync::{Arc};
use std::thread::{spawn, JoinHandle};
use std::io::{BufReader, BufRead, };
use shared_child::SharedChild;
use std::process::Command;
use os_pipe::pipe;

fn run(pager: Arc<Pager2>) -> (Arc<SharedChild>, JoinHandle<()>) {
    let (reader, writer) = pipe().unwrap();
    let child = Arc::new(SharedChild::spawn(Command::new("cargo")
                                            .arg("test")
                                            .arg("--color=always")
                                            .stdout(writer.try_clone().unwrap())
                                            .stderr(writer))
                         .unwrap());

    let monitor = child.clone();

    spawn(move || {
        monitor.wait().unwrap();
    });

    let th = spawn(move || {
        let mut buffer = BufReader::new(reader);
        loop {
            let mut buf = String::new();
            match buffer.read_line(&mut buf) {
                Ok(0)  => { pager.add("EOF".to_owned()); break; },
                Ok(_)  => { pager.add(buf); },
                Err(e) => { pager.add(format!("Error: {}", e)); break; },
            };
        };
    });

    (child, th)
}
*/

use std::error::Error;
fn main() -> Result<(), Box<Error>> {
/*
    let p = Arc::new(Pager2::new());

    let (_, th) = run(p.clone());

    p.run();

    th.join().unwrap();
*/
    Ok(())
}

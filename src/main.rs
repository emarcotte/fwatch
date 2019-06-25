mod fwatch;

use clap::{App, Arg};
use fwatch::Runtime;
use regex::Regex;

fn main() -> Result<(), String> {
    parse_cli()?.run()?;
    Ok(())
}

fn parse_cli() -> Result<Runtime, String> {
    let matches = App::new("fwatch")
        .version("1.0")
        .about("Watch files")
        .arg(
            Arg::with_name("ext")
                .long("ext")
                .short("e")
                .value_name("extension")
                .takes_value(true)
                .help("filter files to a file extension"),
        )
        .arg(
            Arg::with_name("regex")
                .long("regex")
                .value_name("extension")
                .takes_value(true)
                .help("filter files by regex"),
        )
        .arg(Arg::with_name("slop").multiple(true).last(true))
        .get_matches();

    let slop = matches
        .values_of("slop")
        .map(|e| e.map(|e| e.to_string()).collect::<Vec<String>>());

    let template = match slop {
        Some(opts) => Ok(opts),
        None => Err("No command provided".to_string()),
    }?;

    let extension = matches.value_of("ext").map(|e| e.to_string());

    let regex = match matches.value_of("regex").map(|e| Regex::new(e)) {
        Some(Err(e)) => Err(e.to_string()),
        Some(Ok(r)) => Ok(Some(r)),
        None => Ok(None),
    }?;

    Ok(Runtime::new(template, extension, regex))
}

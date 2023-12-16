mod fwatch;
mod pager;

use clap::{App, AppSettings, Arg, Shell, SubCommand, };
use fwatch::Runtime;
use regex::Regex;
use std::error::Error;

enum CommandInput {
    Run(Runtime, Vec<String>),
    Completions,
}

#[tokio::main]
async fn main() {
    match parse_cli() {
        Err(e) => {
            eprintln!("CLI Error: {}", e);
        }
        Ok(CommandInput::Run(runtime, dirs)) => {
            match runtime.run().await {
                Err(e) => eprintln!("Top level error {:?}", e),
                Ok(_)  => {},
            };
        },
        Ok(CommandInput::Completions)  => {
            build_cli()
                .gen_completions_to(
                    "fwatch",
                    Shell::Bash,
                    &mut std::io::stdout());
        },
    };
}

fn build_cli() -> App<'static, 'static> {
    App::new("fwatch")
        .version(clap::crate_version!())
        .about("Watch files")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("completions")
            .about("Generates bash completions"))
        .subcommand(SubCommand::with_name("run")
                    .arg(Arg::with_name("dirs")
                         .help("Directories to monitor for changes recursively")
                         .multiple(true)
                         .required(true)
                         .min_values(1))
                    .arg(Arg::with_name("pager")
                         .long("pager")
                         .short("p")
                         .help("Run with a pager"))
                    .arg(Arg::with_name("ext")
                         .long("ext")
                         .short("e")
                         .value_name("extension")
                         .takes_value(true)
                         .help("filter files to a file extension"))
                    .arg(Arg::with_name("regex")
                         .long("regex")
                         .value_name("extension")
                         .takes_value(true)
                         .help("filter files by regex"))
                    .arg(Arg::with_name("command")
                         .help("The template command to run on changes. Allows for a single placeholder '{}' to input the file name into.")
                         .multiple(true)
                         .min_values(1)
                         .required(true)
                         .last(true)))
}

fn parse_cli() -> Result<CommandInput, Box<dyn Error>> {
    match build_cli().get_matches().subcommand() {
        ("completions", _) => Ok(CommandInput::Completions),
        ("run", Some(matches)) => {
            let mut runtime = Runtime::new(
                matches.values_of("command")
                .ok_or_else(|| "No template provided")?
                .map(str::to_string)
                .collect())?;


            matches.value_of("ext")
                .map(|e| runtime.set_extension(e.to_string()));

            matches.value_of("regex")
                .map(|re| runtime.set_regex(Regex::new(re).expect("Invalid regex")));

            let dirs = matches.values_of("dirs")
                .ok_or_else(|| "No dirs provided")?
                .map(str::to_string)
                .collect();

            runtime.use_pager(matches.is_present("pager"))?;

            Ok(CommandInput::Run(runtime, dirs))
        }
        (_, _) => unimplemented!(),
    }
}

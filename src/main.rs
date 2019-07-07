mod fwatch;
mod pager;
mod pager2;

use clap::{App, AppSettings, Arg, Shell, SubCommand, };
use fwatch::Runtime;
use regex::Regex;

enum CommandInput {
    Run(Runtime),
    Completions,
}

fn main() -> Result<(), Box<std::error::Error>> {
    match parse_cli()? {
        CommandInput::Run(runtime) => {
            runtime.run()?;
        },
        CommandInput::Completions  => {
            build_cli().gen_completions_to("fwatch", Shell::Bash, &mut std::io::stdout());
        },
    };

    Ok(())
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

fn parse_cli() -> Result<CommandInput, String> {
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

            matches.values_of("dirs")
                .ok_or_else(|| "No dirs provided")?
                .for_each(|dir| {
                    runtime.watch_directories(&dir)
                        .expect("Error watching directories");
                });

            runtime.use_pager(matches.is_present("pager"));

            Ok(CommandInput::Run(runtime))
        }
        (_, _) => unimplemented!(),
    }
}

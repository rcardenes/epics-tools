use clap::{Command, arg};
use epics_tools::UnifiedResult;
use epics_tools::config::DEFAULT_WAIT_TIME;
use futures::TryFutureExt;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug)]
struct Config {
    names: Vec<String>,
    wait_time: f32,
    // Flags
    asynchronous: bool,
    enum_as_number: bool,
    terse: bool,
    wide: bool,
}

fn wait_time_in_range(s: &str) -> Result<f32, String> {
    let time: f32 = s
        .parse()
        .map_err(|_| format!("The wait time must be a real number"))?;
    if time > 0.0 {
        Ok(time)
    } else {
        Err(format!("Wait time must be a positive value"))
    }
}

async fn get_arguments() -> UnifiedResult<Config> {
    let matches = Command::new(PKG_NAME)
        .version(PKG_VERSION)
        .author(PKG_AUTHORS)
        .about("Rust caget")
        .args([
            arg!(wait: -w <sec> "-w <sec>: Wait time, specifies CA timeout")
                .default_value(DEFAULT_WAIT_TIME)
                .value_parser(wait_time_in_range),
            arg!(asget: -c "Asynchronous get (use a callback and wait for completion)"),
            arg!(terse: -t "Terse mode - print only value, without name"),
            arg!(wide: -a "Wide mode \"name timestamp value stat sevr\""),
            arg!(names: <PV> ... "PV names"),
        ])
        .get_matches();

    let names = matches.get_many::<String>("names")
        .unwrap()
        .cloned()
        .collect();
    let wait_time = *matches.get_one::<f32>("wait").unwrap();

    Ok(Config {
        names,
        wait_time,
        asynchronous: matches.get_flag("asget"),
        enum_as_number: true, // TODO: This needs to be implemented
        terse: matches.get_flag("terse"),
        wide: matches.get_flag("wide"),
    })
}

async fn run(config: Config) -> UnifiedResult<()> {
    println!("{config:#?}");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = get_arguments().and_then(run).await {
        eprintln!("{e:?}");
    }
}
//! `georgian-for-garmin`: a thin CLI over the core operations.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::{Error, ErrorCode, connect, ops};
use serde_json::{Map, Value, json};

const EXAMPLES: &str = "\
Examples:
  georgian-for-garmin inspect
  georgian-for-garmin install
  georgian-for-garmin install --size-px 24 --json
  echo '{\"size_px\": 27}' | georgian-for-garmin restore-stock --input -
  georgian-for-garmin --help --json        # MCP tool definitions

After install or restore-stock, disconnect the USB cable and restart the watch.
Quit Garmin Express first; it holds the watch's USB connection.
Set GEORGIAN_FOR_GARMIN_TRACE=1 to print each MTP container and USB transfer to stderr.";

/// Install a Georgian-extended font on a Garmin watch over USB.
#[derive(Parser)]
#[command(name = "georgian-for-garmin", version, after_help = EXAMPLES)]
struct Cli {
    /// Print one JSON value on stdout; errors go to stderr as {"error": {"code", "message"}}
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the watch model, supported font sizes and what is staged (read-only)
    Inspect(InputArgs),
    /// Stage the Georgian font on the watch and verify it
    Install(StageArgs),
    /// Stage Garmin's original font on the watch and verify it
    RestoreStock(StageArgs),
}

impl Command {
    fn operation(&self) -> &'static str {
        match self {
            Command::Inspect(_) => "inspect",
            Command::Install(_) => "install",
            Command::RestoreStock(_) => "restore_stock",
        }
    }
}

#[derive(Args)]
struct InputArgs {
    /// Input as a JSON object keyed like the operation's schema, from a file or - for stdin
    #[arg(long, value_name = "FILE|-")]
    input: Option<String>,
}

#[derive(Args)]
struct StageArgs {
    /// Font size in pixels: 21, 24, 27 or 31 [default: 27]
    #[arg(long, value_name = "PX")]
    size_px: Option<u16>,

    #[command(flatten)]
    input: InputArgs,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidInput, message)
}

/// The `--input` object with flags laid over it.
fn build_input(input: &InputArgs, flags: Map<String, Value>) -> Result<Value, Error> {
    let mut object = match input.input.as_deref() {
        None => Map::new(),
        Some(source) => {
            let text = if source == "-" {
                let mut text = String::new();
                io::stdin()
                    .read_to_string(&mut text)
                    .map_err(|e| invalid(format!("cannot read stdin: {e}")))?;
                text
            } else {
                std::fs::read_to_string(source)
                    .map_err(|e| invalid(format!("cannot read {source}: {e}")))?
            };
            match serde_json::from_str(&text)
                .map_err(|e| invalid(format!("--input is not valid JSON: {e}")))?
            {
                Value::Object(object) => object,
                _ => return Err(invalid("--input must be a JSON object")),
            }
        }
    };
    object.extend(flags);
    Ok(Value::Object(object))
}

fn request(command: &Command) -> Result<Value, Error> {
    match command {
        Command::Inspect(input) => build_input(input, Map::new()),
        Command::Install(args) | Command::RestoreStock(args) => {
            let mut flags = Map::new();
            if let Some(px) = args.size_px {
                flags.insert("size_px".into(), json!(px));
            }
            build_input(&args.input, flags)
        }
    }
}

fn print_human(operation: &str, out: &Value) {
    if operation != "inspect" {
        let verb = if out["status"] == "already_staged" {
            "Already staged"
        } else {
            "Staged and verified"
        };
        println!(
            "{verb}: {} ({} font, {} bytes, SHA-256 {}).",
            out["path"].as_str().unwrap_or_default(),
            out["variant"].as_str().unwrap_or_default(),
            out["bytes"],
            out["sha256"].as_str().unwrap_or_default()
        );
        println!("{}", out["next_step"].as_str().unwrap_or_default());
        return;
    }
    let device = &out["device"];
    println!(
        "{} ({}), software {}",
        device["model"].as_str().unwrap_or_default(),
        device["part_number"].as_str().unwrap_or_default(),
        device["software_version"].as_str().unwrap_or_default()
    );
    match out["extdata"].as_object() {
        None => println!("ExtData staging folder: not found"),
        Some(ext) => {
            let files = ext["files"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            println!(
                "ExtData staging folder: {} on storage {}, {} the GARMIN listing (found by {}); {} staged file(s)",
                ext["handle"].as_str().unwrap_or_default(),
                ext["storage_id"].as_str().unwrap_or_default(),
                if ext["listed_under_garmin"] == true {
                    "in"
                } else {
                    "hidden from"
                },
                ext["found_by"]
                    .as_str()
                    .unwrap_or_default()
                    .replace('_', " "),
                files.len()
            );
            for file in files {
                println!(
                    "  {} ({} bytes)",
                    file["name"].as_str().unwrap_or_default(),
                    file["bytes"]
                );
            }
        }
    }
    println!("Fonts:");
    for font in out["fonts"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        println!(
            "  {:>2} px  {}  {:<13}  staged: {}{}",
            font["size_px"],
            font["part"].as_str().unwrap_or_default(),
            if font["supported"] == true {
                "supported"
            } else {
                "not supported"
            },
            font["staged"].as_str().unwrap_or_default(),
            if font["default"] == true {
                "  (default)"
            } else {
                ""
            }
        );
    }
    if out["supported"] != true {
        println!("This watch accepts none of the fonts in this build.");
    }
}

fn fail(error: &Error, json: bool) -> ExitCode {
    if json {
        eprintln!("{}", error.to_json());
    } else {
        eprintln!("error: {error}");
    }
    ExitCode::from(error.exit_code() as u8)
}

fn print_json(value: &Value) {
    let mut stdout = io::stdout().lock();
    let _ = serde_json::to_writer_pretty(&mut stdout, value);
    let _ = writeln!(stdout);
}

/// `--help --json`: MCP tool definitions, for one subcommand if one is named.
fn tool_help(args: &[OsString]) -> Value {
    let named = args.iter().skip(1).find_map(|a| match a.to_str()? {
        "inspect" => Some("inspect"),
        "install" => Some("install"),
        "restore-stock" => Some("restore_stock"),
        _ => None,
    });
    match named.and_then(ops::operation) {
        Some(op) => json!({"tools": [op.tool()]}),
        None => ops::tools(),
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let options = args.iter().skip(1).take_while(|a| *a != "--");
    let json = options.clone().any(|a| a == "--json");
    if json && options.clone().any(|a| a == "-h" || a == "--help") {
        print_json(&tool_help(&args));
        return ExitCode::SUCCESS;
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            let _ = e.print();
            return ExitCode::SUCCESS;
        }
        Err(e) if json => {
            let rendered = e.render().to_string();
            let message = rendered
                .lines()
                .next()
                .unwrap_or_default()
                .trim_start_matches("error: ");
            return fail(&invalid(message), true);
        }
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(2);
        }
    };
    let operation = cli.command.operation();
    let result = request(&cli.command)
        .and_then(|input| ops::call(operation, input, &FontSet::bundled(), &mut connect));
    match result {
        Ok(out) if cli.json => {
            print_json(&out);
            ExitCode::SUCCESS
        }
        Ok(out) => {
            print_human(operation, &out);
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e, cli.json),
    }
}

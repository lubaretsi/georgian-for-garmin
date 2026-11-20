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
  georgian-for-garmin inspect --report     # the watch's font list, to paste into an issue
  georgian-for-garmin install              # every size of the watch's pack
  georgian-for-garmin install --sizes-px 24,27 --json
  echo '{\"sizes_px\": [27]}' | georgian-for-garmin restore-stock --input -
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
    /// Show the watch model, its font pack, whether it was tested, and what is staged (read-only)
    Inspect(InspectArgs),
    /// Stage the Georgian font in every size of the watch's pack and verify it
    Install(StageArgs),
    /// Stage Garmin's original font in every size of the watch's pack and verify it
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
struct InspectArgs {
    /// Print only the watch report (model, software, font list; no unit ID or serial number)
    /// as Markdown for a GitHub issue; with --json, the report object
    #[arg(long)]
    report: bool,

    #[command(flatten)]
    input: InputArgs,
}

#[derive(Args)]
struct StageArgs {
    /// Only these pixel sizes, comma-separated [default: every size of the pack the watch lists]
    #[arg(long, value_name = "PX,...", value_delimiter = ',', num_args = 1..)]
    sizes_px: Vec<u16>,

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
        Command::Inspect(args) => build_input(&args.input, Map::new()),
        Command::Install(args) | Command::RestoreStock(args) => {
            let mut flags = Map::new();
            if !args.sizes_px.is_empty() {
                flags.insert("sizes_px".into(), json!(args.sizes_px));
            }
            build_input(&args.input, flags)
        }
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn items(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn print_human(operation: &str, out: &Value) {
    if operation != "inspect" {
        println!(
            "{} font, pack {}:",
            if out["variant"] == "georgian" {
                "Georgian"
            } else {
                "Original"
            },
            text(&out["pack"])
        );
        for part in items(&out["parts"]) {
            println!(
                "  {:>2} px  {}  {}  ({} bytes, SHA-256 {})",
                part["size_px"],
                text(&part["path"]),
                if part["status"] == "already_staged" {
                    "already staged"
                } else {
                    "staged and verified"
                },
                part["bytes"],
                text(&part["sha256"])
            );
        }
        println!("{}", text(&out["next_step"]));
        return;
    }
    let device = &out["device"];
    println!(
        "{} ({}), software {}",
        text(&device["model"]),
        text(&device["part_number"]),
        text(&device["software_version"])
    );
    match out["pack"].as_object() {
        None => println!("Font pack: none; this watch lists none of the packs in this build."),
        Some(pack) => {
            let tested = &pack["tested"];
            let parts: Vec<&str> = items(&tested["verified_parts"]).iter().map(text).collect();
            let software: Vec<&str> = items(&tested["verified_software"])
                .iter()
                .map(text)
                .collect();
            println!("Font pack: {} ({})", text(&pack["name"]), text(&pack["id"]));
            match text(&tested["status"]) {
                "tested" => println!("Tested on this model and software ({}).", parts.join(", ")),
                "tested_other_software" => println!(
                    "Tested on this model with software {} ({}), not on this version.",
                    software.join(", "),
                    parts.join(", ")
                ),
                _ => println!("Not tested on this model yet."),
            }
        }
    }
    match out["extdata"].as_object() {
        None => println!("ExtData staging folder: not found"),
        Some(ext) => {
            let files = items(&ext["files"]);
            println!(
                "ExtData staging folder: {} on storage {}, {} the GARMIN listing (found by {}); {} staged file(s)",
                text(&ext["handle"]),
                text(&ext["storage_id"]),
                if ext["listed_under_garmin"] == true {
                    "in"
                } else {
                    "hidden from"
                },
                text(&ext["found_by"]).replace('_', " "),
                files.len()
            );
            for file in files {
                println!("  {} ({} bytes)", text(&file["name"]), file["bytes"]);
            }
        }
    }
    println!("Fonts in this build:");
    for font in items(&out["fonts"]) {
        println!(
            "  {:>2} px  {}  {:<26}  {:<13}  staged: {}",
            font["size_px"],
            text(&font["part"]),
            text(&font["pack"]),
            if font["target"] == true {
                "installs"
            } else if font["listed"] == true {
                "listed"
            } else {
                "not listed"
            },
            text(&font["staged"])
        );
    }
    if out["supported"] != true {
        println!("Nothing in this build can be installed on this watch.");
    }
    if out["pack"]["tested"]["status"] != "tested" {
        println!(
            "To help add this watch, share its font list: georgian-for-garmin inspect --report"
        );
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
    let report_only = matches!(&cli.command, Command::Inspect(args) if args.report);
    let result = request(&cli.command)
        .and_then(|input| ops::call(operation, input, &FontSet::bundled(), &mut connect));
    match result {
        Ok(out) if report_only && cli.json => {
            print_json(&out["report"]);
            ExitCode::SUCCESS
        }
        Ok(out) if report_only => {
            print!("{}", text(&out["report"]["text"]));
            ExitCode::SUCCESS
        }
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

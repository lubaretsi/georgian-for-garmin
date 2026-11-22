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
  georgian-for-garmin inspect --report     # the watch report, as in the app's GitHub issue
  georgian-for-garmin inspect-drive --report   # the same for a watch mounted as a USB drive
  georgian-for-garmin install              # every listed part of every pack
  georgian-for-garmin install --parts 006-D9035-02,006-D9035-03 --json
  echo '{\"parts\": [\"006-D9035-03\"]}' | georgian-for-garmin restore-stock --input -
  georgian-for-garmin inspect --report --json | jq '{report: ., answers: {result: \"georgian\"}}' \\
    | georgian-for-garmin issue-link --input -   # a prefilled GitHub issue link
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
    /// Show the watch model, its font packs, whether they were tested, and what is staged (read-only)
    Inspect(InspectArgs),
    /// Stage the Georgian font in every part of the bundled packs the watch lists and verify it
    Install(StageArgs),
    /// Stage Garmin's original font in every part of the bundled packs the watch lists and verify it
    RestoreStock(StageArgs),
    /// Read a watch that connects as a USB drive instead of MTP (read-only; nothing installs)
    InspectDrive(InspectDriveArgs),
    /// Print the GitHub link that files a watch report with the issue form filled in
    IssueLink(InputArgs),
}

impl Command {
    fn operation(&self) -> &'static str {
        match self {
            Command::Inspect(_) => "inspect",
            Command::Install(_) => "install",
            Command::RestoreStock(_) => "restore_stock",
            Command::InspectDrive(_) => "inspect_drive",
            Command::IssueLink(_) => "issue_link",
        }
    }

    /// Whether only the watch report is printed.
    fn report_only(&self) -> bool {
        match self {
            Command::Inspect(args) => args.report,
            Command::InspectDrive(args) => args.report,
            _ => false,
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
    /// Print only the watch report (no unit ID or serial number), the text the app shares in a
    /// GitHub issue; with --json, the report object
    #[arg(long)]
    report: bool,

    #[command(flatten)]
    input: InputArgs,
}

#[derive(Args)]
struct InspectDriveArgs {
    /// The watch volume's root, e.g. /Volumes/GARMIN or E:\ [default: the one mounted volume
    /// with GARMIN/GarminDevice.xml]
    #[arg(long)]
    path: Option<String>,

    /// Print only the watch report; with --json, the report object
    #[arg(long)]
    report: bool,

    #[command(flatten)]
    input: InputArgs,
}

#[derive(Args)]
struct StageArgs {
    /// Only these part numbers, comma-separated, e.g. 006-D9035-03 [default: every part of the
    /// bundled packs that the watch lists; `inspect` shows them]
    #[arg(long, value_name = "PART,...", value_delimiter = ',', num_args = 1..)]
    parts: Vec<String>,

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
        Command::IssueLink(input) => build_input(input, Map::new()),
        Command::InspectDrive(args) => {
            let mut flags = Map::new();
            if let Some(path) = &args.path {
                flags.insert("path".into(), json!(path));
            }
            build_input(&args.input, flags)
        }
        Command::Install(args) | Command::RestoreStock(args) => {
            let mut flags = Map::new();
            if !args.parts.is_empty() {
                flags.insert("parts".into(), json!(args.parts));
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

/// A part's size column: "27 px" for a bitmap part, "TrueType" for a TrueType part.
fn size(part: &Value) -> String {
    match part["size_px"].as_u64() {
        Some(px) => format!("{px:>2} px"),
        None => "TrueType".into(),
    }
}

fn print_human(operation: &str, out: &Value) {
    match operation {
        "issue_link" => {
            println!("{}", text(&out["url"]));
            if out["report_in_url"] != true {
                eprintln!(
                    "The report is too long for the link; paste report_text (see --json) into \
                     the form's report field."
                );
            }
            return;
        }
        "inspect_drive" => {
            let device = &out["device"];
            println!(
                "{} ({}), software {}, connected as a USB drive at {}",
                text(&device["model"]),
                text(&device["part_number"]),
                text(&device["software_version"]),
                text(&out["path"])
            );
            println!("{}", text(&out["hosts"]["reason"]));
            println!(
                "Nothing can be installed while the watch is a USB drive; it needs MTP. To share \
                 its font list: georgian-for-garmin inspect-drive --report"
            );
            return;
        }
        _ => {}
    }
    if operation != "inspect" {
        println!(
            "{} font:",
            if out["variant"] == "georgian" {
                "Georgian"
            } else {
                "Original"
            }
        );
        for pack in items(&out["packs"]) {
            println!("  {} ({})", text(&pack["name"]), text(&pack["id"]));
            for part in items(&pack["parts"]) {
                println!(
                    "    {:>8}  {}  {}  ({} bytes, SHA-256 {})",
                    size(part),
                    text(&part["path"]),
                    if part["status"] == "already_staged" {
                        "already there"
                    } else {
                        "written and verified"
                    },
                    part["bytes"],
                    text(&part["sha256"])
                );
            }
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
    println!("{}", text(&out["hosts"]["reason"]));
    let packs = items(&out["packs"]);
    if packs.is_empty() {
        println!("Font packs: none; this watch lists none of the packs in this build.");
    } else {
        println!("Font packs the watch lists (install writes each listed part):");
    }
    for pack in packs {
        println!(
            "  {} ({}, weight {})",
            text(&pack["name"]),
            text(&pack["id"]),
            pack["weight"]
        );
        for part in items(&pack["parts"]) {
            println!(
                "    {:>8}  {}  {}",
                size(part),
                text(&part["part"]),
                text(&part["tested"]).replace('_', " ")
            );
        }
    }
    let tested = &out["tested"];
    if !packs.is_empty() {
        match text(&tested["status"]) {
            "tested" => println!("Every part was tested on this model and software."),
            "tested_other_software" => println!(
                "Every part was tested on this model, but not all on this software (tested on {}).",
                items(&tested["verified_software"])
                    .iter()
                    .map(text)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => println!("Not every part was tested on this model yet."),
        }
    }
    match out["extdata"].as_object() {
        None => println!("ExtData folder: not found"),
        Some(ext) => {
            let files = items(&ext["files"]);
            println!(
                "ExtData folder: {} on storage {}, {} the GARMIN listing (found by {}); {} file(s)",
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
            "  {:>8}  {}  {:<36}  {:<10}  file: {}",
            size(font),
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
    if out["tested"]["status"] != "tested" {
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
        "inspect-drive" => Some("inspect_drive"),
        "issue-link" => Some("issue_link"),
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
    let report_only = cli.command.report_only();
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

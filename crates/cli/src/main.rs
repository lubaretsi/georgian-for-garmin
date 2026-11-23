//! `georgian-for-garmin`: a thin CLI over the core operations, and its `rpc` and `mcp`
//! sessions.

#[cfg(test)]
#[path = "../../core/tests/support/fake.rs"]
mod fake;
mod mcp;

use std::borrow::Cow;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use georgian_for_garmin_core::fonts::FontSet;
use georgian_for_garmin_core::session::{Session, Worker};
use georgian_for_garmin_core::{Error, ErrorCode, connect, ops, report, rpc};
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
  georgian-for-garmin extdata-list --hash 006-D9035-03.fnt   # every ExtData file, hidden ones too
  georgian-for-garmin wait-for-watch --timeout-seconds 120   # after a restart
  echo '{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"inspect\"}' | georgian-for-garmin rpc
  georgian-for-garmin mcp                  # an MCP server on stdio
  georgian-for-garmin --help --json        # MCP tool definitions

After install or restore-stock, disconnect the USB cable and restart the watch.
Quit Garmin Express first; it holds the watch's USB connection. rpc and mcp keep the watch open
until they end.
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
    /// Show the watch model, its support status, its font packs, whether they were tested, and what is staged (read-only)
    Inspect(InspectArgs),
    /// Stage the Georgian font in every part of the bundled packs the watch lists and verify it
    Install(StageArgs),
    /// Stage Garmin's original font in every part of the bundled packs the watch lists and verify it
    RestoreStock(StageArgs),
    /// Read a watch that connects as a USB drive instead of MTP (read-only; nothing installs)
    InspectDrive(InspectDriveArgs),
    /// Print the GitHub link that files a watch report with the issue form filled in
    IssueLink(InputArgs),
    /// List every file in the watch's ExtData folder, hidden ones included (read-only)
    ExtdataList(ExtdataListArgs),
    /// Wait until a watch can be opened, e.g. after a restart (read-only)
    WaitForWatch(WaitArgs),
    /// Copy one ExtData file to a new local file (development builds; reads the watch only)
    #[cfg(feature = "dev")]
    ReadFile(ReadFileArgs),
    /// Stage a local file as ExtData/<part>.tmp and read it back (development builds)
    #[cfg(feature = "dev")]
    StageFile(StageFileArgs),
    /// Delete a file stage-file or mtp-create wrote, by its id (development builds)
    #[cfg(feature = "dev")]
    RemoveStaged(RemoveStagedArgs),
    /// Print the watch's MTP GetDeviceInfo (development builds; reads the watch only)
    #[cfg(feature = "dev")]
    MtpDeviceInfo(InputArgs),
    /// Print the watch's MTP storage IDs (development builds; reads the watch only)
    #[cfg(feature = "dev")]
    MtpStorageIds(InputArgs),
    /// Print one storage's MTP GetStorageInfo (development builds; reads the watch only)
    #[cfg(feature = "dev")]
    MtpStorageInfo(StorageArgs),
    /// List object handles in a folder, hidden ones too with --hidden (development builds;
    /// reads the watch only)
    #[cfg(feature = "dev")]
    MtpHandles(HandlesArgs),
    /// Print one object's MTP GetObjectInfo (development builds; reads the watch only)
    #[cfg(feature = "dev")]
    MtpObjectInfo(HandleArgs),
    /// Read an object, or a range of it, as base64 or to a new local file (development builds;
    /// reads the watch only)
    #[cfg(feature = "dev")]
    MtpRead(MtpReadArgs),
    /// Send one read-only PTP transaction (USB only; development builds; reads the watch only)
    #[cfg(feature = "dev")]
    PtpTransaction(PtpTransactionArgs),
    /// Create one file in ExtData after stage-file's checks; needs --allow-write (development
    /// builds)
    #[cfg(feature = "dev")]
    MtpCreate(MtpCreateArgs),
    /// Delete one identified file in ExtData, for good; needs --allow-write (development builds)
    #[cfg(feature = "dev")]
    MtpDelete(MtpDeleteArgs),
    /// Serve the operations as JSON-RPC 2.0, one message per line on stdin and stdout, keeping
    /// the watch open until stdin ends
    Rpc,
    /// Serve the operations as MCP tools on stdin and stdout, keeping the watch open until the
    /// client disconnects
    Mcp,
}

impl Command {
    fn operation(&self) -> &'static str {
        match self {
            Command::Inspect(_) => "inspect",
            Command::Install(_) => "install",
            Command::RestoreStock(_) => "restore_stock",
            Command::InspectDrive(_) => "inspect_drive",
            Command::IssueLink(_) => "issue_link",
            Command::ExtdataList(_) => "extdata_list",
            Command::WaitForWatch(_) => "wait_for_watch",
            #[cfg(feature = "dev")]
            Command::ReadFile(_) => "read_file",
            #[cfg(feature = "dev")]
            Command::StageFile(_) => "stage_file",
            #[cfg(feature = "dev")]
            Command::RemoveStaged(_) => "remove_staged",
            #[cfg(feature = "dev")]
            Command::MtpDeviceInfo(_) => "mtp_device_info",
            #[cfg(feature = "dev")]
            Command::MtpStorageIds(_) => "mtp_storage_ids",
            #[cfg(feature = "dev")]
            Command::MtpStorageInfo(_) => "mtp_storage_info",
            #[cfg(feature = "dev")]
            Command::MtpHandles(_) => "mtp_handles",
            #[cfg(feature = "dev")]
            Command::MtpObjectInfo(_) => "mtp_object_info",
            #[cfg(feature = "dev")]
            Command::MtpRead(_) => "mtp_read",
            #[cfg(feature = "dev")]
            Command::PtpTransaction(_) => "ptp_transaction",
            #[cfg(feature = "dev")]
            Command::MtpCreate(_) => "mtp_create",
            #[cfg(feature = "dev")]
            Command::MtpDelete(_) => "mtp_delete",
            Command::Rpc | Command::Mcp => unreachable!("sessions are served before operations"),
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

#[derive(Args)]
struct ExtdataListArgs {
    /// Read these files and print their SHA-256s: names, comma-separated, e.g.
    /// 006-D9035-03.fnt
    #[arg(long, value_name = "NAME,...", value_delimiter = ',', num_args = 1..)]
    hash: Vec<String>,

    /// Files to skip, in name order [default: 0]
    #[arg(long)]
    offset: Option<u64>,

    /// Files to list at most, 1 to 1000 [default: 200]
    #[arg(long)]
    limit: Option<u64>,

    /// Write the whole list to this JSON file instead of printing a page
    #[arg(long, value_name = "FILE")]
    output_file: Option<String>,

    #[command(flatten)]
    input: InputArgs,
}

#[derive(Args)]
struct WaitArgs {
    /// Seconds to wait at most, up to 3600; 0 checks once [default: 60]
    #[arg(long, value_name = "SECONDS")]
    timeout_seconds: Option<f64>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct ReadFileArgs {
    /// The file's name in ExtData, from either listing
    #[arg(long)]
    name: Option<String>,

    /// The local file to create; it must not exist
    #[arg(long)]
    path: Option<String>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct StageFileArgs {
    /// The local file to stage
    #[arg(long, value_name = "FILE")]
    source: Option<String>,

    /// The name in ExtData: <part>.tmp for a part the watch declares, e.g. 006-D8448-70.tmp or
    /// D844870.tmp
    #[arg(long)]
    name: Option<String>,

    /// Check everything and write nothing
    #[arg(long)]
    dry_run: bool,

    /// SHA-256s of Garmin's own file for this part, comma-separated: such a file may be replaced
    #[arg(long, value_name = "HEX,...", value_delimiter = ',', num_args = 1..)]
    garmin_sha256: Vec<String>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct RemoveStagedArgs {
    /// The id stage-file or mtp-create printed
    #[arg(long)]
    id: Option<String>,

    #[command(flatten)]
    input: InputArgs,
}

/// A number in decimal or with a 0x prefix, as handles and operation codes are often written.
#[cfg(feature = "dev")]
fn number<T: TryFrom<u64>>(text: &str) -> Result<T, String> {
    let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .map_err(|e| e.to_string())?;
    T::try_from(value).map_err(|_| format!("{text} is out of range"))
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct StorageArgs {
    /// The storage ID, e.g. 0x00020001
    #[arg(long, value_parser = number::<u32>)]
    storage: Option<u32>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct HandlesArgs {
    /// The storage ID, e.g. 0x00020001
    #[arg(long, value_parser = number::<u32>)]
    storage: Option<u32>,

    /// A folder's handle; 0xFFFFFFFF for the root, 0 for every object
    #[arg(long, value_parser = number::<u32>)]
    parent: Option<u32>,

    /// List with Garmin's GetObjectIds (0x9002), which shows hidden children too
    #[arg(long)]
    hidden: bool,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct HandleArgs {
    /// The object's handle, e.g. 0x01000040
    #[arg(long, value_parser = number::<u32>)]
    handle: Option<u32>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct MtpReadArgs {
    /// The object's handle, e.g. 0x01000040
    #[arg(long, value_parser = number::<u32>)]
    handle: Option<u32>,

    /// The first byte to read [default: 0]
    #[arg(long, value_parser = number::<u32>)]
    offset: Option<u32>,

    /// Bytes to read at most [default: to the end]
    #[arg(long, value_parser = number::<u32>)]
    length: Option<u32>,

    /// A new local file for the bytes instead of base64 on stdout; it must not exist
    #[arg(long, value_name = "FILE")]
    path: Option<String>,

    /// The most bytes printed as base64 [default: 1048576]
    #[arg(long)]
    max_bytes: Option<u64>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct PtpTransactionArgs {
    /// The operation code, e.g. 0x1007; only reads are sent
    #[arg(long, value_parser = number::<u16>)]
    opcode: Option<u16>,

    /// Up to five parameters, comma-separated, e.g. 0x00020001,0,0xFFFFFFFF
    #[arg(long, value_name = "N,...", value_delimiter = ',', num_args = 1.., value_parser = number::<u32>)]
    params: Vec<u32>,

    /// A new local file for the data instead of base64 on stdout; it must not exist
    #[arg(long, value_name = "FILE")]
    path: Option<String>,

    /// The most bytes printed as base64 [default: 1048576]
    #[arg(long)]
    max_bytes: Option<u64>,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct MtpCreateArgs {
    /// The new file's name in ExtData: <part>.tmp, .fnt or .bin, e.g. 006-D8448-70.tmp
    #[arg(long)]
    name: Option<String>,

    /// The local file with the contents
    #[arg(long, value_name = "FILE")]
    source: Option<String>,

    /// Write to the watch; without it nothing is written
    #[arg(long)]
    allow_write: bool,

    #[command(flatten)]
    input: InputArgs,
}

#[cfg(feature = "dev")]
#[derive(Args)]
struct MtpDeleteArgs {
    /// The object's handle, e.g. 0x01000102
    #[arg(long, value_parser = number::<u32>)]
    handle: Option<u32>,

    /// Delete from the watch, for good; without it nothing is deleted
    #[arg(long)]
    allow_write: bool,

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

/// `flags` with `name` set to `value` when it is given.
fn flag(flags: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        flags.insert(name.into(), value);
    }
}

fn request(command: &Command) -> Result<Value, Error> {
    let mut flags = Map::new();
    let input = match command {
        Command::Inspect(args) => &args.input,
        Command::IssueLink(input) => input,
        Command::InspectDrive(args) => {
            flag(&mut flags, "path", args.path.as_ref().map(|p| json!(p)));
            &args.input
        }
        Command::Install(args) | Command::RestoreStock(args) => {
            let parts = (!args.parts.is_empty()).then(|| json!(args.parts));
            flag(&mut flags, "parts", parts);
            &args.input
        }
        Command::ExtdataList(args) => {
            flag(
                &mut flags,
                "hash",
                (!args.hash.is_empty()).then(|| json!(args.hash)),
            );
            flag(&mut flags, "offset", args.offset.map(|n| json!(n)));
            flag(&mut flags, "limit", args.limit.map(|n| json!(n)));
            let output_file = args.output_file.as_ref().map(|p| json!(p));
            flag(&mut flags, "output_file", output_file);
            &args.input
        }
        Command::WaitForWatch(args) => {
            flag(
                &mut flags,
                "timeout_seconds",
                args.timeout_seconds.map(|n| json!(n)),
            );
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::ReadFile(args) => {
            flag(&mut flags, "name", args.name.as_ref().map(|n| json!(n)));
            flag(&mut flags, "path", args.path.as_ref().map(|p| json!(p)));
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::StageFile(args) => {
            flag(&mut flags, "source", args.source.as_ref().map(|s| json!(s)));
            flag(&mut flags, "name", args.name.as_ref().map(|n| json!(n)));
            flag(
                &mut flags,
                "dry_run",
                args.dry_run.then_some(Value::Bool(true)),
            );
            let garmin = (!args.garmin_sha256.is_empty()).then(|| json!(args.garmin_sha256));
            flag(&mut flags, "garmin_sha256", garmin);
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::RemoveStaged(args) => {
            flag(&mut flags, "id", args.id.as_ref().map(|i| json!(i)));
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::MtpDeviceInfo(input) | Command::MtpStorageIds(input) => input,
        #[cfg(feature = "dev")]
        Command::MtpStorageInfo(args) => {
            flag(&mut flags, "storage", args.storage.map(|n| json!(n)));
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::MtpHandles(args) => {
            flag(&mut flags, "storage", args.storage.map(|n| json!(n)));
            flag(&mut flags, "parent", args.parent.map(|n| json!(n)));
            flag(
                &mut flags,
                "hidden",
                args.hidden.then_some(Value::Bool(true)),
            );
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::MtpObjectInfo(args) => {
            flag(&mut flags, "handle", args.handle.map(|n| json!(n)));
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::MtpRead(args) => {
            flag(&mut flags, "handle", args.handle.map(|n| json!(n)));
            flag(&mut flags, "offset", args.offset.map(|n| json!(n)));
            flag(&mut flags, "length", args.length.map(|n| json!(n)));
            flag(&mut flags, "path", args.path.as_ref().map(|p| json!(p)));
            flag(&mut flags, "max_bytes", args.max_bytes.map(|n| json!(n)));
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::PtpTransaction(args) => {
            flag(&mut flags, "opcode", args.opcode.map(|n| json!(n)));
            let params = (!args.params.is_empty()).then(|| json!(args.params));
            flag(&mut flags, "params", params);
            flag(&mut flags, "path", args.path.as_ref().map(|p| json!(p)));
            flag(&mut flags, "max_bytes", args.max_bytes.map(|n| json!(n)));
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::MtpCreate(args) => {
            flag(&mut flags, "name", args.name.as_ref().map(|n| json!(n)));
            flag(&mut flags, "source", args.source.as_ref().map(|s| json!(s)));
            let allow = args.allow_write.then_some(Value::Bool(true));
            flag(&mut flags, "allow_write", allow);
            &args.input
        }
        #[cfg(feature = "dev")]
        Command::MtpDelete(args) => {
            flag(&mut flags, "handle", args.handle.map(|n| json!(n)));
            let allow = args.allow_write.then_some(Value::Bool(true));
            flag(&mut flags, "allow_write", allow);
            &args.input
        }
        Command::Rpc | Command::Mcp => unreachable!("sessions are served before operations"),
    };
    build_input(input, flags)
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
    // The raw MTP operations are for scripts and people reading MTP: their JSON is the output.
    if operation.starts_with("mtp_") || operation == "ptp_transaction" {
        return print_json(out);
    }
    match operation {
        "issue_link" => {
            println!("{}", text(&out["url"]));
            if out["report_in_url"] != true {
                eprintln!(
                    "The report is too long for the link; paste report_text (see --json) into \
                     the form's empty Watch report box."
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
            println!("{}", support_words(&out["support"]));
            if out["support"]["report_wanted"] == true {
                println!(
                    "Nothing can be installed while the watch is a USB drive; it needs MTP. To \
                     share its font list: georgian-for-garmin inspect-drive --report"
                );
            } else {
                println!("Nothing can be installed while the watch is a USB drive; it needs MTP.");
            }
            return;
        }
        "extdata_list" => return print_listing(out),
        "wait_for_watch" => {
            println!(
                "{} can be opened (waited {:.1} s).",
                text(&out["model"]),
                out["waited_seconds"].as_f64().unwrap_or_default()
            );
            return;
        }
        "read_file" => {
            println!(
                "GARMIN/ExtData/{} ({} bytes, SHA-256 {}) copied to {}",
                text(&out["name"]),
                out["bytes"],
                text(&out["sha256"]),
                text(&out["path"])
            );
            return;
        }
        "stage_file" => {
            let what = match text(&out["status"]) {
                "staged_verified" => "written and verified",
                "already_staged" => "already there",
                _ => match text(&out["action"]) {
                    "create" => "would be written (dry run)",
                    "replace" => "would replace the file there (dry run)",
                    _ => "already there (dry run)",
                },
            };
            println!(
                "{}: {what} ({} bytes, SHA-256 {})",
                text(&out["path"]),
                out["bytes"],
                text(&out["sha256"])
            );
            if let Some(replaced) = out["replaces"].as_object() {
                println!(
                    "Replaces {} (SHA-256 {}, {}).",
                    text(&replaced["name"]),
                    text(&replaced["sha256"]),
                    text(&replaced["identified_as"])
                );
            }
            if let Some(id) = out["id"].as_str() {
                println!("Id: {id} (georgian-for-garmin remove-staged --id {id})");
            }
            return;
        }
        "remove_staged" => {
            let what = match text(&out["status"]) {
                "removed" => "deleted",
                "already_removed" => "was deleted before",
                _ => "is not on the watch",
            };
            println!("{} ({}) {what}.", text(&out["path"]), text(&out["id"]));
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
    println!("{}", support_words(&out["support"]));
    println!(
        "Georgian on the watch: {}",
        state_words(text(&out["state"]))
    );
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
            "  {:>8}  {}  {:<36}  {:<10}  file: {}{}",
            size(font),
            text(&font["part"]),
            text(&font["pack"]),
            if font["target"] == true {
                "installs"
            } else if font["restore"] == true {
                "restores"
            } else if font["listed"] == true {
                "listed"
            } else {
                "not listed"
            },
            text(&font["file"]).replace('_', " "),
            if font["imported"] == true {
                " (imported by the watch)"
            } else {
                ""
            }
        );
    }
    if out["supported"] != true {
        println!("Nothing in this build can be installed on this watch.");
        if out["restorable"] == true {
            println!("restore-stock still puts Garmin's original fonts back.");
        }
    }
    if out["support"]["report_wanted"] == true {
        println!(
            "To help add this watch, share its font list: georgian-for-garmin inspect --report"
        );
    }
}

/// `extdata_list` as a table.
fn print_listing(out: &Value) {
    let ext = &out["extdata"];
    println!(
        "ExtData folder: {} on storage {} (found by {}); {} file(s); hidden listing: {}",
        text(&ext["handle"]),
        text(&ext["storage_id"]),
        text(&ext["found_by"]).replace('_', " "),
        out["total"],
        text(&out["hidden_listing"]),
    );
    if let Some(note) = out["note"].as_str() {
        println!("Note: {note}");
    }
    if let Some(path) = out["output_file"].as_str() {
        println!("Wrote the list to {path}.");
    }
    for file in items(&out["files"]) {
        let flags = [
            (file["hidden"] == true).then_some("hidden"),
            (file["read_only"] == true).then_some("read-only"),
        ];
        let flags: Vec<&str> = flags.into_iter().flatten().collect();
        println!(
            "  {:<24} {:>10}  {:<18} {}",
            text(&file["name"]),
            file["bytes"],
            flags.join(", "),
            file["sha256"].as_str().unwrap_or_default()
        );
    }
    if let Some(next) = out["next_offset"].as_u64() {
        println!("More: --offset {next}");
    }
    for name in items(&out["hash_not_found"]) {
        println!("No file named {}", text(name));
    }
}

/// `inspect`'s `support` in a sentence, with the issue to follow.
fn support_words(support: &Value) -> String {
    let mut line = match text(&support["status"]) {
        "tested" => "Status: tested; Georgian was confirmed on this model and software.".to_owned(),
        "expected" => "Status: expected; Georgian installs but isn't confirmed on this model and \
                       software yet."
            .to_owned(),
        "unsupported" => format!("Status: unsupported; {}", text(&support["reason"]["en"])),
        _ => "Status: unknown; this build has no font for this watch.".to_owned(),
    };
    if let Some(issue) = support["issue"].as_u64() {
        line.push_str(&format!(" Updates: {}/{issue}", report::ISSUES));
    }
    line
}

/// `inspect`'s `state` in a sentence.
fn state_words(state: &str) -> &'static str {
    match state {
        "unsupported" => "nothing to install on this watch.",
        "not_installed" => "not installed.",
        "waiting" => "waiting on the watch; unplug it and restart it to finish.",
        "installed" => "installed.",
        "partly" => "in some fonts; install adds the rest.",
        "beta" => "an earlier test version; install replaces it.",
        "stock_waiting" => "Garmin's original fonts are waiting; unplug and restart to finish.",
        "not_visible" => {
            "can't be seen: the watch doesn't show its font files over USB. Installing again is \
             harmless."
        }
        "other" => "a font file this build doesn't know is on the watch; nothing can be changed.",
        _ => "unknown: the font folder wasn't found.",
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
    let named = args
        .iter()
        .skip(1)
        .filter_map(|a| a.to_str())
        .filter(|a| !a.starts_with('-'))
        .find_map(|a| ops::operation(&a.replace('-', "_")));
    match named {
        Some(op) => json!({"tools": [op.tool()]}),
        None => ops::tools(),
    }
}

/// A session that keeps the watch open between calls, on its own thread.
fn worker() -> Worker {
    Worker::spawn(|| Session::new(Cow::Owned(FontSet::bundled()), Box::new(connect)))
}

fn serve_mcp() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("error: cannot start the MCP server: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(mcp::serve(worker(), rmcp::transport::stdio())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
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
    match cli.command {
        Command::Rpc => {
            rpc::serve(io::stdin().lock(), io::stdout(), worker());
            return ExitCode::SUCCESS;
        }
        Command::Mcp => return serve_mcp(),
        _ => {}
    }
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

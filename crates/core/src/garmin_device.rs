//! `GARMIN/GarminDevice.xml`: the watch's model and the update files it accepts.

use roxmltree::{Document, Node};

use crate::error::{Error, ErrorCode};

/// Where a watch accepts standalone font packs, as declared in its manifest.
pub const EXTDATA_PATH: &str = "Garmin/ExtData";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateFile {
    pub part_number: String,
    pub path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarminDevice {
    pub description: String,
    pub part_number: String,
    /// Garmin's version number, e.g. `1905`, formatted as `19.05`.
    pub software_version: String,
    pub update_files: Vec<UpdateFile>,
}

impl GarminDevice {
    /// Whether the watch declares `part` as an update file under `Garmin/ExtData`.
    pub fn declares_extdata_part(&self, part: &str) -> bool {
        self.update_files
            .iter()
            .any(|f| f.part_number == part && f.path.eq_ignore_ascii_case(EXTDATA_PATH))
    }
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn text(node: Node<'_, '_>, name: &str) -> String {
    child(node, name)
        .and_then(|c| c.text())
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn format_version(raw: &str) -> String {
    match raw.parse::<u32>() {
        Ok(n) if raw.len() >= 3 => format!("{}.{:02}", n / 100, n % 100),
        _ => raw.to_owned(),
    }
}

pub fn parse(bytes: &[u8]) -> Result<GarminDevice, Error> {
    let invalid = |m: String| {
        Error::new(
            ErrorCode::GarminFolderNotFound,
            format!("GarminDevice.xml: {m}"),
        )
    };
    let xml = std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?;
    let doc =
        Document::parse(xml.trim_start_matches('\u{feff}')).map_err(|e| invalid(e.to_string()))?;
    let root = doc.root_element();
    let model = child(root, "Model").ok_or_else(|| invalid("no Model element".into()))?;
    let update_files = child(root, "MassStorageMode")
        .into_iter()
        .flat_map(|m| {
            m.children()
                .filter(|c| c.is_element() && c.tag_name().name() == "UpdateFile")
        })
        .map(|f| UpdateFile {
            part_number: text(f, "PartNumber"),
            path: text(f, "Path"),
            file_name: text(f, "FileName"),
        })
        .collect();
    Ok(GarminDevice {
        description: text(model, "Description"),
        part_number: text(model, "PartNumber"),
        software_version: format_version(&text(model, "SoftwareVersion")),
        update_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_model_and_extdata_parts() {
        let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Device xmlns=\"http://www.garmin.com/xmlschemas/GarminDevice/v2\">
  <Model><PartNumber>006-B3703-00</PartNumber><SoftwareVersion>1905</SoftwareVersion><Description>Venu 2</Description></Model>
  <Id>1</Id>
  <MassStorageMode>
    <DataType><Name>FIT</Name></DataType>
    <UpdateFile><PartNumber>006-D8422-03</PartNumber><Path>Garmin/Text</Path><FileName>french.ln4</FileName></UpdateFile>
    <UpdateFile><PartNumber>006-D9035-03</PartNumber><Version><Major>2</Major><Minor>10</Minor></Version><Path>Garmin/ExtData</Path><FileName>006-D9035-03.tmp</FileName></UpdateFile>
  </MassStorageMode>
</Device>";
        let d = parse(xml.as_bytes()).unwrap();
        assert_eq!(
            (d.description.as_str(), d.part_number.as_str()),
            ("Venu 2", "006-B3703-00")
        );
        assert_eq!(d.software_version, "19.05");
        assert!(d.declares_extdata_part("006-D9035-03"));
        assert!(!d.declares_extdata_part("006-D8422-03"));
        assert!(!d.declares_extdata_part("006-D9035-01"));
        assert!(parse(b"<Device/>").is_err());
    }
}

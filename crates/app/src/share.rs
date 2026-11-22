//! What the check screen shows besides the questions: the phone page's QR code, drawn here
//! without the network, and the beta test marks' pictures. The issue link itself is the core's
//! `issue_link` operation.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use georgian_for_garmin_core::fonts::{FontSet, Kind};
use georgian_for_garmin_core::report::PHONE_PAGE;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Serialize;

/// The phone test page, in the app's language.
#[derive(Debug, Serialize)]
pub struct PhonePage {
    pub url: String,
    /// The URL's QR code as an SVG `data:` URL.
    pub qr: String,
}

/// The phone page in `language` ("ka" is the page's default) and its QR code.
pub fn phone_page(language: &str) -> PhonePage {
    let url = if language == "ka" {
        PHONE_PAGE.to_owned()
    } else {
        format!("{PHONE_PAGE}?lang={language}")
    };
    let code = QrCode::new(url.as_bytes()).expect("a short URL fits in a QR code");
    let image = code
        .render::<svg::Color<'_>>()
        .min_dimensions(160, 160)
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    PhonePage {
        url,
        qr: format!("data:image/svg+xml;base64,{}", STANDARD.encode(image)),
    }
}

/// A beta build's test mark as the marker question shows it.
#[derive(Debug, Serialize)]
pub struct MarkPicture {
    pub part: String,
    pub kind: Kind,
    pub variant: String,
    pub style: String,
    /// The test word in this part's Georgian build as a PNG `data:` URL.
    pub picture: String,
}

/// Every part's test mark, in registry order; empty in stable builds.
pub fn mark_pictures(fonts: &FontSet) -> Vec<MarkPicture> {
    fonts
        .fonts()
        .filter_map(|(_, font)| {
            let mark = font.mark.as_ref()?;
            Some(MarkPicture {
                part: font.part.clone(),
                kind: font.kind,
                variant: mark.variant.clone(),
                style: mark.style.clone(),
                picture: format!("data:image/png;base64,{}", STANDARD.encode(&mark.picture)),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phone_page_follows_the_language() {
        assert_eq!(phone_page("ka").url, PHONE_PAGE);
        assert_eq!(phone_page("en").url, format!("{PHONE_PAGE}?lang=en"));
        assert!(phone_page("en").qr.starts_with("data:image/svg+xml;base64,"));
    }
}

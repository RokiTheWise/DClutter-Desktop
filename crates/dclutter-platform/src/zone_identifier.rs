//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Parses the `Zone.Identifier` alternate data stream that browsers and
//! Windows attach to downloaded files ("Mark of the Web"):
//!
//! ```text
//! [ZoneTransfer]
//! ZoneId=3
//! ReferrerUrl=https://example.com/page
//! HostUrl=https://example.com/file.pdf
//! ```

/// What a `Zone.Identifier` stream says about where a file came from.
#[derive(Debug, Default, PartialEq)]
pub struct ZoneTransfer {
    pub zone_id: Option<u32>,
    pub host_url: Option<String>,
    pub referrer_url: Option<String>,
}

/// URLs Zone.Identifier uses as placeholders rather than real addresses.
const PLACEHOLDER_URLS: [&str; 2] = ["about:internet", "about:blank"];

impl ZoneTransfer {
    pub fn parse(text: &str) -> Self {
        let mut result = Self::default();
        let mut in_section = false;

        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_section = line.eq_ignore_ascii_case("[ZoneTransfer]");
                continue;
            }
            if !in_section {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "zoneid" => result.zone_id = value.parse().ok(),
                "hosturl" => result.host_url = real_url(value),
                "referrerurl" => result.referrer_url = real_url(value),
                _ => {}
            }
        }
        result
    }

    /// Zones 3 (Internet) and 4 (Restricted) mean the file came from
    /// outside this machine and its local network.
    pub fn is_from_internet(&self) -> bool {
        matches!(self.zone_id, Some(3 | 4))
    }
}

fn real_url(value: &str) -> Option<String> {
    let is_placeholder = PLACEHOLDER_URLS.iter().any(|p| value.eq_ignore_ascii_case(p));
    (!value.is_empty() && !is_placeholder).then(|| value.to_owned())
}

/// Decodes the stream's bytes. Most writers use ASCII or UTF-8, but some
/// tools write UTF-16 with a byte-order mark.
pub fn decode(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let units: Vec<u16> = rest
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_typical_browser_download() {
        let zone = ZoneTransfer::parse(
            "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://example.com/page\r\nHostUrl=https://example.com/file.pdf\r\n",
        );
        assert_eq!(
            zone,
            ZoneTransfer {
                zone_id: Some(3),
                host_url: Some("https://example.com/file.pdf".into()),
                referrer_url: Some("https://example.com/page".into()),
            }
        );
        assert!(zone.is_from_internet());
    }

    #[test]
    fn treats_about_internet_as_no_url() {
        let zone = ZoneTransfer::parse("[ZoneTransfer]\nZoneId=3\nHostUrl=about:internet\n");
        assert_eq!(zone.host_url, None);
        assert!(zone.is_from_internet());
    }

    #[test]
    fn ignores_keys_outside_the_zone_transfer_section() {
        let zone = ZoneTransfer::parse("[Other]\nHostUrl=https://wrong.example\n[ZoneTransfer]\nZoneId=2\n");
        assert_eq!(zone.host_url, None);
        assert_eq!(zone.zone_id, Some(2));
        assert!(!zone.is_from_internet());
    }

    #[test]
    fn keys_are_case_insensitive() {
        let zone = ZoneTransfer::parse("[zonetransfer]\nzoneid=3\nhosturl=https://example.com/a\n");
        assert_eq!(zone.host_url.as_deref(), Some("https://example.com/a"));
    }

    #[test]
    fn decodes_utf16_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("[ZoneTransfer]\nZoneId=3\n".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode(&bytes), "[ZoneTransfer]\nZoneId=3\n");
    }

    #[test]
    fn decodes_utf8_with_bom() {
        assert_eq!(decode(b"\xEF\xBB\xBFZoneId=3"), "ZoneId=3");
    }
}

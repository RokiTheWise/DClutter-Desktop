//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! The seam between DClutter and the host operating system. A port of
//! `DirectoryMetadataProvider` and, later, `FileActions` from the macOS app.

mod scan;
#[cfg_attr(not(windows), allow(dead_code))]
mod zone_identifier;

pub use scan::{UNKNOWN_INTERNET_SOURCE, candidates_in};

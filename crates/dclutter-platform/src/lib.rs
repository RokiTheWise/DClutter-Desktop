//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! The seam between DClutter and the host operating system. A port of
//! `DirectoryMetadataProvider` and `FileActions` from the macOS app.

mod actions;
mod destinations;
mod locks;
mod moves;
mod recents;
mod renames;
mod scan;
mod sessions;
mod thumbnails;
#[cfg_attr(not(windows), allow(dead_code))]
mod zone_identifier;

pub use actions::{TrashError, move_to_recycle_bin};
pub use destinations::{Destination, DestinationStore, MAX_DESTINATIONS};
pub use moves::{MoveError, move_back, move_into_folder};
pub use recents::{MAX_RECENT_FOLDERS, RecentFolders};
pub use renames::{RenameError, rename_in_place};
pub use scan::{UNKNOWN_INTERNET_SOURCE, candidates_in};
pub use sessions::SessionStore;
pub use thumbnails::{Thumbnail, ThumbnailKind, thumbnail};

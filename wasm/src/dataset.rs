//! Dataset management for the data-fabrication challenge.
//!
//! Provides functions for accessing active dataset information and metadata.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::storage;

/// Metadata about the current dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    /// Unique identifier for the dataset
    pub id: String,
    /// Name of the dataset
    pub name: String,
    /// Number of conversations in the dataset
    pub conversation_count: u32,
    /// Total messages across all conversations
    pub total_messages: u32,
    /// Size in bytes
    pub size_bytes: u64,
    /// Creation timestamp (Unix epoch)
    pub created_at: u64,
    /// Whether dataset is active for submissions
    pub is_active: bool,
}

/// Get the active dataset raw bytes.
///
/// Returns serialized dataset info for the current active dataset.
/// Returns an empty vector if no dataset is active.
pub fn get_active_dataset() -> Vec<u8> {
    match storage::get_eval_config() {
        Some(data) => data,
        None => Vec::new(),
    }
}

/// Get the dataset info metadata.
///
/// Returns the active dataset info if available, or a default inactive dataset.
pub fn get_dataset_info() -> DatasetInfo {
    // Try to get stored dataset info from storage
    let key = b"dataset:info";
    if let Some(data) = storage::get_raw(key) {
        if let Ok(info) = bincode::deserialize::<DatasetInfo>(&data) {
            return info;
        }
    }

    // Return a default inactive dataset info
    DatasetInfo {
        id: String::new(),
        name: String::new(),
        conversation_count: 0,
        total_messages: 0,
        size_bytes: 0,
        created_at: 0,
        is_active: false,
    }
}

/// Store dataset info in storage.
pub fn set_dataset_info(info: &DatasetInfo) -> bool {
    match bincode::serialize(info) {
        Ok(data) => storage::set_raw(b"dataset:info", &data),
        Err(_) => false,
    }
}

/// Check if there is an active dataset available.
pub fn has_active_dataset() -> bool {
    get_dataset_info().is_active
}

/// Get the dataset size in bytes.
pub fn get_dataset_size() -> u64 {
    get_dataset_info().size_bytes
}

/// Get the conversation count for the active dataset.
pub fn get_conversation_count() -> u32 {
    get_dataset_info().conversation_count
}

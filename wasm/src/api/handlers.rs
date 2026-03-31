//! Route handlers for the data-fabrication challenge.
//!
//! All handlers return WasmRouteResponse and use storage module for persistence.
//! No panics - all errors are handled gracefully.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;
use platform_challenge_sdk_wasm::host_functions::host_consensus_get_epoch;
use platform_challenge_sdk_wasm::{WasmRouteRequest, WasmRouteResponse};
use serde::{Deserialize, Serialize};

use crate::scoring;
use crate::storage::{self, agents, eval, leaderboard};
use crate::types::{ChallengeParams, UploadState};

// ════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ════════════════════════════════════════════════════════════════════════════

/// Create a JSON response from a serializable value.
fn json_response<T: Serialize>(value: &T) -> WasmRouteResponse {
    let body = serde_json::to_vec(value).unwrap_or_default();
    WasmRouteResponse { status: 200, body }
}

/// Create a JSON error response.
fn json_error(status: u16, msg: &str) -> WasmRouteResponse {
    let body = serde_json::to_vec(&serde_json::json!({"error": msg})).unwrap_or_default();
    WasmRouteResponse { status, body }
}

/// Create unauthorized response (401).
fn unauthorized_response() -> WasmRouteResponse {
    json_error(401, "unauthorized")
}

/// Create bad request response (400).
fn bad_request_response() -> WasmRouteResponse {
    json_error(400, "bad request")
}

/// Create not found response (404).
fn not_found_response() -> WasmRouteResponse {
    json_error(404, "not found")
}

/// Check if request is authenticated.
fn is_authenticated(request: &WasmRouteRequest) -> bool {
    request
        .auth_hotkey
        .as_ref()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
}

/// Get a path parameter by name.
fn get_param<'a>(request: &'a WasmRouteRequest, name: &str) -> Option<&'a str> {
    request
        .params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Parse JSON body into a struct.
fn parse_json_body<T: serde::de::DeserializeOwned>(request: &WasmRouteRequest) -> Option<T> {
    if request.body.is_empty() {
        return None;
    }
    serde_json::from_slice(&request.body).ok()
}

/// Get current epoch from host, defaulting to 0.
fn get_current_epoch() -> u64 {
    let epoch = host_consensus_get_epoch();
    if epoch >= 0 {
        epoch as u64
    } else {
        0
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Sudo Helpers
// ════════════════════════════════════════════════════════════════════════════

/// Global state for admin configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalState {
    pub evaluation_enabled: bool,
    pub upload_state: UploadState,
    pub sudo_owner: String,
}

impl Default for GlobalState {
    fn default() -> Self {
        Self {
            evaluation_enabled: true,
            upload_state: UploadState::Enabled,
            sudo_owner: String::new(),
        }
    }
}

fn get_global_state() -> GlobalState {
    storage::get_bincode(&storage::global_key("global_state")).unwrap_or_default()
}

fn set_global_state(state: &GlobalState) -> bool {
    storage::set_bincode(&storage::global_key("global_state"), state)
}

/// Check if hotkey is sudo owner.
fn is_sudo_owner(hotkey: &str) -> bool {
    let state = get_global_state();
    // If no owner set, allow any authenticated request (first-run behavior)
    if state.sudo_owner.is_empty() {
        return true;
    }
    state.sudo_owner == hotkey
}

/// Require sudo owner authentication.
fn require_sudo(request: &WasmRouteRequest) -> Result<(), WasmRouteResponse> {
    if !is_authenticated(request) {
        return Err(unauthorized_response());
    }
    let hotkey = request.auth_hotkey.as_deref().unwrap_or("");
    if !is_sudo_owner(hotkey) {
        return Err(json_error(403, "forbidden: sudo owner only"));
    }
    Ok(())
}

/// Check if upload is enabled.
fn is_upload_enabled() -> bool {

/// Check if upload is in pending mode (accept but queue).
pub fn is_upload_pending() -> bool {
    get_global_state().upload_state == UploadState::Pending
}
    get_global_state().upload_state == UploadState::Enabled
}

/// Check if evaluation is enabled.
fn is_evaluation_enabled() -> bool {
    get_global_state().evaluation_enabled
}

// ════════════════════════════════════════════════════════════════════════════
// Public Read Handlers
// ════════════════════════════════════════════════════════════════════════════

/// GET /leaderboard - Return current leaderboard with scores and miner hotkeys.
pub fn handle_leaderboard(_request: &WasmRouteRequest) -> WasmRouteResponse {
    leaderboard::rebuild();
    let entries = leaderboard::get_leaderboard();

    // Build rich response with additional metadata
    let response: Vec<serde_json::Value> = entries
        .iter()
        .filter(|e| !storage::is_banned(&e.hotkey))
        .map(|entry| {
            serde_json::json!({
                "rank": entry.rank,
                "agent_hash": entry.agent_hash,
                "hotkey": entry.hotkey,
                "score": entry.score,
                "epoch": entry.epoch,
            })
        })
        .collect();

    json_response(&response)
}

/// GET /status - Return current challenge status and health.
pub fn handle_status(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let epoch = get_current_epoch();
    let state = get_global_state();
    leaderboard::rebuild();
    let leaderboard_count = leaderboard::count();

    let response = serde_json::json!({
        "epoch": epoch,
        "evaluation_enabled": state.evaluation_enabled,
        "upload_state": match state.upload_state { UploadState::Disabled => "disabled", UploadState::Pending => "pending", UploadState::Enabled => "enabled" },
        "upload_enabled": matches!(state.upload_state, UploadState::Enabled),
        "total_submissions": leaderboard_count,
        "active_miners": storage::get_active_miner_count(),
        "validator_count": storage::get_validator_count(),
    });

    json_response(&response)
}

/// GET /health - Health check endpoint.
pub fn handle_health(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let response = serde_json::json!({
        "status": "healthy",
        "challenge": "data-fabrication",
        "version": "0.1.0",
    });
    json_response(&response)
}

/// GET /stats - Challenge statistics.
pub fn handle_stats(_request: &WasmRouteRequest) -> WasmRouteResponse {
    leaderboard::rebuild();
    let entries = leaderboard::get_leaderboard();
    let total_submissions = entries.len() as u64;
    let active_miners = entries.iter().filter(|e| e.score > 0.0).count() as u64;

    let response = serde_json::json!({
        "total_submissions": total_submissions,
        "active_miners": active_miners,
        "validator_count": storage::get_validator_count(),
        "total_agents": total_submissions,
        "active_agents": active_miners,
    });

    json_response(&response)
}

/// GET /dataset - Return active dataset of evaluation tasks.
pub fn handle_dataset(_request: &WasmRouteRequest) -> WasmRouteResponse {
    // For data-fabrication, dataset is the challenge params
    let params: ChallengeParams =
        storage::get_bincode(&storage::global_key("challenge_params")).unwrap_or_default();

    let response = serde_json::json!({
        "min_conversations": params.min_conversations,
        "max_conversations": params.max_conversations,
        "max_size_bytes": params.max_size_bytes,
        "model": params.model,
    });

    json_response(&response)
}

/// GET /dataset/history - Return historical dataset selections.
pub fn handle_dataset_history(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let history: Vec<serde_json::Value> = alloc::vec::Vec::new();
    json_response(&history)
}

/// GET /dataset/consensus - Check dataset consensus status.
pub fn handle_dataset_consensus(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let response = serde_json::json!({
        "consensus_reached": true,
        "validator_count": storage::get_validator_count(),
    });
    json_response(&response)
}

// ════════════════════════════════════════════════════════════════════════════
// Submission Handlers
// ════════════════════════════════════════════════════════════════════════════

/// GET /submissions - Return pending submissions.
pub fn handle_submissions(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let epoch = get_current_epoch();
    let all_scores = eval::get_all_scores();

    // Build submission list from stored data
    let submissions: Vec<serde_json::Value> = all_scores
        .iter()
        .map(|(agent_hash, hotkey, score)| {
            let status =
                agents::get_status(hotkey, epoch).unwrap_or_else(|| String::from("pending"));
            let last_epoch = eval::get_last_epoch(hotkey).unwrap_or(epoch);
            serde_json::json!({
                "agent_hash": agent_hash,
                "hotkey": hotkey,
                "score": score,
                "epoch": last_epoch,
                "status": status,
            })
        })
        .collect();

    json_response(&submissions)
}

/// GET /submissions/:id - Return specific submission status by ID.
pub fn handle_submission_by_id(request: &WasmRouteRequest) -> WasmRouteResponse {
    let id = match get_param(request, "id") {
        Some(id) => id,
        None => return bad_request_response(),
    };

    // Try to find by agent_hash (id could be agent_hash or hotkey)
    let all_scores = eval::get_all_scores();
    let found = all_scores
        .iter()
        .find(|(agent_hash, hotkey, _)| agent_hash == id || hotkey == id);

    match found {
        Some((agent_hash, hotkey, score)) => {
            let epoch = eval::get_last_epoch(hotkey).unwrap_or(0);
            let status =
                agents::get_status(hotkey, epoch).unwrap_or_else(|| String::from("scored"));

            let response = serde_json::json!({
                "agent_hash": agent_hash,
                "hotkey": hotkey,
                "score": score,
                "epoch": epoch,
                "status": status,
            });
            json_response(&response)
        }
        None => not_found_response(),
    }
}

/// GET /results/:id - Return evaluation results by ID.
pub fn handle_results(request: &WasmRouteRequest) -> WasmRouteResponse {
    let id = match get_param(request, "id") {
        Some(id) => id,
        None => return bad_request_response(),
    };

    let all_scores = eval::get_all_scores();
    let found = all_scores
        .iter()
        .find(|(agent_hash, hotkey, _)| agent_hash == id || hotkey == id);

    match found {
        Some((agent_hash, hotkey, score)) => {
            let epoch = eval::get_last_epoch(hotkey).unwrap_or(0);
            let status =
                agents::get_status(hotkey, epoch).unwrap_or_else(|| String::from("scored"));

            let response = serde_json::json!({
                "agent_hash": agent_hash,
                "hotkey": hotkey,
                "score": score,
                "epoch": epoch,
                "status": status,
            });
            json_response(&response)
        }
        None => not_found_response(),
    }
}

/// GET /agent/:hotkey - Return agent info by hotkey.
pub fn handle_agent_by_hotkey(request: &WasmRouteRequest) -> WasmRouteResponse {
    let hotkey = match get_param(request, "hotkey") {
        Some(h) => h,
        None => return bad_request_response(),
    };

    let epoch = get_current_epoch();
    let score = eval::get_score(hotkey);
    let status = agents::get_status(hotkey, epoch);
    let code_hash = agents::get_hash(hotkey, epoch);
    let last_epoch = eval::get_last_epoch(hotkey).unwrap_or(epoch);

    let response = serde_json::json!({
        "hotkey": hotkey,
        "epoch": last_epoch,
        "score": score,
        "status": status,
        "code_hash": code_hash,
    });

    json_response(&response)
}

/// GET /agent/:hotkey/logs - Return evaluation logs for a miner.
pub fn handle_logs(request: &WasmRouteRequest) -> WasmRouteResponse {
    let hotkey = match get_param(request, "hotkey") {
        Some(h) => h,
        None => return bad_request_response(),
    };

    let epoch = get_current_epoch();
    // Return stored logs if available
    let logs_key = storage::key2("logs", hotkey);
    let logs: Vec<String> = storage::get_bincode(&logs_key).unwrap_or_default();

    json_response(&serde_json::json!({
        "hotkey": hotkey,
        "epoch": epoch,
        "logs": logs,
    }))
}

/// GET /agent/:hotkey/code - Return stored agent code package for a miner.
pub fn handle_code(request: &WasmRouteRequest) -> WasmRouteResponse {
    let hotkey = match get_param(request, "hotkey") {
        Some(h) => h,
        None => return bad_request_response(),
    };

    let epoch = get_current_epoch();
    let code = agents::get_code(hotkey, epoch)
        .or_else(|| agents::get_code(hotkey, epoch.saturating_sub(1)));

    match code {
        Some(data) => WasmRouteResponse {
            status: 200,
            body: data,
        },
        None => json_error(404, "code not found"),
    }
}

/// GET /get_weights - Return current weight assignments for all miners.
pub fn handle_get_weights(_request: &WasmRouteRequest) -> WasmRouteResponse {
    leaderboard::rebuild();
    let entries = leaderboard::get_leaderboard();

    // Filter out banned miners and zero scores
    let weights: Vec<serde_json::Value> = entries
        .iter()
        .filter(|e| !storage::is_banned(&e.hotkey) && e.score > 0.0)
        .map(|e| {
            let weight = scoring::to_weight(e.score, 1.0);
            serde_json::json!({
                "hotkey": e.hotkey,
                "weight": weight,
            })
        })
        .collect();

    // Normalize weights to sum to 1.0
    let total: f64 = weights
        .iter()
        .filter_map(|w| w.get("weight").and_then(|v| v.as_f64()))
        .sum();

    if total <= 0.0 {
        return json_response(&Vec::<serde_json::Value>::new());
    }

    let normalized: Vec<serde_json::Value> = weights
        .iter()
        .map(|w| {
            let weight = w.get("weight").and_then(|v| v.as_f64()).unwrap_or(0.0);
            serde_json::json!({
                "hotkey": w.get("hotkey"),
                "weight": weight / total,
            })
        })
        .collect();

    json_response(&normalized)
}

// ════════════════════════════════════════════════════════════════════════════
// Configuration Handlers
// ════════════════════════════════════════════════════════════════════════════

/// Timout configuration for review assignments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub llm_review_timeout_blocks: u64,
    pub ast_review_timeout_blocks: u64,
    pub basilica_timeout_blocks: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            llm_review_timeout_blocks: 3600,
            ast_review_timeout_blocks: 1800,
            basilica_timeout_blocks: 7200,
        }
    }
}

/// GET /config - Return current configuration.
pub fn handle_get_config(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let state = get_global_state();
    let params: ChallengeParams =
        storage::get_bincode(&storage::global_key("challenge_params")).unwrap_or_default();

    let response = serde_json::json!({
        "evaluation_enabled": state.evaluation_enabled,
        "upload_state": match state.upload_state { UploadState::Disabled => "disabled", UploadState::Pending => "pending", UploadState::Enabled => "enabled" },
        "upload_enabled": matches!(state.upload_state, UploadState::Enabled),
        "min_conversations": params.min_conversations,
        "max_conversations": params.max_conversations,
        "max_size_bytes": params.max_size_bytes,
    });

    json_response(&response)
}

/// POST /config - Update configuration (requires auth).
pub fn handle_set_config(request: &WasmRouteRequest) -> WasmRouteResponse {
    if let Err(e) = require_sudo(request) {
        return e;
    }

    #[derive(Deserialize)]
    struct ConfigRequest {
        evaluation_enabled: Option<bool>,
        upload_enabled: Option<bool>,
        min_conversations: Option<u32>,
        max_conversations: Option<u32>,
        max_size_bytes: Option<u64>,
    }

    let body: ConfigRequest = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let mut state = get_global_state();
    if let Some(e) = body.evaluation_enabled {
        state.evaluation_enabled = e;
    }
    if let Some(u) = body.upload_enabled {
        state.upload_state = if u { UploadState::Enabled } else { UploadState::Disabled };
    }

    let ok = set_global_state(&state);

    // Update challenge params if provided
    if body.min_conversations.is_some()
        || body.max_conversations.is_some()
        || body.max_size_bytes.is_some()
    {
        let mut params: ChallengeParams =
            storage::get_bincode(&storage::global_key("challenge_params")).unwrap_or_default();
        if let Some(min) = body.min_conversations {
            params.min_conversations = min;
        }
        if let Some(max) = body.max_conversations {
            params.max_conversations = max;
        }
        if let Some(size) = body.max_size_bytes {
            params.max_size_bytes = size;
        }
        let _ = storage::set_bincode(&storage::global_key("challenge_params"), &params);
    }

    json_response(&serde_json::json!({
        "success": ok,
        "evaluation_enabled": state.evaluation_enabled,
        "upload_state": match state.upload_state { UploadState::Disabled => "disabled", UploadState::Pending => "pending", UploadState::Enabled => "enabled" },
        "upload_enabled": matches!(state.upload_state, UploadState::Enabled),
    }))
}

/// GET /config/timeout - Return current timeout configuration.
pub fn handle_get_timeout_config(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let config: TimeoutConfig =
        storage::get_bincode(&storage::global_key("timeout_config")).unwrap_or_default();
    json_response(&config)
}

/// POST /config/timeout - Update timeout configuration (requires auth).
pub fn handle_set_timeout_config(request: &WasmRouteRequest) -> WasmRouteResponse {
    if let Err(e) = require_sudo(request) {
        return e;
    }

    let config: TimeoutConfig = match parse_json_body(request) {
        Some(c) => c,
        None => return bad_request_response(),
    };

    let ok = storage::set_bincode(&storage::global_key("timeout_config"), &config);
    json_response(&serde_json::json!({
        "success": ok,
        "config": config,
    }))
}

// ════════════════════════════════════════════════════════════════════════════
// Submission & Evaluation Handlers
// ════════════════════════════════════════════════════════════════════════════

/// POST /submit - Submit fabricated data package for evaluation.
pub fn handle_submit(request: &WasmRouteRequest) -> WasmRouteResponse {
    // Check upload state
    let upload_state = get_global_state().upload_state;
    match upload_state {
        UploadState::Disabled => {
            return json_error(403, "upload is currently disabled by admin");
        }
        UploadState::Pending => {
            // Accept and queue, but do not process
            if !is_authenticated(request) {
                return unauthorized_response();
            }
            let hotkey = request.auth_hotkey.as_deref().unwrap_or("");
            let epoch = get_current_epoch();
            let _ = agents::store_status(hotkey, epoch, "queued");
            return json_response(&serde_json::json!({
                "status": "queued",
                "message": "Upload accepted. Processing is paused. Pending admin approval."
            }));
        }
        UploadState::Enabled => {}
    }
    
    // Normal flow continues for Enabled state


    if !is_authenticated(request) {
        return unauthorized_response();
    }

    #[derive(Deserialize)]
    struct SubmitRequest {
        name: Option<String>,
        #[serde(default)]
        code_hash: Option<String>,
        #[serde(default)]
        package: Option<alloc::vec::Vec<u8>>,
        epoch: Option<u64>,
    }

    let body: SubmitRequest = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let hotkey = request.auth_hotkey.as_deref().unwrap_or("");
    let epoch = body.epoch.unwrap_or_else(get_current_epoch);

    // Generate agent_hash from code_hash or hotkey
    let agent_hash = match &body.code_hash {
        Some(hash) if !hash.is_empty() => hash.clone(),
        _ => {
            // Generate hash from hotkey + epoch
            let mut hasher_input = String::new();
            let _ = write!(hasher_input, "{}:{}", hotkey, epoch);
            // Simple hash simulation (in real use, would use sha2)
            let mut result = String::new();
            for (i, b) in hasher_input.bytes().enumerate() {
                let _ = write!(result, "{:02x}", (b as usize + i) % 256);
            }
            result
        }
    };

    // Store submission data
    let _ = agents::store_hash(hotkey, epoch, &agent_hash);
    let _ = agents::store_status(hotkey, epoch, "pending");
    let _ = eval::store_record(hotkey, epoch, &agent_hash);
    let _ = eval::set_last_epoch(hotkey, epoch);

    // Store package code if provided
    if let Some(ref pkg) = body.package {
        if !pkg.is_empty() {
            let _ = agents::store_code(hotkey, epoch, pkg);
        }
    }

    json_response(&serde_json::json!({
        "success": true,
        "agent_hash": agent_hash,
        "hotkey": hotkey,
        "epoch": epoch,
        "status": "pending",
    }))
}

/// POST /evaluate - Trigger evaluation for pending submissions.
pub fn handle_evaluate(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_evaluation_enabled() {
        return json_error(503, "evaluation is currently disabled by admin");
    }

    if !is_authenticated(request) {
        return unauthorized_response();
    }

    #[derive(Deserialize)]
    struct EvaluateRequest {
        #[serde(default)]
        hotkey: Option<String>,
        #[serde(default)]
        agent_hash: Option<String>,
        #[serde(default)]
        score: Option<f64>,
    }

    let body: EvaluateRequest = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let epoch = get_current_epoch();

    // If score provided, store it directly (for testing/debugging)
    if let (Some(hotkey), Some(score)) = (&body.hotkey, &body.score) {
        let agent_hash = body
            .agent_hash
            .clone()
            .unwrap_or_else(|| eval::get_record(hotkey, epoch).unwrap_or_default());

        if !agent_hash.is_empty() {
            let _ = eval::set_score(hotkey, *score);
            let _ = eval::set_score_by_hash(&agent_hash, hotkey, *score);
            let _ = agents::store_status(hotkey, epoch, "scored");
            leaderboard::rebuild();
        }
    }

    json_response(&serde_json::json!({
        "success": true,
        "evaluation_triggered": true,
        "epoch": epoch,
    }))
}

/// POST /dataset/propose - Propose task indices for dataset consensus.
pub fn handle_dataset_propose(request: &WasmRouteRequest) -> WasmRouteResponse {
    if !is_authenticated(request) {
        return unauthorized_response();
    }

    #[derive(Deserialize)]
    struct ProposeRequest {
        validator_id: String,
        indices: alloc::vec::Vec<u32>,
    }

    let _body: ProposeRequest = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    json_response(&serde_json::json!({
        "success": true,
        "message": "dataset proposal recorded",
    }))
}

// ════════════════════════════════════════════════════════════════════════════
// Sudo/Admin Handlers
// ════════════════════════════════════════════════════════════════════════════

/// GET /sudo/state - Return current global state.
pub fn handle_sudo_get_state(_request: &WasmRouteRequest) -> WasmRouteResponse {
    let state = get_global_state();
    json_response(&state)
}

/// POST /sudo/evaluation - Enable/disable evaluation.
pub fn handle_sudo_set_evaluation(request: &WasmRouteRequest) -> WasmRouteResponse {
    if let Err(e) = require_sudo(request) {
        return e;
    }

    #[derive(Deserialize)]
    struct Req {
        enabled: bool,
    }

    let body: Req = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let mut state = get_global_state();
    state.evaluation_enabled = body.enabled;
    let ok = set_global_state(&state);

    json_response(&serde_json::json!({
        "success": ok,
        "evaluation_enabled": state.evaluation_enabled,
    }))
}

/// POST /sudo/upload - Enable/disable upload/submit.
pub fn handle_sudo_set_upload(request: &WasmRouteRequest) -> WasmRouteResponse {
    if let Err(e) = require_sudo(request) {
        return e;
    }

    #[derive(Deserialize)]
    struct Req {
        // Accept both "state" (new) and "enabled" (legacy) formats
        state: Option<String>,
        enabled: Option<bool>,
    }

    let body: Req = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let mut state = get_global_state();
    
    // Handle new "state" format
    if let Some(s) = body.state {
        state.upload_state = match s.as_str() {
            "disabled" => UploadState::Disabled,
            "pending" => UploadState::Pending,
            "enabled" => UploadState::Enabled,
            _ => {
                return json_response(&serde_json::json!({
                    "error": "Invalid state. Use: disabled, pending, or enabled"
                }));
            }
        };
    }
    // Handle legacy "enabled" format
    else if let Some(e) = body.enabled {
        state.upload_state = if e { UploadState::Enabled } else { UploadState::Disabled };
    }
    
    let ok = set_global_state(&state);

    json_response(&serde_json::json!({
        "success": ok,
        "upload_state": match state.upload_state { UploadState::Disabled => "disabled", UploadState::Pending => "pending", UploadState::Enabled => "enabled" },
        "upload_enabled": matches!(state.upload_state, UploadState::Enabled),
    }))
}

/// POST /sudo/ban - Ban a miner hotkey.
pub fn handle_sudo_ban(request: &WasmRouteRequest) -> WasmRouteResponse {
    if let Err(e) = require_sudo(request) {
        return e;
    }

    #[derive(Deserialize)]
    struct BanRequest {
        hotkey: String,
    }

    let body: BanRequest = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let ok = storage::ban_hotkey(&body.hotkey);
    leaderboard::rebuild();

    json_response(&serde_json::json!({
        "success": ok,
        "banned": body.hotkey,
    }))
}

/// POST /sudo/unban - Unban a miner hotkey.
pub fn handle_sudo_unban(request: &WasmRouteRequest) -> WasmRouteResponse {
    if let Err(e) = require_sudo(request) {
        return e;
    }

    #[derive(Deserialize)]
    struct UnbanRequest {
        hotkey: String,
    }

    let body: UnbanRequest = match parse_json_body(request) {
        Some(b) => b,
        None => return bad_request_response(),
    };

    let ok = storage::unban_hotkey(&body.hotkey);
    leaderboard::rebuild();

    json_response(&serde_json::json!({
        "success": ok,
        "unbanned": body.hotkey,
    }))
}

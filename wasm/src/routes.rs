use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use platform_challenge_sdk_wasm::{WasmRouteDefinition, WasmRouteRequest, WasmRouteResponse};

use crate::api::handlers;

pub fn get_route_definitions() -> Vec<WasmRouteDefinition> {
    vec![
        // Public read endpoints
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/leaderboard"),
            description: String::from("Returns current leaderboard with scores and miner hotkeys"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/status"),
            description: String::from("Returns current challenge status and health"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/health"),
            description: String::from("Health check endpoint"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/stats"),
            description: String::from("Challenge statistics: total submissions, active miners"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset"),
            description: String::from("Returns active dataset of evaluation tasks"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset/history"),
            description: String::from("Returns historical dataset selections"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/dataset/consensus"),
            description: String::from("Check dataset consensus status"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/submissions"),
            description: String::from("Returns pending submissions awaiting evaluation"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/submissions/:id"),
            description: String::from("Returns specific submission status by ID"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey"),
            description: String::from("Returns agent info by hotkey"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/logs"),
            description: String::from("Returns evaluation logs for a miner"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/agent/:hotkey/code"),
            description: String::from("Returns stored agent code package for a miner"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/results/:id"),
            description: String::from("Returns evaluation results by ID"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/get_weights"),
            description: String::from("Returns current weight assignments for all miners"),
            requires_auth: false,
        },
        // Configuration endpoints
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/config"),
            description: String::from("Returns current configuration"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/config"),
            description: String::from("Updates configuration (requires auth)"),
            requires_auth: true,
        },
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/config/timeout"),
            description: String::from("Returns current timeout configuration"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/config/timeout"),
            description: String::from("Updates timeout configuration (requires auth)"),
            requires_auth: true,
        },
        // Submission endpoints (auth required)
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/submit"),
            description: String::from("Submit fabricated data package for evaluation"),
            requires_auth: true,
        },
        // Evaluation endpoints
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/evaluate"),
            description: String::from("Trigger evaluation for pending submissions (requires auth)"),
            requires_auth: true,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/dataset/propose"),
            description: String::from("Propose task indices for dataset consensus (requires auth)"),
            requires_auth: true,
        },
        // Admin/Sudo endpoints
        WasmRouteDefinition {
            method: String::from("GET"),
            path: String::from("/sudo/state"),
            description: String::from("Returns current global state (evaluation/upload enabled)"),
            requires_auth: false,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/sudo/evaluation"),
            description: String::from("Enable/disable evaluation (sudo owner only)"),
            requires_auth: true,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/sudo/upload"),
            description: String::from("Enable/disable upload/submit (sudo owner only)"),
            requires_auth: true,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/sudo/ban"),
            description: String::from("Ban a miner hotkey (sudo owner only)"),
            requires_auth: true,
        },
        WasmRouteDefinition {
            method: String::from("POST"),
            path: String::from("/sudo/unban"),
            description: String::from("Unban a miner hotkey (sudo owner only)"),
            requires_auth: true,
        },
    ]
}

pub fn handle_route_request(request: &WasmRouteRequest) -> WasmRouteResponse {
    let path = request.path.as_str();
    let method = request.method.as_str();

    match (method, path) {
        // Public read endpoints
        ("GET", "/leaderboard") => handlers::handle_leaderboard(request),
        ("GET", "/status") => handlers::handle_status(request),
        ("GET", "/health") => handlers::handle_health(request),
        ("GET", "/stats") => handlers::handle_stats(request),
        ("GET", "/dataset") => handlers::handle_dataset(request),
        ("GET", "/dataset/history") => handlers::handle_dataset_history(request),
        ("GET", "/dataset/consensus") => handlers::handle_dataset_consensus(request),
        ("GET", "/submissions") => handlers::handle_submissions(request),
        ("GET", "/get_weights") => handlers::handle_get_weights(request),

        // Configuration endpoints
        ("GET", "/config") => handlers::handle_get_config(request),
        ("POST", "/config") => handlers::handle_set_config(request),
        ("GET", "/config/timeout") => handlers::handle_get_timeout_config(request),
        ("POST", "/config/timeout") => handlers::handle_set_timeout_config(request),

        // Submission endpoint
        ("POST", "/submit") => handlers::handle_submit(request),

        // Evaluation endpoints
        ("POST", "/evaluate") => handlers::handle_evaluate(request),
        ("POST", "/dataset/propose") => handlers::handle_dataset_propose(request),

        // Admin/Sudo endpoints
        ("GET", "/sudo/state") => handlers::handle_sudo_get_state(request),
        ("POST", "/sudo/evaluation") => handlers::handle_sudo_set_evaluation(request),
        ("POST", "/sudo/upload") => handlers::handle_sudo_set_upload(request),
        ("POST", "/sudo/ban") => handlers::handle_sudo_ban(request),
        ("POST", "/sudo/unban") => handlers::handle_sudo_unban(request),

        _ => {
            // Dynamic path matching
            if method == "GET" {
                if path.starts_with("/submissions/") {
                    return handlers::handle_submission_by_id(request);
                }
                if path.starts_with("/results/") {
                    return handlers::handle_results(request);
                }
                if path.starts_with("/agent/") {
                    if path.ends_with("/logs") {
                        return handlers::handle_logs(request);
                    }
                    if path.ends_with("/code") {
                        return handlers::handle_code(request);
                    }
                    // /agent/:hotkey — lookup by hotkey
                    return handlers::handle_agent_by_hotkey(request);
                }
            }
            WasmRouteResponse {
                status: 404,
                body: Vec::new(),
            }
        }
    }
}

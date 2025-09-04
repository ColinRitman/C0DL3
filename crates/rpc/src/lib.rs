use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// RPC server for COLD L3
pub struct RPCServer {
    state: Arc<RwLock<RPCState>>,
}

/// RPC server state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPCState {
    pub node_status: String,
    pub current_height: u64,
    pub total_transactions: u64,
    pub connected_peers: usize,
}

impl Default for RPCState {
    fn default() -> Self {
        Self {
            node_status: "running".to_string(),
            current_height: 0,
            total_transactions: 0,
            connected_peers: 0,
        }
    }
}

/// JSON-RPC request
#[derive(Debug, Deserialize)]
pub struct RPCRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
pub struct RPCResponse {
    pub jsonrpc: String,
    pub result: serde_json::Value,
    pub id: u64,
}

/// JSON-RPC error response
#[derive(Debug, Serialize)]
pub struct RPCError {
    pub jsonrpc: String,
    pub error: RPCErrorData,
    pub id: u64,
}

/// JSON-RPC error data
#[derive(Debug, Serialize)]
pub struct RPCErrorData {
    pub code: i32,
    pub message: String,
}

impl RPCServer {
    /// Create a new RPC server
    pub async fn new() -> Result<Self> {
        info!("Creating RPC server");
        
        Ok(Self {
            state: Arc::new(RwLock::new(RPCState::default())),
        })
    }

    /// Start the RPC server
    pub async fn start(&self, addr: SocketAddr) -> Result<()> {
        info!("Starting RPC server on {}", addr);
        
        let state = self.state.clone();
        
        // Create router
        let app = Router::new()
            .route("/", post(handle_rpc))
            .route("/health", get(health_check))
            .route("/status", get(get_status))
            .with_state(state);
        
        // Start server
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    /// Update server state
    pub async fn update_state(&self, new_state: RPCState) -> Result<()> {
        let mut state = self.state.write().await;
        *state = new_state;
        Ok(())
    }
}

/// Handle JSON-RPC requests
async fn handle_rpc(
    State(state): State<Arc<RwLock<RPCState>>>,
    Json(request): Json<RPCRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("Handling RPC request: {}", request.method);
    
    let response = match request.method.as_str() {
        "getBlockHeight" => {
            let state = state.read().await;
            RPCResponse {
                jsonrpc: "2.0".to_string(),
                result: serde_json::json!({
                    "height": state.current_height
                }),
                id: request.id,
            }
        }
        "getNodeStatus" => {
            let state = state.read().await;
            RPCResponse {
                jsonrpc: "2.0".to_string(),
                result: serde_json::json!({
                    "status": state.node_status,
                    "connected_peers": state.connected_peers
                }),
                id: request.id,
            }
        }
        "getTransactionCount" => {
            let state = state.read().await;
            RPCResponse {
                jsonrpc: "2.0".to_string(),
                result: serde_json::json!({
                    "total_transactions": state.total_transactions
                }),
                id: request.id,
            }
        }
        _ => {
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    
    Ok(Json(serde_json::to_value(response).unwrap()))
}

/// Health check endpoint
async fn health_check() -> StatusCode {
    StatusCode::OK
}

/// Get node status endpoint
async fn get_status(State(state): State<Arc<RwLock<RPCState>>>) -> Json<RPCState> {
    let state = state.read().await;
    Json(state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpc_server_creation() {
        let server = RPCServer::new().await.unwrap();
        assert!(true); // If we get here, creation succeeded
    }

    #[tokio::test]
    async fn test_state_update() {
        let server = RPCServer::new().await.unwrap();
        
        let new_state = RPCState {
            node_status: "stopped".to_string(),
            current_height: 100,
            total_transactions: 1000,
            connected_peers: 5,
        };
        
        server.update_state(new_state).await.unwrap();
        
        let current_state = server.state.read().await.clone();
        assert_eq!(current_state.current_height, 100);
        assert_eq!(current_state.total_transactions, 1000);
    }
}

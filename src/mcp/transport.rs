//! MCP transport layer for stdio and SSE.
//!
//! Provides functions to start the MCP server with different transports.

use rmcp::ServiceExt;
use rmcp::transport::io::stdio;

use super::server::RlmMcpServer;

/// Starts the MCP server with stdio transport.
///
/// The server reads JSON-RPC messages from stdin and writes responses to stdout.
/// This is the standard transport for Claude Code integration.
///
/// # Errors
///
/// Returns an error if the server fails to start or encounters a runtime error.
pub async fn serve_stdio(server: RlmMcpServer) -> anyhow::Result<()> {
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// Starts the MCP server with streamable HTTP transport.
///
/// Listens on the given host and port for incoming MCP connections at `/mcp`.
/// Named `serve_sse` for CLI familiarity; the underlying transport is MCP's
/// streamable HTTP (the successor to the legacy SSE transport).
///
/// # Errors
///
/// Returns an error if the server fails to bind or encounters a runtime error.
pub async fn serve_sse(server: RlmMcpServer, host: &str, port: u16) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    use std::sync::Arc;

    let ct = tokio_util::sync::CancellationToken::new();

    // Capture the server's config so the factory can recreate instances
    let db_path = server.db_path().to_path_buf();

    let service = StreamableHttpService::new(
        move || {
            RlmMcpServer::new(db_path.clone()).map_err(|e| std::io::Error::other(e.to_string()))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let addr = format!("{host}:{port}");
    let tcp_listener = tokio::net::TcpListener::bind(&addr).await?;

    // Log to stderr since stdout is reserved for MCP protocol messages
    #[allow(clippy::print_stderr)]
    {
        eprintln!("RLM-RS MCP server listening on http://{addr}/mcp");
    }

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            ct.cancel();
        })
        .await?;

    Ok(())
}

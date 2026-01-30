//! MCP server implementation for rlm-rs.
//!
//! Exposes the agentic query pipeline and storage as MCP tools and resources.
//! Uses `spawn_blocking` to bridge the `!Send` [`SqliteStorage`] into the
//! async rmcp runtime.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, Implementation, ListResourceTemplatesResult,
    ListResourcesResult, PaginatedRequestParams, ProtocolVersion, RawResource, RawResourceTemplate,
    ReadResourceRequestParams, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, tool, tool_handler, tool_router};

use crate::agent::client::create_provider;
use crate::agent::config::AgentConfig;
use crate::agent::finding::Relevance;
use crate::agent::orchestrator::{CliOverrides, Orchestrator};
use crate::storage::{SqliteStorage, Storage};

use super::params::QueryParams;

/// Opens storage and verifies it is initialized. Returns an `McpError` on failure.
fn open_storage(db_path: &std::path::Path) -> Result<SqliteStorage, McpError> {
    let storage = SqliteStorage::open(db_path)
        .map_err(|e| McpError::internal_error(format!("Failed to open storage: {e}"), None))?;

    if !storage.is_initialized().unwrap_or(false) {
        return Err(McpError::internal_error(
            "RLM database not initialized. Run `rlm-rs init` first.",
            None,
        ));
    }

    Ok(storage)
}

/// RLM-RS MCP server.
///
/// Provides MCP tools for running the agentic query pipeline and MCP resources
/// for browsing buffers and chunks in storage.
#[derive(Clone)]
pub struct RlmMcpServer {
    tool_router: ToolRouter<Self>,
    db_path: PathBuf,
    orchestrator: Arc<Orchestrator>,
}

#[tool_router]
impl RlmMcpServer {
    /// Run the full agentic query pipeline: plan → search → fan-out → synthesis.
    ///
    /// Accepts a query and buffer name, executes the entire RLM pipeline
    /// internally, and returns the synthesized result as JSON.
    #[tool(
        name = "query",
        description = "Execute the RLM agentic query pipeline. Plans an analysis strategy, searches for relevant chunks, fans out concurrent subcall agents for extraction, and synthesizes findings into a coherent response. Returns JSON with the synthesized response and pipeline metadata."
    )]
    async fn query(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();
        let orchestrator = self.orchestrator.clone();

        let result = tokio::task::spawn_blocking(move || {
            let storage = open_storage(&db_path)?;

            // Resolve buffer name
            let buffer_name = &params.buffer_name;
            storage
                .get_buffer_by_name(buffer_name)
                .map_err(|e| McpError::internal_error(format!("Storage error: {e}"), None))?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Buffer not found: {buffer_name}"), None)
                })?;

            let cli_overrides = CliOverrides {
                search_mode: params.search_mode,
                batch_size: params.batch_size,
                threshold: params.threshold,
                max_chunks: params.max_chunks.filter(|&n| n > 0),
                top_k: params.top_k,
                num_agents: params.num_agents,
                finding_threshold: params.finding_threshold.map(|s| Relevance::parse(&s)),
                skip_plan: params.skip_plan,
            };

            // Run the async orchestrator from within the blocking context
            let rt = tokio::runtime::Handle::current();
            rt.block_on(orchestrator.query(
                &storage,
                &params.query,
                Some(buffer_name.as_str()),
                Some(cli_overrides),
            ))
            .map_err(|e| McpError::internal_error(format!("Query pipeline failed: {e}"), None))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {e}"), None))??;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("Serialization error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for RlmMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "rlm-rs".to_string(),
                title: Some("RLM-RS MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: Some("https://github.com/zircote/rlm-rs".to_string()),
            },
            instructions: Some(
                "RLM-RS: Recursive Language Model server for analyzing documents that exceed \
                 context limits. Use the `query` tool to run the full agentic pipeline. \
                 Browse buffers and chunks via resources."
                    .to_string(),
            ),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let db_path = self.db_path.clone();

        let resources = tokio::task::spawn_blocking(move || {
            let Ok(storage) = open_storage(&db_path) else {
                return Ok(Vec::new()); // Not initialized → empty list
            };

            let buffers = storage.list_buffers().map_err(|e| {
                McpError::internal_error(format!("Failed to list buffers: {e}"), None)
            })?;

            let mut resources: Vec<Resource> = Vec::new();
            for buf in buffers {
                let name = match &buf.name {
                    Some(n) => n.clone(),
                    None => continue,
                };
                let uri = format!("rlm-rs://{name}");
                let chunk_count = buf.metadata.chunk_count.unwrap_or(0);

                let mut raw = RawResource::new(uri, format!("Buffer: {name}"));
                raw.description = Some(format!(
                    "{} bytes, {} chunks",
                    buf.metadata.size, chunk_count,
                ));
                raw.mime_type = Some("application/json".to_string());
                resources.push(raw.no_annotation());

                // Also list individual chunks as sub-resources
                if let Some(buffer_id) = buf.id
                    && let Ok(chunks) = storage.get_chunks(buffer_id)
                {
                    for chunk in &chunks {
                        let chunk_uri = format!("rlm-rs://{name}/{}", chunk.index);
                        let mut chunk_raw =
                            RawResource::new(chunk_uri, format!("{name}/chunk-{}", chunk.index));
                        chunk_raw.description = Some(format!(
                            "Chunk {} ({} bytes)",
                            chunk.index,
                            chunk.content.len(),
                        ));
                        chunk_raw.mime_type = Some("text/plain".to_string());
                        resources.push(chunk_raw.no_annotation());
                    }
                }
            }

            Ok::<_, McpError>(resources)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {e}"), None))??;

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParams { uri, .. }: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let path = uri
            .strip_prefix("rlm-rs://")
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("Invalid URI scheme, expected rlm-rs://: {uri}"),
                    None,
                )
            })?
            .to_string();

        let db_path = self.db_path.clone();
        let response_uri = uri.clone();

        let content = tokio::task::spawn_blocking(move || {
            let storage = open_storage(&db_path)?;

            let parts: Vec<&str> = path.split('/').collect();

            match parts.as_slice() {
                [buffer_name] => {
                    // Buffer metadata as JSON
                    let buf = storage
                        .get_buffer_by_name(buffer_name)
                        .map_err(|e| {
                            McpError::internal_error(format!("Storage error: {e}"), None)
                        })?
                        .ok_or_else(|| {
                            McpError::resource_not_found(
                                format!("Buffer not found: {buffer_name}"),
                                None,
                            )
                        })?;

                    serde_json::to_string_pretty(&buf).map_err(|e| {
                        McpError::internal_error(format!("Serialization error: {e}"), None)
                    })
                }
                [buffer_name, chunk_idx_str] => {
                    // Chunk content by index
                    let buf = storage
                        .get_buffer_by_name(buffer_name)
                        .map_err(|e| {
                            McpError::internal_error(format!("Storage error: {e}"), None)
                        })?
                        .ok_or_else(|| {
                            McpError::resource_not_found(
                                format!("Buffer not found: {buffer_name}"),
                                None,
                            )
                        })?;

                    let buffer_id = buf.id.ok_or_else(|| {
                        McpError::internal_error("Buffer has no ID", None)
                    })?;

                    let idx: usize = chunk_idx_str.parse().map_err(|_| {
                        McpError::invalid_params(
                            format!("Invalid chunk index: {chunk_idx_str}"),
                            None,
                        )
                    })?;

                    let chunks = storage.get_chunks(buffer_id).map_err(|e| {
                        McpError::internal_error(format!("Storage error: {e}"), None)
                    })?;

                    let chunk = chunks
                        .iter()
                        .find(|c| c.index == idx)
                        .ok_or_else(|| {
                            McpError::resource_not_found(
                                format!("Chunk index {idx} not found in buffer {buffer_name}"),
                                None,
                            )
                        })?;

                    Ok(chunk.content.clone())
                }
                _ => Err(McpError::invalid_params(
                    format!("Invalid URI format: {path}. Expected rlm-rs://{{buffer}} or rlm-rs://{{buffer}}/{{chunk_index}}"),
                    None,
                )),
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {e}"), None))??;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(content, response_uri)],
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let buf_template = RawResourceTemplate {
            uri_template: "rlm-rs://{buffer_name}".to_string(),
            name: "Buffer metadata".to_string(),
            title: None,
            description: Some(
                "Returns JSON metadata for the named buffer including size, chunk count, and content type."
                    .to_string(),
            ),
            mime_type: Some("application/json".to_string()),
            icons: None,
        };

        let chunk_template = RawResourceTemplate {
            uri_template: "rlm-rs://{buffer_name}/{chunk_index}".to_string(),
            name: "Chunk content".to_string(),
            title: None,
            description: Some(
                "Returns the text content of a specific chunk by buffer name and chunk index."
                    .to_string(),
            ),
            mime_type: Some("text/plain".to_string()),
            icons: None,
        };

        Ok(ListResourceTemplatesResult {
            resource_templates: vec![buf_template.no_annotation(), chunk_template.no_annotation()],
            next_cursor: None,
            meta: None,
        })
    }
}

impl RlmMcpServer {
    /// Returns the database path.
    #[must_use]
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    /// Creates a new MCP server.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the RLM `SQLite` database.
    ///
    /// # Errors
    ///
    /// Returns an error if the agent configuration cannot be loaded from
    /// environment variables or if the LLM provider cannot be created.
    pub fn new(db_path: PathBuf) -> Result<Self, crate::error::Error> {
        let config = AgentConfig::from_env().map_err(|e| {
            crate::error::CommandError::ExecutionFailed(format!("Agent configuration error: {e}"))
        })?;

        let provider = create_provider(&config).map_err(|e| {
            crate::error::CommandError::ExecutionFailed(format!("Provider creation failed: {e}"))
        })?;

        let orchestrator = Arc::new(Orchestrator::new(Arc::from(provider), config));

        Ok(Self {
            tool_router: Self::tool_router(),
            db_path,
            orchestrator,
        })
    }
}

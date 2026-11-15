#![forbid(unsafe_code)]

pub mod agent_runtime;
pub mod application;
pub mod cli;
pub mod context;
pub mod context_cells;
pub mod dashboard;
pub mod events;
pub mod execution_state;
pub mod export;
pub mod github_integration;
pub mod impact;
pub mod indexer;
pub mod mcp;
pub mod mcp_manager;
pub mod memory;
pub mod model;
pub mod parser;
pub mod portal;
pub mod rest;
pub mod routing;
pub mod search;
pub mod secret;
pub mod server;
pub mod settings;
pub mod skills;
pub mod storage;
pub mod tasks;
pub mod util;
pub mod workspace;

pub use context::{ContextOptions, build_context};
pub use indexer::{IndexOptions, build as build_index};
pub use model::{ContextBundle, GraphIndex, ImpactReport, Result, SearchHit, Symbol};
pub use search::find_symbols;
pub use storage::{load as load_index, save as save_index};

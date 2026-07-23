#![forbid(unsafe_code)]

pub mod application;
pub mod cli;
pub mod context;
pub mod dashboard;
pub mod events;
pub mod export;
pub mod impact;
pub mod indexer;
pub mod mcp;
pub mod mcp_manager;
pub mod memory;
pub mod model;
pub mod parser;
pub mod rest;
pub mod search;
pub mod secret;
pub mod server;
pub mod settings;
pub mod skills;
pub mod storage;
pub mod util;
pub mod workspace;

pub use context::{build_context, ContextOptions};
pub use indexer::{build as build_index, IndexOptions};
pub use model::{ContextBundle, GraphIndex, ImpactReport, Result, SearchHit, Symbol};
pub use search::find_symbols;
pub use storage::{load as load_index, save as save_index};

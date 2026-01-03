//! MCP (Model Context Protocol) JSON-RPC server implementation for valknut.
//!
//! This module provides a complete implementation of an MCP server that exposes
//! valknut's code analysis capabilities through JSON-RPC 2.0 over stdin/stdout.

pub mod formatters;
pub mod protocol;
pub mod server;
pub mod tools;

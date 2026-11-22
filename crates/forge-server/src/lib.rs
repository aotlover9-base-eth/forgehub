//! `ForgeHub` server library: configuration, storage, repositories,
//! and the HTTP layer. The binary in `main.rs` is a thin entry point
//! over this library so tests can exercise everything in-process.

#![forbid(unsafe_code)]

pub mod admin;
pub mod auth;
pub mod collections;
pub mod config;
pub mod db;
pub mod device;
pub mod discovery;
pub mod error;
pub mod http;
pub mod imports;
pub mod mail;
pub mod pages;
pub mod ratelimit;
pub mod repo;
pub mod snippets;
pub mod state;
pub mod tokens;

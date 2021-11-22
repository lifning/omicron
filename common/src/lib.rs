/*!
 * # Oxide Control Plane
 *
 * The overall architecture for the Oxide Control Plane is described in [RFD
 * 61](https://61.rfd.oxide.computer/).  This crate implements common facilities
 * used in the control plane.  Other top-level crates implement pieces of the
 * control plane (e.g., `omicron_nexus`).
 *
 * The best documentation for the control plane is RFD 61 and the rustdoc in
 * this crate.  Since this crate doesn't provide externally-consumable
 * interfaces, the rustdoc (generated with `--document-private-items`) is
 * intended primarily for engineers working on this crate.
 */

/*
 * We only use rustdoc for internal documentation, including private items, so
 * it's expected that we'll have links to private items in the docs.
 */
#![allow(rustdoc::private_intra_doc_links)]
/* TODO(#32): Remove this exception once resolved. */
#![allow(clippy::field_reassign_with_default)]

pub mod api;
pub mod backoff;
pub mod cmd;
pub mod config;
pub mod packaging;

macro_rules! generate_logging_api {
    ($path:literal) => {
        progenitor::generate_api!(
            $path,
            slog::Logger,
            |log: &slog::Logger, request: &reqwest::Request| {
                debug!(log, "client request";
                    "method" => %request.method(),
                    "uri" => %request.url(),
                    "body" => ?&request.body(),
                );
            },
            |log: &slog::Logger, result: &Result<_, _>| {
                debug!(log, "client response"; "result" => ?result);
            },
        );
    };
}

pub mod sled_agent_client;
pub use sled_agent_client::Client as SledAgentClient;
pub use sled_agent_client::TestInterfaces as SledAgentTestInterfaces;
pub mod nexus_client;
pub use nexus_client::Client as NexusClient;
pub mod oximeter_client;
pub use oximeter_client::Client as OximeterClient;

#[macro_use]
extern crate slog;

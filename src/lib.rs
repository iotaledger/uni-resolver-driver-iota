// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::env;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, ensure, Context};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use identity_iota::document::CoreDocument;
use identity_iota::iota::{IotaDID, IotaDocumentMetadata};
use identity_iota::prelude::Resolver;
use identity_iota::resolver::ErrorCause;
use identity_iota_core::rebased::client::IdentityClientReadOnly;
use identity_iota_core::IotaDocument;
use iota_sdk::types::base_types::ObjectID;
use iota_sdk::{IotaClient, IotaClientBuilder};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

type SharedResolver = Arc<Resolver<IotaDocument>>;

/// Custom endpoint for the IOTA network.
pub const IOTA_CUSTOM_NODE_ENDPOINT: &str = "IOTA_CUSTOM_NODE_ENDPOINT";
pub const IOTA_CUSTOM_IDENTITY_PKG_ID: &str = "IOTA_CUSTOM_IDENTITY_PKG_ID";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
// TODO: Add mainnet
pub enum Network {
    /// Testnet configuration
    Testnet,
    /// Devnet configuration
    Devnet,
    /// Custom network configuration with required endpoint and package ID
    Custom { endpoint: String, pkg_id: String },
}

impl Network {
    /// Parses the `NETWORK` environment variable to create a set of `Network` configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - NETWORK environment variable is not set
    /// - Custom network is specified but required environment variables are missing
    /// - Unsupported network type is specified
    pub fn from_env() -> anyhow::Result<HashSet<Self>> {
        let network_var = env::var("NETWORK").context("NETWORK environment variable is not set")?;
        let mut networks = HashSet::new();

        for network in network_var.split(',') {
            match network.trim().to_lowercase().as_str() {
                "testnet" => {
                    networks.insert(Self::Testnet);
                }
                "devnet" => {
                    networks.insert(Self::Devnet);
                }
                "custom" => {
                    let endpoint = env::var(IOTA_CUSTOM_NODE_ENDPOINT)
                        .context("Custom network requires IOTA_CUSTOM_NODE_ENDPOINT to be set")?;

                    let pkg_id = env::var(IOTA_CUSTOM_IDENTITY_PKG_ID)
                        .context("Custom network requires IOTA_CUSTOM_IDENTITY_PKG_ID to be set")?;

                    networks.insert(Self::Custom { endpoint, pkg_id });
                }
                invalid => bail!("Unsupported network type: {}", invalid),
            }
        }

        ensure!(!networks.is_empty(), "No valid networks were specified");

        Ok(networks)
    }

    /// Returns an IOTA client configured for the network.
    ///
    /// # Errors
    ///
    /// Returns an error if the client cannot be created.
    pub async fn get_client(&self) -> anyhow::Result<IotaClient> {
        let client = match self {
            Network::Testnet => IotaClientBuilder::default()
                .build_testnet()
                .await
                .context("Failed to create testnet client")?,

            Network::Devnet => IotaClientBuilder::default()
                .build_devnet()
                .await
                .context("Failed to create devnet client")?,

            Network::Custom { endpoint, .. } => IotaClientBuilder::default()
                .build(endpoint)
                .await
                .context("Failed to create custom network client")?,
        };

        Ok(client)
    }
}
#[derive(Default)]
pub struct Server {
    resolver: Option<SharedResolver>,
}

impl Server {
    pub fn with_resolver(mut self, resolver: Resolver<IotaDocument>) -> Self {
        self.resolver = Some(Arc::new(resolver));
        self
    }

    pub async fn run(self, listener: TcpListener) -> anyhow::Result<()> {
        let resolver = match self.resolver {
            Some(resolver) => resolver,
            None => init_resolver().await?,
        };
        let app = app(resolver).await?;
        let addr = listener.local_addr()?;

        tracing::debug!("Server is starting at {addr}");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionResponse {
    pub did_document: CoreDocument,
    pub did_resolution_metadata: IotaDocumentMetadata,
}

#[tracing::instrument(
    name = "Resolve DID",
    level = "debug",
    skip_all,
    fields(did = %arg),
    ret,
    err(Debug),
)]
async fn resolve_did(
    Path(arg): Path<String>,
    State(resolver): State<SharedResolver>,
) -> Result<Json<ResolutionResponse>, (StatusCode, String)> {
    let did = IotaDID::parse(&arg).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let identity = resolver.resolve(&did).await.map_err(|e| match e.error_cause() {
        ErrorCause::HandlerError { source, .. } if source.to_string().contains("could not find") => (
            StatusCode::NOT_FOUND,
            "The requested DID document was not found".to_owned(),
        ),

        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    })?;

    Ok(Json(ResolutionResponse {
        did_document: identity.core_document().clone(),
        did_resolution_metadata: identity.metadata.clone(),
    }))
}

async fn app(resolver: SharedResolver) -> anyhow::Result<Router> {
    Ok(Router::new()
        .route("/1.0/identifiers/{did}", get(resolve_did))
        .with_state(resolver))
}

/// Initialize identity clients for all configured networks.
async fn init_resolver() -> anyhow::Result<SharedResolver> {
    let mut clients = vec![];
    let networks = Network::from_env()?;

    for network in networks {
        let client = network.get_client().await.context("Failed to create IOTA client")?;

        let identity_client = match network {
            Network::Custom { pkg_id, .. } => {
                let pkg_id = ObjectID::from_str(&pkg_id).context("Failed to parse custom network package ID")?;

                IdentityClientReadOnly::new_with_pkg_id(client, pkg_id)
                    .await
                    .context("Failed to create custom network identity client")?
            }
            Network::Testnet | Network::Devnet => IdentityClientReadOnly::new(client)
                .await
                .context("Failed to create identity client")?,
        };

        let network_name = identity_client.network().to_string();
        tracing::debug!("Initialized client for network: {}", network_name);
        let network_name: &'static str = Box::leak(network_name.into_boxed_str());
        clients.push((network_name, identity_client));
    }

    ensure!(
        !clients.is_empty(),
        "No clients were created. Make sure you provide a configuration for at least one network"
    );

    let mut resolver = Resolver::<IotaDocument>::new();

    resolver.attach_multiple_iota_handlers(clients);

    Ok(Arc::new(resolver))
}

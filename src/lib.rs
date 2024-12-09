// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
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
use identity_iota_core::rebased::client::IdentityClientReadOnly;
use identity_iota_core::rebased::migration::get_identity;
use iota_sdk::types::base_types::ObjectID;
use iota_sdk::{IotaClient, IotaClientBuilder};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

type NetworkClients = Arc<HashMap<String, IdentityClientReadOnly>>;

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
    clients: Option<NetworkClients>,
}

impl Server {
    pub fn with_clients(mut self, clients: HashMap<String, IdentityClientReadOnly>) -> Self {
        self.clients = Some(Arc::new(clients));
        self
    }

    pub async fn run(self, listener: TcpListener) -> anyhow::Result<()> {
        let clients = match self.clients {
            Some(clients) => clients,
            None => init_clients().await?,
        };
        let app = app(clients).await?;
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
    State(clients): State<NetworkClients>,
) -> Result<Json<ResolutionResponse>, (StatusCode, String)> {
    let did = IotaDID::parse(&arg).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let network = did.network_str().to_string();

    let object_id = ObjectID::from_str(did.tag_str()).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let client = clients
        .get(&network)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Unsupported network: {}", network)))?;

    let identity = get_identity(client, object_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "The requested DID document was not found".to_owned(),
            )
        })?;

    Ok(Json(ResolutionResponse {
        did_document: identity.core_document().clone(),
        did_resolution_metadata: identity.metadata.clone(),
    }))
}

async fn app(clients: NetworkClients) -> anyhow::Result<Router> {
    Ok(Router::new()
        .route("/1.0/identifiers/:did", get(resolve_did))
        .with_state(clients))
}

/// Initialize identity clients for all configured networks.
async fn init_clients() -> anyhow::Result<NetworkClients> {
    let mut clients = HashMap::new();
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

        if clients.insert(network_name.clone(), identity_client).is_some() {
            tracing::warn!("Overwrote existing client for network: {}", network_name);
        }
    }

    ensure!(!clients.is_empty(), "No identity clients were created");

    Ok(Arc::new(clients))
}

// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Display;
use std::sync::Arc;

use anyhow::{bail, ensure, Context};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use identity_iota::core::Url;
use identity_iota::document::CoreDocument;
use identity_iota::iota::{IotaDID, IotaDocumentMetadata};
use identity_iota_core::rebased::client::IdentityClientReadOnly;
use identity_iota_core::rebased::Error as IdentityError;
use identity_iota_core::IotaDocument;
use iota_sdk::types::base_types::ObjectID;
use iota_sdk::{IotaClient, IotaClientBuilder};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

type SharedResolver = Arc<Resolver>;

/// Custom endpoint for the IOTA network.
pub const IOTA_CUSTOM_NODE_ENDPOINT: &str = "IOTA_CUSTOM_NODE_ENDPOINT";
pub const IOTA_CUSTOM_IDENTITY_PKG_ID: &str = "IOTA_CUSTOM_IDENTITY_PKG_ID";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Network {
    /// Mainnet configuration.
    Mainnet,
    /// Testnet configuration.
    Testnet,
    /// Devnet configuration.
    Devnet,
    /// Custom network configuration with required endpoint and package ID.
    Custom { endpoint: Url, pkg_id: ObjectID },
}

impl Network {
    /// Parses the `NETWORK` environment variable to create a set of `Network` configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - NETWORK environment variable is not set;
    /// - Custom network is specified but required environment variables are missing;
    /// - Unsupported network type is specified;
    pub fn from_env() -> anyhow::Result<HashSet<Self>> {
        let network_var = env::var("NETWORK").context("`NETWORK` environment variable is not set")?;
        let mut networks = HashSet::new();

        for network in network_var.split(',') {
            match network.trim().to_lowercase().as_str() {
                "mainnet" => {
                    networks.insert(Self::Mainnet);
                }
                "testnet" => {
                    networks.insert(Self::Testnet);
                }
                "devnet" => {
                    networks.insert(Self::Devnet);
                }
                "custom" => {
                    let endpoint = env::var(IOTA_CUSTOM_NODE_ENDPOINT)
                        .context("Custom network requires env variable `IOTA_CUSTOM_NODE_ENDPOINT` to be set")?
                        .parse()
                        .context("provided endpoint is not a valid URL")?;

                    let pkg_id = env::var(IOTA_CUSTOM_IDENTITY_PKG_ID)
                        .context("Custom network requires env variable `IOTA_CUSTOM_IDENTITY_PKG_ID` to be set")?
                        .parse()
                        .context("malformed package ID")?;

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
            Network::Mainnet => IotaClientBuilder::default()
                .build_mainnet()
                .await
                .context("failed to create mainnet client")?,
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
    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
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
) -> Result<Json<ResolutionResponse>, Response> {
    let did = IotaDID::parse(&arg).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;
    let did_document = resolver.resolve(&did).await.map_err(IntoResponse::into_response)?;

    Ok(Json(ResolutionResponse {
        did_document: did_document.core_document().clone(),
        did_resolution_metadata: did_document.metadata,
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
            Network::Custom { pkg_id, .. } => IdentityClientReadOnly::new_with_pkg_id(client, pkg_id)
                .await
                .context("Failed to create custom network identity client")?,
            Network::Mainnet | Network::Testnet | Network::Devnet => IdentityClientReadOnly::new(client)
                .await
                .context("Failed to create identity client")?,
        };

        let network_name = identity_client.network().to_string();
        tracing::debug!("Initialized client for network: {}", network_name);
        clients.push((network_name, identity_client));
    }

    ensure!(
        !clients.is_empty(),
        "No clients were created. Make sure you provide a configuration for at least one network"
    );

    Ok(Arc::new(Resolver::new(clients)))
}

#[derive(Debug)]
pub enum DidResolutionErrorKind {
    NotFound(Box<IdentityError>),
    UnknownNetwork(String),
    Client(Box<IdentityError>),
}

impl Display for DidResolutionErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(e) => e.fmt(f),
            Self::UnknownNetwork(network) => write!(f, "unknown network `{network}`"),
            Self::Client(_) => f.write_str("internal client error"),
        }
    }
}

impl std::error::Error for DidResolutionErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotFound(e) => Some(e),
            Self::Client(e) => Some(e),
            Self::UnknownNetwork(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct DidResolutionError {
    pub did: IotaDID,
    pub kind: DidResolutionErrorKind,
}

impl Display for DidResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to resolve DID `{}`", self.did)
    }
}

impl std::error::Error for DidResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}

impl IntoResponse for DidResolutionError {
    fn into_response(self) -> axum::response::Response {
        use DidResolutionErrorKind::*;
        let status_code = match &self.kind {
            NotFound(_) => StatusCode::NOT_FOUND,
            UnknownNetwork(_) => StatusCode::BAD_REQUEST,
            Client(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        // Use anyhow to format the error and its sources.
        let err_msg = format!("{:#}", anyhow::Error::from(self));
        (status_code, err_msg).into_response()
    }
}

pub struct Resolver {
    clients: HashMap<String, IdentityClientReadOnly>,
}

impl Resolver {
    pub fn new(clients: impl IntoIterator<Item = (String, IdentityClientReadOnly)>) -> Self {
        let clients = clients.into_iter().collect();
        Self { clients }
    }

    pub async fn resolve(&self, did: &IotaDID) -> Result<IotaDocument, DidResolutionError> {
        let network = did.network_str();
        let client = self.clients.get(network).ok_or_else(|| DidResolutionError {
            did: did.clone(),
            kind: DidResolutionErrorKind::UnknownNetwork(network.to_owned()),
        })?;

        match client.resolve_did(did).await {
            Ok(doc) => Ok(doc),
            Err(e @ IdentityError::DIDResolutionError(_)) => Err(DidResolutionErrorKind::NotFound(Box::new(e))),
            Err(e) => Err(DidResolutionErrorKind::Client(Box::new(e))),
        }
        .map_err(|kind| DidResolutionError { did: did.clone(), kind })
    }
}

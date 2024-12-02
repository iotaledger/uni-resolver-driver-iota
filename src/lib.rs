// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Context};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use identity_iota::document::CoreDocument;
use identity_iota::iota::{IotaDID, IotaDocumentMetadata};
use identity_iota_core::rebased::client::IdentityClientReadOnly;
use identity_iota_core::rebased::migration::get_identity;
// use identity_iota_core::rebased::migration::identity::get_identity;
use iota_sdk::types::base_types::ObjectID;
use iota_sdk::IotaClientBuilder;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

type NetworkClients = Arc<HashMap<String, IdentityClientReadOnly>>;

/// Custom endpoint for the IOTA network.
pub const IOTA_CUSTOM_NODE_ENDPOINT: &str = "IOTA_CUSTOM_NODE_ENDPOINT";
pub const IOTA_CUSTOM_IDENTITY_PKG_ID: &str = "IOTA_CUSTOM_IDENTITY_PKG_ID";

/// The identity package ID to use for the resolver. This is for the mainnet.
pub const IOTA_MAINNET_IDENTITY_PKG_ID: &str = "IOTA_MAINNET_IDENTITY_PKG_ID";
const IOTA_MAINNET_NODE_ENDPOINT: &str = "IOTA_MAINNET_NODE_ENDPOINT";

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

async fn init_clients() -> anyhow::Result<NetworkClients> {
    let mut clients = HashMap::new();

    let networks = [
        (IOTA_MAINNET_NODE_ENDPOINT, IOTA_MAINNET_IDENTITY_PKG_ID),
        (IOTA_CUSTOM_NODE_ENDPOINT, IOTA_CUSTOM_IDENTITY_PKG_ID),
    ];

    for (endpoint_var, pkg_id_var) in networks {
        if let (Some(endpoint), Some(pkg_id)) = (env::var(endpoint_var).ok(), env::var(pkg_id_var).ok()) {
            let client = IotaClientBuilder::default()
                .build(&endpoint)
                .await
                .with_context(|| "unable to create a client".to_string())?;

            let identity_pkg_id =
                ObjectID::from_str(&pkg_id).context("unable to parse the provided identity package ID")?;

            let identity_client = IdentityClientReadOnly::new(client, identity_pkg_id)
                .await
                .context("unable to create an identity client")?;

            let network = identity_client.network().to_string();
            clients.insert(network, identity_client);
        }
    }

    if clients.is_empty() {
        bail!("No identity clients were created");
    }

    Ok(Arc::new(clients))
}

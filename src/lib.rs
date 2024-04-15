// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Context};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use identity_iota::{
    document::CoreDocument,
    iota::{IotaDID, IotaDocument, IotaDocumentMetadata},
    resolver::Resolver,
};
use iota_sdk::client::Client;
use serde::{Deserialize, Serialize};
use std::{env, sync::Arc};
use tokio::net::TcpListener;

type SharedResolver = Arc<Resolver<IotaDocument>>;

pub const IOTA_NETWORK_NAME: &str = "iota";
pub const SMR_NETWORK_NAME: &str = "smr";
pub const IOTA_NODE_ENDPOINT: &str = "IOTA_NODE_ENDPOINT";
pub const SMR_NODE_ENDPOINT: &str = "SMR_NODE_ENDPOINT";
pub const IOTA_CUSTOM_NETWORK_NAME: &str = "IOTA_CUSTOM_NETWORK_NAME";
pub const IOTA_CUSTOM_NODE_ENDPOINT: &str = "IOTA_CUSTOM_NODE_ENDPOINT";

#[derive(Debug, Default)]
pub struct Server {
    resolver: Option<SharedResolver>,
}

impl Server {
    pub fn with_resolver(mut self, resolver: Resolver<IotaDocument>) -> Self {
        self.resolver = Some(Arc::new(resolver));
        self
    }

    pub async fn run(self, listener: TcpListener) -> anyhow::Result<()> {
        let resolver = self.resolver.unwrap_or(resolver().await?);
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
    let resolved = resolver
        .resolve(&did)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ResolutionResponse {
        did_document: resolved.core_document().clone(),
        did_resolution_metadata: resolved.metadata,
    }))
}

async fn app(resolver: SharedResolver) -> anyhow::Result<Router> {
    Ok(Router::new()
        .route("/1.0/identifiers/:did", get(resolve_did))
        .with_state(resolver))
}

async fn resolver() -> anyhow::Result<SharedResolver> {
    let mut clients = vec![];

    if let Ok(iota_endpoint) = env::var(IOTA_NODE_ENDPOINT) {
        let client: Client = Client::builder()
            .with_primary_node(&iota_endpoint, None)
            .context("unable to create a client for the provided endpoint")?
            .finish()
            .await
            .context("unable to create a client for the provided endpoint")?;

        clients.push((IOTA_NETWORK_NAME, client));
    }

    if let Ok(iota_endpoint) = env::var(SMR_NODE_ENDPOINT) {
        let client: Client = Client::builder()
            .with_primary_node(&iota_endpoint, None)
            .context("unable to create a client for the provided endpoint")?
            .finish()
            .await
            .context("unable to create a client for the provided endpoint")?;

        clients.push((SMR_NETWORK_NAME, client));
    }

    let custom_hrp = env::var(IOTA_CUSTOM_NETWORK_NAME).ok();
    let custom_endpoint = env::var(IOTA_CUSTOM_NODE_ENDPOINT).ok();
    if let (Some(custom_hrp), Some(custom_endpoint)) = (custom_hrp, custom_endpoint) {
        let client: Client = Client::builder()
            .with_primary_node(&custom_endpoint, None)
            .expect("unable to create a client for the provided endpoint")
            .finish()
            .await
            .expect("unable to create a client for the provided endpoint");

        let static_str: &'static str = Box::leak(custom_hrp.to_owned().into_boxed_str());
        clients.push((static_str, client));
    }

    if clients.is_empty() {
        bail!(
            "No clients were created. Make sure you provide a configuration for at least one network"
        )
    }

    let mut resolver = Resolver::<IotaDocument>::new();
    resolver.attach_multiple_iota_handlers(clients);

    Ok(Arc::new(resolver))
}

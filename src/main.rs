// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use uni_resolver_driver_iota::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    let listener = TcpListener::bind("0.0.0.0:8080")
        .await
        .context("failed to bind to port 8080")?;

    Server::default().run(listener).await?;
    Ok(())
}

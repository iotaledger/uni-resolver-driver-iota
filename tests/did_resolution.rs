// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod common;

use common::TestServer;
use reqwest::Client;
use uni_resolver_driver_iota::ResolutionResponse;

#[tokio::test]
// Creates and fetches a DID document using the resolver server.
async fn did_resolution() -> anyhow::Result<()> {
    let mut server = TestServer::new().await?;
    let target_doc = server.create_did().await?;

    let client = Client::default();
    let res = client
        .get(format!(
            "http://{}/1.0/identifiers/{}",
            server.address(),
            target_doc.id()
        ))
        .send()
        .await?;
    assert!(res.status().is_success());

    let fetched_doc = res.json::<ResolutionResponse>().await?.did_document;
    assert_eq!(target_doc.core_document(), &fetched_doc);

    Ok(())
}

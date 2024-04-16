// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod common;

use common::TestServer;
use reqwest::Client;
use uni_resolver_driver_iota::ResolutionResponse;

#[tokio::test]
// Creates and fetches a DID document using the resolver server.
async fn did_resolution_works() -> anyhow::Result<()> {
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

#[tokio::test]
// Attemps to fetch a non-existent DID document to get a 404.
async fn missing_did_resolution_fails_with_404() -> anyhow::Result<()> {
    let server = TestServer::new().await?;

    let client = Client::default();
    let res = client
        .get(format!(
            "http://{}/1.0/identifiers/{}",
            server.address(),
            "did:iota:snd:0x4bbd377239914fced5c1207a28443064050e880a1234858904e0ce31a5a9768c"
        ))
        .send()
        .await?;
    assert_eq!(res.status().as_u16(), 404);

    Ok(())
}

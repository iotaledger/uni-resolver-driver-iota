// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod common;

use common::TestServer;
use reqwest::Client;
use uni_resolver_driver_iota::{DidResolutionError, DidResolutionErrorKind, ResolutionResponse};

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

// Attempts to fetch a non-existent DID document to get a 404.
#[tokio::test]
async fn missing_did_resolution_fails_with_404() -> anyhow::Result<()> {
    let server = TestServer::new().await?;
    let client = Client::default();
    let did = "did:iota:devnet:0xf4d6f08f5a1b80dd578da7dc1b49c886d580acd4cf7d48119dfeb82b538ad88b";

    let res = client
        .get(format!("http://{}/1.0/identifiers/{did}", server.address(),))
        .send()
        .await?;

    assert_eq!(res.status().as_u16(), 404);

    Ok(())
}

// Attempts to fetch a DID Document for a non configured network fails with 500.
#[tokio::test]
async fn unknown_network_fails_with_400() -> anyhow::Result<()> {
    let server = TestServer::new().await?;
    let client = Client::default();
    let did = "did:iota:unknown:0xf4d6f08f5a1b80dd578da7dc1b49c886d580acd4cf7d48119dfeb82b538ad88b";

    let res = client
        .get(format!("http://{}/1.0/identifiers/{did}", server.address(),))
        .send()
        .await?;

    assert_eq!(res.status().as_u16(), 400);

    let err_msg = res.text().await?;
    let expected_err = anyhow::Error::from(DidResolutionError {
        did: did.parse().unwrap(),
        kind: DidResolutionErrorKind::UnknownNetwork("unknown".to_owned()),
    });
    assert_eq!(err_msg, format!("{expected_err:#}"));

    Ok(())
}

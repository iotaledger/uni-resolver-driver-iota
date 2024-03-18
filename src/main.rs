// Copyright 2020-2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use identity_iota::{
    iota::{IotaDID, IotaDocument},
    resolver::Resolver,
};
use iota_sdk::client::Client;
use std::{collections::HashMap, env, sync::Arc};
use warp::{
    http::{Response, StatusCode},
    reject::Rejection,
    Filter,
};

const IOTA_NETWORK_NAME: &str = "iota";
const SMR_NETWORK_NAME: &str = "smr";
const IOTA_NODE_ENDPOINT: &str = "IOTA_NODE_ENDPOINT";
const SMR_NODE_ENDPOINT: &str = "SMR_NODE_ENDPOINT";
const IOTA_CUSTOM_NETWORK_NAME: &str = "IOTA_CUSTOM_NETWORK_NAME";
const IOTA_CUSTOM_NODE_ENDPOINT: &str = "IOTA_CUSTOM_NODE_ENDPOINT";

#[tokio::main]
async fn main() {
    let mut clients = vec![];
    let envs = env::vars().collect::<HashMap<String, String>>();

    if let Some(iota_endpoint) = envs.get(IOTA_NODE_ENDPOINT) {
        let client: Client = Client::builder()
            .with_primary_node(iota_endpoint, None)
            .expect("unable to create a client for the provided endpoint")
            .finish()
            .await
            .expect("unable to create a client for the provided endpoint");

        clients.push((IOTA_NETWORK_NAME, client));
    }

    if let Some(iota_endpoint) = envs.get(SMR_NODE_ENDPOINT) {
        let client: Client = Client::builder()
            .with_primary_node(iota_endpoint, None)
            .expect("unable to create a client for the provided endpoint")
            .finish()
            .await
            .expect("unable to create a client for the provided endpoint");

        clients.push((SMR_NETWORK_NAME, client));
    }

    let custom_hrp = envs.get(IOTA_CUSTOM_NETWORK_NAME);
    let custom_endpoint = envs.get(IOTA_CUSTOM_NODE_ENDPOINT);
    if let (Some(custom_hrp), Some(custom_endpoint)) = (custom_hrp, custom_endpoint) {
        let client: Client = Client::builder()
            .with_primary_node(custom_endpoint, None)
            .expect("unable to create a client for the provided endpoint")
            .finish()
            .await
            .expect("unable to create a client for the provided endpoint");

        let static_str: &'static str = Box::leak(custom_hrp.to_owned().into_boxed_str());
        clients.push((static_str, client));
    }

    if clients.is_empty() {
        panic!(
            "No clients were created. Make sure you provide a configuration for at least one network"
        )
    }

    let mut resolver = Resolver::<IotaDocument>::new();
    resolver.attach_multiple_iota_handlers(clients);
    let resolver = Arc::new(resolver);

    let routes = warp::get()
        .and(warp::path("1.0"))
        .and(warp::path("identifiers"))
        .and(warp::path::param())
        .and(warp::path::end())
        .map(move |bla: String| ResolverWrapper {
            resolver: resolver.clone(),
            did: bla,
        })
        .and_then(move |wrapper: ResolverWrapper| async move {
            let did: IotaDID = match IotaDID::parse(wrapper.did) {
                Ok(did) => did,
                Err(_) => {
                    return Ok::<Response<String>, Rejection>(
                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body("invalid input!".to_string())
                            .unwrap(),
                    );
                }
            };

            let resolved: IotaDocument = match wrapper.resolver.resolve(&did).await {
                Ok(doc) => doc,
                Err(_) => {
                    return Ok::<Response<String>, Rejection>(
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body("error!".to_string())
                            .unwrap(),
                    );
                }
            };
            let doc = resolved.core_document().to_string();
            let metadata = resolved.metadata.to_string();
            let body = format!(r#"{{"didDocument": {doc}, "didResolutionMetadata": {metadata}}}"#,);
            Ok::<Response<String>, Rejection>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "did+json")
                    .body(body)
                    .unwrap(),
            )
        });

    println!("starting server...");
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}

struct ResolverWrapper {
    resolver: Arc<Resolver<IotaDocument>>,
    did: String,
}

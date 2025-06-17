// Copyright 2020-2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use fastcrypto::ed25519::Ed25519PublicKey;
use fastcrypto::traits::ToFromBytes;
use identity_iota::iota::IotaDocument;
use identity_iota::storage::{JwkDocumentExt, Storage, StorageSigner};
use identity_iota::verification::jwk::Jwk;
use identity_iota::verification::jws::JwsAlgorithm;
use identity_iota::verification::MethodScope;
use identity_iota_core::rebased::client::{IdentityClient, IdentityClientReadOnly};
use identity_storage::{JwkMemStore, JwkStorage, KeyId, KeyIdMemstore, KeyType};
use iota_sdk::types::base_types::IotaAddress;
use iota_sdk::{IotaClient, IotaClientBuilder};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tracing_subscriber::EnvFilter;
use uni_resolver_driver_iota::{Resolver, Server};

pub type MemStorage = Storage<JwkMemStore, KeyIdMemstore>;

pub const DEVNET_FAUCET_ENDPOINT: &str = "https://faucet.devnet.iota.cafe/v1/gas";

static TRACING_LOCK: OnceLock<()> = OnceLock::new();

fn init_tracing() {
    TRACING_LOCK.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .compact()
            .init()
    });
}

pub struct TestServer {
    pub _handle: JoinHandle<anyhow::Result<()>>,
    client: IdentityClientReadOnly,
    address: SocketAddr,
    storage: Arc<MemStorage>,
}

impl TestServer {
    pub async fn new() -> anyhow::Result<Self> {
        init_tracing();
        dotenvy::dotenv().ok();

        let storage = Arc::new(Storage::new(JwkMemStore::new(), KeyIdMemstore::new()));

        let client: IotaClient = IotaClientBuilder::default().build_devnet().await?;
        let client = IdentityClientReadOnly::new(client).await?;

        let server = Server::default().with_resolver(Resolver::new([("devnet".to_owned(), client.clone())]));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("failed to bind to random port")?;
        let address = listener.local_addr()?;
        println!("Server running on: {}", address);
        Ok(Self {
            client,
            storage,
            address,
            _handle: tokio::spawn(server.run(listener)),
        })
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub async fn create_did(&mut self) -> anyhow::Result<IotaDocument> {
        create_did(&self.client, &self.storage).await.map(|(_, doc, _)| doc)
    }
}

/// Creates a DID Document and publishes it in a new Alias Output.
///
/// Its functionality is equivalent to the "create DID" example
/// and exists for convenient calling from the other examples.
pub async fn create_did(
    client: &IdentityClientReadOnly,
    storage: &Arc<MemStorage>,
) -> anyhow::Result<(IotaAddress, IotaDocument, String)> {
    let (address, key_id, pub_key_jwk) = get_address(storage).await.context("failed to get address with funds")?;

    // Fund the account
    request_faucet_funds(address).await?;

    let signer = StorageSigner::new(storage, key_id.clone(), pub_key_jwk);

    let identity_client = IdentityClient::new(client.clone(), signer).await?;

    let network_name = client.network();
    let (document, fragment): (IotaDocument, String) = create_did_document(network_name, storage).await?;

    let document = identity_client
        .publish_did_document(document)
        .build_and_execute(&identity_client)
        .await?
        .output;

    Ok((address, document, fragment))
}

/// Creates an example DID document with the given `network_name`.
///
/// Its functionality is equivalent to the "create DID" example
/// and exists for convenient calling from the other examples.
pub async fn create_did_document(
    network_name: &str,
    storage: &Arc<MemStorage>,
) -> anyhow::Result<(IotaDocument, String)> {
    let mut document: IotaDocument = IotaDocument::new(&network_name.try_into()?);

    let fragment: String = document
        .generate_method(
            storage,
            JwkMemStore::ED25519_KEY_TYPE,
            JwsAlgorithm::EdDSA,
            None,
            MethodScope::VerificationMethod,
        )
        .await?;

    Ok((document, fragment))
}

/// Initializes the [`Storage`] and generates a new address.
pub async fn get_address(storage: &Arc<MemStorage>) -> anyhow::Result<(IotaAddress, KeyId, Jwk)> {
    let generated_key = storage
        .key_storage()
        .generate(KeyType::new("Ed25519"), JwsAlgorithm::EdDSA)
        .await?;

    let key_id = generated_key.key_id;

    let pub_key_jwt = generated_key.jwk.to_public().expect("should not fail");
    let pub_key_bytes = pub_key_jwt
        .try_okp_params()
        .map(|key| identity_iota::verification::jwu::decode_b64(key.x.clone()).expect("should be decodable"))?;

    let address = Ed25519PublicKey::from_bytes(&pub_key_bytes)?;

    Ok((IotaAddress::from(&address), key_id, pub_key_jwt))
}

/// Requests funds from the faucet for the given `address`.
async fn request_faucet_funds(address: IotaAddress) -> anyhow::Result<()> {
    let output = Command::new("iota")
        .arg("client")
        .arg("faucet")
        .arg("--address")
        .arg(address.to_string())
        .arg("--url")
        .arg(DEVNET_FAUCET_ENDPOINT)
        .arg("--json")
        .output()
        .await
        .context("Failed to execute command")?;

    // Check if the output is success
    if !output.status.success() {
        anyhow::bail!(
            "Failed to request funds from faucet: {}",
            std::str::from_utf8(&output.stderr).unwrap()
        );
    }

    Ok(())
}

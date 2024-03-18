<!-- This READM is based on the BEST-README-Template (https://github.com/othneildrew/Best-README-Template) -->

<div id="top"></div>
<!--
*** Thanks for checking out the Best-README-Template. If you have a suggestion
*** that would make this better, please fork the repo and create a pull request
*** or simply open an issue with the tag "enhancement".
*** Don't forget to give the project a star!
*** Thanks again! Now go create something AMAZING! :D
-->

<!-- PROJECT SHIELDS -->

<!--
*** I'm using markdown "reference style" links for readability.
*** Reference links are enclosed in brackets [ ] instead of parentheses ( ).
*** See the bottom of this document for the declaration of the reference variables
*** for contributors-url, forks-url, etc. This is an optional, concise syntax you may use.
*** https://www.markdownguide.org/basic-syntax/#reference-style-links
-->

<!-- [![Contributors][contributors-shield]][contributors-url] -->

<!-- [![Forks][forks-shield]][forks-url] -->

<!-- [![Stargazers][stars-shield]][stars-url] -->

[![Issues][issues-shield]][issues-url]
[![Apache 2.0 license][license-shield]][license-url]
[![Discord][discord-shield]][discord-url]
[![StackExchange][stackexchange-shield]][stackexchange-url]

<!-- Add additional Badges. Some examples >
![Format Badge](https://github.com/iotaledger/template/workflows/Format/badge.svg "Format Badge")
![Audit Badge](https://github.com/iotaledger/template/workflows/Audit/badge.svg "Audit Badge")
![Clippy Badge](https://github.com/iotaledger/template/workflows/Clippy/badge.svg "Clippy Badge")
![BuildBadge](https://github.com/iotaledger/template/workflows/Build/badge.svg "Build Badge")
![Test Badge](https://github.com/iotaledger/template/workflows/Test/badge.svg "Test Badge")
![Coverage Badge](https://coveralls.io/repos/github/iotaledger/template/badge.svg "Coverage Badge")


<!-- PROJECT LOGO -->

# Universal Resolver Driver for IOTA

This is a driver implementation of [Universal Resolver](https://github.com/decentralized-identity/universal-resolver/) for the `did:iota` identifier.

## Specifications

[IOTA DID Method Specification v1.0](https://wiki.iota.org/identity.rs/references/specifications/iota-did-method-spec/)

## Example DIDs

:warning: TODO: publish DID(s) on mainnet and add them here.

## Build and Run (Docker)

```bash
docker build . -t iotaledger/uni-resolver-driver-iota
```

```bash
docker run -p 8080:8080 iotaledger/uni-resolver-driver-iota
```

```bash
 curl -X GET localhost:8080/1.0/identifiers/<did>
```

## Build and Run (Rust)

```bash
cargo run --release
```

## Driver Environment Variables

`IOTA_NODE_ENDPOINT` Endpoint for the `iota` network.
`IOTA_SMR_NODE_ENDPOINT` Endpoint for the `smr` network.
`IOTA_CUSTOM_NETWORK_NAME` HRP a of custom network.
`IOTA_CUSTOM_NODE_ENDPOINT` Endpoint for the custom network.

Note: at least one network must be configured.

## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

If you have a suggestion that would make this better, please fork the repo and create a pull request. You can also simply open an issue with the tag "enhancement".
Don't forget to give the project a star! Thanks again!

1. Fork the Project
1. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
1. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
1. Push to the Branch (`git push origin feature/AmazingFeature`)
1. Open a Pull Request

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- LICENSE -->

## License

Distributed under the Apache License. See `LICENSE` for more information.

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->

<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->

[discord-shield]: https://img.shields.io/badge/Discord-9cf.svg?style=for-the-badge&logo=discord
[discord-url]: https://discord.iota.org
[issues-shield]: https://img.shields.io/github/issues/iotaledger/template.svg?style=for-the-badge
[issues-url]: https://github.com/iotaledger/template/issues
[license-shield]: https://img.shields.io/github/license/iotaledger/template.svg?style=for-the-badge
[license-url]: https://github.com/iotaledger/template/blob/main/LICENSE
[stackexchange-shield]: https://img.shields.io/badge/StackExchange-9cf.svg?style=for-the-badge&logo=stackexchange
[stackexchange-url]: https://iota.stackexchange.com

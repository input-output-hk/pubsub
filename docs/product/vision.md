# Vision & Problem Statement

## Mission

To provide the Cardano ecosystem (Cardano, Midnight, Partner Chains) with the open-source tools and protocols necessary to build and participate in a unified, secure, and decentralized communication network.

## Vision

To create a decentralized, trustless, and censorship-resistant communication protocol available across Cardano, Midnight and PartnerChains, transforming them into a truly interactive and interoperable multi-chain ecosystem.

## Problem

The IOG ecosystem lacks a native, unified, and secure communication layer. This forces projects on Cardano and Midnight to plan for a future reliant on insecure, centralized Web2 platforms (Discord, X) or incompatible Web3 solutions.

### Core Problems

| Problem | Impact |
|---------|--------|
| **Cross-Chain Fragmentation** | Without a unified standard, each chain develops isolated solutions, creating a disjointed user experience and preventing seamless interoperability |
| **Significant Security Risks** | Communicating critical financial information (e.g., DeFi liquidations, governance votes) through untrusted channels exposes all users to phishing, scams, and censorship |
| **Incompatible Technology** | Existing pub/sub solutions like libp2p and XMTP are not compatible with Cardano's native mini-protocol network stack, making them inefficient and burdensome for SPOs to run |
| **Stifled Innovation** | The absence of a real-time, cross-chain communication primitive prevents developers from building sophisticated applications that can interact seamlessly across Cardano and Midnight |

## Solution

Agora provides a unified, phased solution designed specifically for the IOG ecosystem:

### Unified Standard
A single protocol for Cardano, Midnight, and partner chains, ensuring interoperability from day one.

### Decentralized Identity
Use of Hyperledger Identus DIDs as the core identifier, enabling true cross-chain communication and identity management.

### Native Network Integration
Prioritizing native network integration using Cardano's core protocols (with a mini-protocol being the leading approach under evaluation) to ensure optimal performance, security, and low operational overhead for SPOs.

### Data Availability Layer
Incorporating a DA layer to provide censorship resistance and verifiable proof of message delivery for all subscribers.

### Phased Decentralization
Launching with Beacon, a centralized backend for Midnight, to meet immediate deadlines, while building the fully decentralized Agora network for the long term.

### Private & Secure
End-to-end encryption for all messages, with research into Trusted Execution Environments (TEEs) to enable truly private communication channels.

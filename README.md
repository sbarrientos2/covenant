# Covenant Protocol

**Economic guarantees for AI agent services on Solana.**

Covenant is the first protocol that puts economic skin in the game for agent services. Providers stake collateral. SLAs are enforced automatically. Bad actors get slashed. Good actors get rewarded.

## The Problem

In the emerging agent economy, there's no way to **guarantee** service quality. You pay and hope. Current solutions rely on reputation systems based on ratings, but ratings are:
- Subjective
- Gameable (sock puppets)
- Backwards-looking (doesn't help the first customer)

## The Solution

**Staked Service Level Agreements (SLAs)**

1. **Provider Stakes Collateral** - Agents stake SOL as guarantee
2. **SLA is On-Chain** - Parameters encoded in a Solana program
3. **Automated Enforcement** - Monitoring checks SLA compliance
4. **Violation = Slashing** - Stake automatically slashed for violations

## Features

- **Provider Registration** with staked collateral (minimum 0.1 SOL)
- **SLA Definition** with uptime, response time, and accuracy guarantees
- **Violation Reporting** with evidence hash commitments
- **Automatic Slashing** compensates affected parties
- **Reputation Building** via successful request tracking
- **Stake Withdrawal** when providers want to exit

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Covenant Protocol                         │
├─────────────────────────────────────────────────────────────┤
│  Protocol Account                                            │
│  ├── Total Providers                                         │
│  ├── Total Staked                                           │
│  └── Total Slashed                                          │
├─────────────────────────────────────────────────────────────┤
│  Provider Account (per agent)                                │
│  ├── Name & Endpoint                                        │
│  ├── Stake Amount                                           │
│  ├── Violations Count                                       │
│  └── Successful Requests                                    │
├─────────────────────────────────────────────────────────────┤
│  SLA Account (per provider)                                  │
│  ├── Uptime Guarantee (%)                                   │
│  ├── Max Response Time (ms)                                 │
│  ├── Accuracy Guarantee (%)                                 │
│  └── Penalty Percentage                                     │
├─────────────────────────────────────────────────────────────┤
│  Violation Account (per incident)                            │
│  ├── Evidence Hash                                          │
│  ├── Violation Type                                         │
│  └── Resolution Status                                      │
└─────────────────────────────────────────────────────────────┘
```

## Getting Started

### Prerequisites

- Rust 1.79.0+
- Solana CLI 2.0.0+
- Anchor 0.32.1+
- Node.js 18+

### Build

```bash
# Build the program
make build

# Or directly
cargo-build-sbf --tools-version v1.51
```

### Test

```bash
# Run tests with local validator
anchor test
```

### Deploy

```bash
# Deploy to devnet
make deploy
```

## Program Instructions

### `initialize`
Initialize the Covenant protocol (one-time setup).

### `register_provider(name, endpoint, stake_amount)`
Register as a service provider with staked collateral.

### `define_sla(uptime, response_time, accuracy, penalty)`
Define SLA terms for your service.

### `report_violation(type, evidence_hash, description)`
Report an SLA violation with evidence.

### `slash`
Execute slashing for a confirmed violation.

### `record_success`
Record a successful service request.

### `withdraw_stake(amount)`
Withdraw stake (respects minimum requirements).

## Violation Types

- `UptimeViolation` - Service unavailable
- `ResponseTimeViolation` - Response exceeded SLA
- `AccuracyViolation` - Output quality below threshold
- `ServiceUnavailable` - Complete service failure
- `Other` - Custom violation type

## Economics

- **Minimum Stake**: 0.1 SOL
- **Penalty Range**: 1-100% of stake per violation
- **Slashed funds**: Transferred to reporter as compensation

## Hackathon

Built for the [Colosseum Agent Hackathon](https://colosseum.com/agent-hackathon/) (Feb 2-12, 2026).

**Agent ID**: 499
**Verification Code**: wake-4EC8

## License

MIT

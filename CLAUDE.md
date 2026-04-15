# me-rs — Matching Engine (Rust)

## Project Overview

This is a **crypto spot exchange matching engine** written in Rust. The ME is the single source of truth for all market rules and order matching. It is deterministic, single-threaded per symbol, and outputs immutable Trade events consumed by downstream systems (ledger, balance, reporting).

## Architecture Principles

- **Determinism**: identical input event sequences must produce identical output — no randomness, no wall-clock side effects in matching logic
- **Fairness**: price priority > time priority (FIFO per price level)
- **Auditability**: all state must be fully replayable from the event log
- **Separation of concerns**: the ME does not calculate balances; it only emits Trade events

## Domain Rules

### Market Contract (per symbol)
- Base / Quote assets
- Price tick size and quantity step size
- Minimum notional
- Fee model (maker / taker)

Any order violating the market contract must be **rejected before entering the matching loop**.

### Supported Order Types (Phase 1, spot)
| Type | TIF | Notes |
|------|-----|-------|
| Limit | GTC, IOC, FOK, GTD/Day | May rest on book (GTC) |
| Market | IOC | Price-protected; never rests on book |

- PostOnly is an execution constraint, not an order type
- Market order = IOC + price protection; partial fill + cancel remainder is valid

### Funds & Risk Boundary
- Orders enter the ME **only after funds are frozen** by the upstream risk layer
- Buy order → quote asset frozen; Sell order → base asset frozen
- Settlement is derived strictly from Trade events — balances must never go negative

### Self-Trade Prevention (STP)
- Mandatory for all symbols
- Minimum strategy: **Cancel New** (cancel the incoming order, preserve the resting order)
- Applied after price/time match, before Trade event generation

### ME Output Events
| Event | When |
|-------|------|
| `OrderAck` | Order accepted into the book |
| `OrderReject` | Order rejected before matching |
| `Trade` | A fill occurred |
| `OrderUpdate` | Open / PartialFill / Filled / Cancelled |

Every `Trade` event must carry: `trade_id`, `symbol`, `price`, `quantity`, maker/taker order & user IDs, `taker_side`, fee indicators, `engine_sequence`, `timestamp`.

## Reasoning Order

When analyzing a problem or implementing a feature, follow this order:

1. **Business rules and market semantics** — does this violate exchange contracts?
2. **Deterministic matching behavior** — does this preserve replay correctness?
3. **State transitions and invariants** — what state changes, what must hold before/after?
4. **Data structures and algorithms** — what is the right abstraction?
5. **Rust-specific implementation tradeoffs** — ownership, lifetimes, zero-copy, performance

Prioritize **correctness, determinism, and auditability** over premature optimization.

## Rust Standards

- Use `#[must_use]` on result-returning functions where ignoring the value is a bug
- Prefer `enum`-based state machines over boolean flags for order lifecycle
- Avoid `unwrap` / `expect` in matching-critical paths; propagate errors explicitly
- No `unsafe` without a documented safety invariant in a comment block

## Documentation

All business documentation (specs, design docs, ADRs, protocol definitions) lives under `doc/`. Before implementing any feature, check `doc/` for relevant specs. When in doubt about a business rule, treat `doc/` as the authoritative reference.

## Clarification Policy

If business requirements or matching semantics are ambiguous, **ask a clarifying question** rather than guessing. An incorrect assumption in matching logic is harder to detect than a delayed question.

# Changelog

## [2.0.0](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/compare/v1.1.2...v2.0.0) (2026-03-11)


### ⚠ BREAKING CHANGES

* internals rewritten — mutex+vec fanout replaced with tokio::sync::broadcast, tungstenite upgraded to 0.28, EventEmitter removed, tracing replaces println, TCP backlog and connection handling overhauled. Wire protocol unchanged.

### Features

* broadcast channel refactor, tracing, connection scaling ([d1b0c41](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/d1b0c41d42eb649d7b4001d5e37e764def36e030))
* configurable buffer capacity, oversize error logging, JoinHandle fix, docs ([84fbff0](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/84fbff07dc0db1e9e8463744e87b9a629598577b))
* replace println!/eprintln! with tracing ([0545a2e](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/0545a2e1aeb668bb5e1424982fbc9cb9f62ef3e2))


### Bug Fixes

* send OVOS "connected" greeting on WebSocket connect ([a783664](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/a783664434b4998f73bc80af7506f41e78eb44c4))
* use explicit TCP backlog to prevent connection rejections under load ([3fdb754](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/3fdb75487bd3b12413572447c83253302a00ac5b))


### Performance Improvements

* batch websocket writes, use Utf8Bytes, upgrade tungstenite 0.28 ([7c838ee](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/7c838eec5786889fed7e444bc1741868b2dd1e3f))
* remove EventEmitter, add TCP_NODELAY, enforce max_msg_size via websocket config ([14c1a97](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/14c1a97548270502764f591d384066352288da8f))
* replace mutex+vec fanout with bounded broadcast channel ([380ed17](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/380ed1761d3b414eb0f4b4d28930c6e935e6cc58))

## [1.1.2](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/compare/v1.1.1...v1.1.2) (2026-01-12)


### Bug Fixes

* update tungsten to resolve high-severity vulnerability ([#21](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/issues/21)) ([ef67a46](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/ef67a46448c48b534719b1756fb7c8c286ed48ee))

## [1.1.1](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/compare/v1.1.0...v1.1.1) (2025-07-22)


### Bug Fixes

* **docker:** proper multi-platform manifest ([ee794af](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/ee794af9bbf5cabe68040c9c2ff1151b1ffbfd7d))

## [1.1.0](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/compare/v1.0.1...v1.1.0) (2024-12-13)


### Features

* **config:** Env overrides for all standard config values ([#15](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/issues/15)) ([1e4a5e2](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/1e4a5e2cd69767265e8f21179d9191734974016a))

## [1.0.1](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/compare/v1.0.0...v1.0.1) (2024-12-12)


### Bug Fixes

* automated release workflow ([eef8355](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/commit/eef8355d68f1e2dee95b8989b2b42d619c2c5ee5))

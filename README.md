# OBS DeepFilterNet Noise Cancelling

OBS Studio 用の音声フィルタプラグイン。DeepFilterNet3 を CPU で推論し、マイクのノイズを抑制する。Windows と macOS の両方で動作する。

> **ステータス:** 設計中。実装はこれから。

## 特徴

- **GPU 負荷ゼロ** — 推論は CPU のみ。ゲーム中の GPU を奪わない
- **高音質** — OBS 標準の RNNoise より新しい DeepFilterNet3 を使用
- **低遅延** — プラグイン由来の遅延は約 40 ms
- **単体で完結** — モデルをバイナリに埋め込み。実行時の外部ファイル不要
- **クロスプラットフォーム** — Windows x64 / macOS Universal（arm64 + x86_64）
- **対応バージョン** — OBS Studio 32.2.1 以降 / macOS 13 以降

## ドキュメント

- [設計書](docs/DESIGN.md)

## ビルド

GitHub Actions で Windows / macOS のビルドとパッケージを行う。ローカルビルド手順は設計書の「9.4 開発者向けビルド手順」を参照。

## ライセンス

- プラグイン本体: GPL-2.0-or-later
- DeepFilterNet / libDF / モデル: MIT OR Apache-2.0

DeepFilterNet の著作権表示およびライセンス全文は `LICENSES/` に同梱する。

# OBS DeepFilterNet Noise Cancelling

OBS Studio 用の音声フィルタプラグイン。DeepFilterNet3 を CPU で推論し、マイクのノイズを抑制する。Windows と macOS の両方で動作する。

> **ステータス:** 初期実装済み。OBS での動作確認中。

## 特徴

- **GPU 負荷ゼロ** — 推論は CPU のみ。ゲーム中の GPU を奪わない
- **高音質** — OBS 標準の RNNoise より新しい DeepFilterNet3 を使用
- **低遅延** — プラグイン由来の遅延は約 40 ms
- **単体で完結** — モデルをバイナリに埋め込み。実行時の外部ファイル不要
- **クロスプラットフォーム** — Windows x64 / macOS Universal（arm64 + x86_64）
- **対応バージョン** — OBS Studio 32.2.1 以降 / macOS 13 以降

## インストール

インストーラはありません。Windows は zip、macOS は tar.xz を手動で配置してください。

### Windows（通常インストール）

zip を `C:\ProgramData\obs-studio\` に展開します。以下のように配置されます。

```text
C:\ProgramData\obs-studio\plugins\obs-deepfilter-noise\
├── obs-deepfilter-noise.dll
├── df_ffi.dll
└── data\locale\...
```

### Windows（ポータブル）

zip をポータブル OBS のルートフォルダに展開します。以下のように配置されます。

```text
<portable>\plugins\obs-deepfilter-noise\
├── obs-deepfilter-noise.dll
├── df_ffi.dll
└── data\locale\...
```

> `obs-plugins\64bit` や `bin\64bit` は Legacy 形式です。OBS 32.2.1 以降では使用しないでください。

### macOS

tar.xz を展開し、`obs-deepfilter-noise.plugin` を以下に置きます。

```text
~/Library/Application Support/obs-studio/plugins/obs-deepfilter-noise.plugin
```

## 使い方

1. OBS でマイクなどの音声ソースを選択
2. 右下の **「フィルタ」** をクリック
3. 左下の **「＋」** をクリック
4. **「音声フィルタ」** から **DeepFilterNet Noise Cancelling** を選択

フィルタは音声ソースごとに追加します。「ソースの追加」メニューや OBS の設定画面には表示されません。

## ドキュメント

- [設計書](docs/DESIGN.md)

## ビルド

GitHub Actions の手動実行で Windows / macOS のビルドを行う。ローカルビルド手順は設計書の「7.4 開発者向けビルド手順」を参照。

## ライセンス

- プラグイン本体: GPL-2.0-or-later
- DeepFilterNet / libDF / モデル: MIT OR Apache-2.0

DeepFilterNet の著作権表示およびライセンス全文は `LICENSES/` に同梱する。

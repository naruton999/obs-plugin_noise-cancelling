# obs-plugin_noise-cancelling

OBS Studio 用の音声フィルタプラグイン。DeepFilterNet3（DFN3）でマイクの定常ノイズを抑え、声を明瞭に残します。

- 表示名: **DeepFilterNet ノイズキャンセリング**
- フィルタ ID: `obs_plugin_noise_cancelling_filter`
- 対象 OBS: 31.1.1 API でビルド。32.2.1 以上で動作確認
- 対象 OS: **Windows x64（最優先）** / macOS arm64（ベストエフォート）
- ライセンス: GPL-3.0-or-later

> 「ノイズキャンセリング」は製品名です。技術的にはノイズ抑制（noise suppression）であり、逆位相による能動消音ではありません。

## 特徴

| 機能 | 内容 |
|---|---|
| モデル選択 | LL（低遅延） / 標準（高品質）をプルダウンで切替 |
| サンプルレート | 48 kHz / 44.1 kHz（44.1 kHz は内部で 48 kHz にリサンプル） |
| 遅延モード | 2 hop（既定・安定性優先） / 1 hop（低遅延） |
| 低減量 | 0〜100 dB |
| ミックス | 0〜100%（0% = 遅延乾音） |
| ポストフィルタ | β 0〜0.05 |
| 上級者向けしきい値 | Min / ERB / DF |
| クリック音除去（試作） | 条件付き 7 点メディアンでキーボードの数サンプルクリックを抑える |
| 声の保護（試作） | DFN の LSNR が高い時に乾音をブレンドし、「ん〜」の抑圧を緩和 |
| プリセット | `standard` / `gentle` / `strong` / `speech` / `hybrid`（試作） |
| ステレオ | Mid を処理し、Side を遅延保持して再合成 |
| バイパス | 遅延維持バイパス |
| 保存 | OBS のシーンコレクションに保存 |

## 必要環境

- OBS Studio 32.2.1 以上（ビルドは 31.1.1 API）
- Windows x64 または macOS arm64
- モデルファイル 2 つ（合計約 43 MB）をプラグインの `models/` に配置

## インストール（Windows）

### OBS 33 以降（新配置）

1. `obs-plugin_noise-cancelling-windows-x64.zip` を展開する。
2. `obs-plugin_noise-cancelling` フォルダを
   `C:\ProgramData\obs-studio\plugins\` にコピーする。
3. OBS を再起動する。

```text
C:\ProgramData\obs-studio\plugins\obs-plugin_noise-cancelling\
├─ obs-plugin_noise-cancelling.dll
└─ data\
   ├─ presets\*.toml
   └─ models\*.tar.gz
```

### OBS 32.x / 旧配置

| 種類 | コピー先 |
|---|---|
| DLL | `C:\Program Files\obs-studio\obs-plugins\64bit\` |
| data | `C:\Program Files\obs-studio\data\obs-plugins\obs-plugin_noise-cancelling\` |

管理者権限が必要です。アンインストールはコピーしたファイルとフォルダの削除のみです。

### 署名について

v0.1.0 の Windows 配布物は未署名です。SmartScreen の警告が出る場合は、zip のプロパティで「ブロックの解除」を行ってください。

## インストール（macOS arm64・ベストエフォート）

1. `obs-plugin_noise-cancelling.plugin` を
   `~/Library/Application Support/obs-studio/plugins/` にコピーする。
2. OBS を再起動する。

ad-hoc 署名済みです。配布用に Developer ID で署名する場合:

```bash
CODESIGN_IDENTITY="Developer ID Application: ..." ./build-aux/package-macos.sh
```

macOS は v0.1.0 ではベストエフォートです。Windows での動作確認を優先してください。

## 使い方

1. 音声ソース（マイクなど）の「フィルタ」を開く。
2. 「＋」から **DeepFilterNet ノイズキャンセリング** を追加する。
3. モデルを選ぶ（既定は LL）。
4. プリセットを選び、「プリセットを適用」を押す。
5. 必要に応じて低減量、ミックス、ポストフィルタを調整する。

### モデル

| モデル | ファイル | サイズ | 遅延の目安 | 特徴 |
|---|---|---|---|---|
| LL（低遅延） | `DeepFilterNet3_ll_onnx.tar.gz` | 36,359,660 bytes | 約 10 ms | 低遅延。CPU/メモリは大きめ |
| 標準（高品質） | `DeepFilterNet3_onnx.tar.gz` | 7,983,136 bytes | 約 30 ms | 高品質。低 CPU/メモリ |

モデルは外部ファイルです。`models/` から実行時に読み込みます。モデル切替には数秒かかり、切替中は乾音になります。

### 遅延モード

| モード | ワーカーバッファ | 内部遅延の目安（LL / 48 kHz） |
|---|---|---|
| 2 hop（既定） | 20 ms | 約 30 ms + OBS 約 21 ms |
| 1 hop（低遅延） | 10 ms | 約 20 ms + OBS 約 21 ms |

44.1 kHz ではリサンプル遅延が加算されます。標準モデルは LL より約 20 ms 遅くなります。実際の値は UI の「概算遅延」と、ループバックでの実測で確認してください。

### プリセット

`data/presets/*.toml`（macOS は `.plugin/Contents/Resources/presets/`）を起動時に列挙します。新しいプリセットは TOML を置いてプロパティを開き直すと一覧に追加されます。

| ファイル | 低減量 | PF β | 用途 |
|---|---|---|---|
| `standard.toml` | 100 dB | 0.0 | 標準。最大低減 |
| `gentle.toml` | 12 dB | 0.0 | 控えめ。環境音を残す |
| `strong.toml` | 100 dB | 0.02 | 強め。騒がしい環境 |
| `speech.toml` | 24 dB | 0.01 | 音声向けのバランス |
| `hybrid.toml` | 24 dB | 0.0 | **試作**。クリック除去＋声保護、mix 85% |

プリセットは DSP パラメータのみを変更します。モデルと低遅延モードは変更しません。

### ハイブリッド試作（クリック除去＋声保護）

`hybrid` プリセットは、DeepFilterNet の前に軽いクリック除去を入れ、声と判定された時に乾音を少しブレンドします。

- **クリック音除去**: 条件付き 7 点メディアンフィルタ。数サンプル連続のクリック（キーボードなど）を抑えます。長いクリックや机を伝わる構造音には限界があります。
- **声の保護**: DFN の LSNR が高い時、指定した割合だけ乾音を混ぜます。`0%` で無効、`30%` 前後が試作の既定です。「ん〜」の抑圧が気になる時に上げてください。
- 推奨: 標準モデル、2 hop、`hybrid` プリセット。ポストフィルタは OFF。
- これは試作です。キーボード音が声と重なる場合や、構造音が大きい場合は、OBS 標準 RNNoise / NVIDIA Broadcast / Krisp の方が向いています。

### 設定の保存

低減量、ミックス、モデル、低遅延モード、しきい値などは OBS のシーンコレクションに保存されます。OBS 再起動時は `create()` で保存済み設定を読み直すため、前回の設定で復元されます。

## ビルド

### モデルの取得

```bash
./scripts/fetch-models.sh
```

Windows:

```powershell
./scripts/fetch-models.ps1
```

`models/` に 2 つの tar.gz をダウンロードし、SHA256 を検証します。`models/` は git 管理外です。

### DLL のみビルド（モデル・zip なし）

`cargo build` は DLL だけを作ります。モデルの取得・同梱・zip 作成は行いません。

```powershell
cargo build -p obs-filter --release --target x86_64-pc-windows-msvc
# 生成物: target/x86_64-pc-windows-msvc/release/obs_filter.dll
```

プリセットは DLL に内蔵されています（`presets/*.toml` を `include_str!`）。実行時に必要なのは DLL と外部モデルだけです。OBS のデータフォルダへ `models/*.tar.gz` を置いてください。

**GitHub Actions の Windows ビルドは、用途別に 2 つのワークフローがあります。**

| ワークフロー | artifact | 内容 |
|---|---|---|
| `Windows Build (DLL only)` | `obs-plugin_noise-cancelling-windows-x64-dll` | DLL のみ |
| `Windows Build (Full package)` | `obs-plugin_noise-cancelling-windows-x64-full` | DLL + `data/presets/*.toml` + `data/models/*.tar.gz` |

- **DLL だけ欲しいときは `Windows Build (DLL only)` を選びます。** モデル取得・同梱・zip 作成を行いません。
- 全部込みが欲しいときは `Windows Build (Full package)` を選びます。`package-windows.ps1` を呼び、モデル 2 種とプリセット 5 種を同梱します。ダウンロードした zip を展開すると `obs-plugin_noise-cancelling/` がそのまま使えます。
- テストと clippy は `Windows Checks` という別ワークフローで実行します。

> DLL のみにしても Rust / tract のコンパイル時間はほとんど変わりません。短縮されるのはモデル取得・コピー・zip 作成の部分だけです。

### Windows

```powershell
# DLL のみ（モデル・zip なし）
cargo build -p obs-filter --release --target x86_64-pc-windows-msvc

# 配布用フルパッケージ（presets + models + zip）
./build-aux/package-windows.ps1 -Configuration Release
```

`package-windows.ps1` は `dist/obs-plugin_noise-cancelling-windows-x64.zip` を生成します。

### macOS

```bash
# 配布用フルパッケージ（presets + models + plugin bundle）
./build-aux/package-macos.sh
```

`dist/obs-plugin_noise-cancelling.plugin` が生成されます。

### テスト

```bash
cargo fmt --check
cargo test -p df-core
cargo test -p obs-filter
cargo clippy -p df-core --all-targets -- -D warnings
```

Linux など OBS 非対応ホストで OBS バインディング本体を型チェックする場合:

```bash
cargo check -p obs-filter --features check-only
cargo clippy -p obs-filter --features check-only --lib -- -D warnings
```

`check-only` はローカル型チェック専用です。リリースビルドでは有効にしないでください。

### オフライン WAV 確認

実装完了後、ユーザー提供のサンプル WAV でノイズキャンセリング前後を比較できます。

```bash
cargo run -p df-core --release --example render -- \
  input.wav output.wav --model ll --preset standard
```

- 48 kHz / 44.1 kHz、モノラル / ステレオ、PCM 16/24/32-bit / float に対応。
- 入力と同じサンプルレート・チャンネル数で出力。
- ステレオは Mid/Side 処理。
- `--low-latency` で 1 hop 相当の遅延表示。
- テスト用の音源生成:

```bash
cargo run -p df-core --release --example make_test_wav -- target/test_noisy.wav 3.0 48000
```

## 構成

```text
obs-plugin_noise-cancelling/
├─ crates/df-core/      # DeepFilterNet、FIFO、リサンプル、プリセット、WAV レンダラ
├─ crates/obs-filter/   # OBS FFI、ワーカー、SPSC、UI
├─ presets/             # 同梱プリセット
├─ models/              # 取得したモデル（git 管理外。実行時に必要）
├─ scripts/             # モデル取得
├─ build-aux/           # パッケージ
├─ DESIGN.md            # 設計仕様（公開）
└─ .github/workflows/   # 手動ビルド CI
```

CI 設定・ディレクトリ構成・README の構成は `naruton999/obs-plugin_voice-changer` を参考にしています。コードは新規作成です。

## トラブルシューティング

| 症状 | 確認事項 |
|---|---|
| フィルタ一覧に出ない | DLL / `.plugin` の配置、OBS の再起動、アーキテクチャ（x64 / arm64） |
| 「モデル読込失敗」 | `models/` に 2 つの tar.gz があるか。SHA256 が一致するか |
| 設定が再起動で戻る | 最新ビルドか。`create()` で保存設定を読む修正が入っています |
| 44.1 kHz で動かない | OBS の音声設定が 44.1 kHz か 48 kHz か。UI のレート表示を確認 |
| 音が遅れる | UI の概算遅延を確認。1 hop モードを試す。標準モデルは LL より遅い |
| 音切れする | 2 hop モードを使う。CPU 使用率を確認 |
| ステレオ感が変わる | v0.1 は Mid 処理＋Side 保持。3ch 以上はバイパス |
| macOS で読み込まれない | arm64 ビルドか、署名が ad-hoc か、配置先を確認 |

## ライセンス

GPL-3.0-or-later。詳細は `LICENSE` と `THIRD_PARTY_NOTICES.md` を参照してください。

DeepFilterNet のコードとモデル重みは MIT / Apache-2.0 です。モデルは DeepFilterNet の pinned commit から取得し、`THIRD_PARTY_NOTICES.md` に SHA256 とライセンスを記載しています。

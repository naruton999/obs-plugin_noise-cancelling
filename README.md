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
| プリセット | `standard` / `gentle` / `strong` / `speech` |
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

既定ではモデルをバイナリに埋め込まず、`models/` から読み込みます。ビルド時に `embed-ll` / `embed-standard` / `embed-models` を指定すると、モデルを DLL / `.plugin` に埋め込めます。モデル切替には数秒かかり、切替中は乾音になります。

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

プリセットは DSP パラメータのみを変更します。モデルと低遅延モードは変更しません。

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

### DLL のみビルド（tar.gz を作らない）

モデルを同梱せず、DLL だけを最短で作りたい場合:

```powershell
# 既定の外部モデル。モデルはコンパイルされません
cargo build -p obs-filter --release --target x86_64-pc-windows-msvc
# 生成物: target/x86_64-pc-windows-msvc/release/obs_filter.dll
```

パッケージ script から DLL だけを出す場合:

```powershell
# 外部モデル用 DLL のみ（presets/models/zip なし）
./build-aux/package-windows.ps1 -Configuration Release -DllOnly

# モデル埋め込みの単一 DLL（tar.gz なし）
./build-aux/package-windows.ps1 -Configuration Release -DllOnly -EmbedModels
```

`-DllOnly` の出力は `dist/windows/obs-plugin_noise-cancelling.dll` だけです。zip、`data/presets`、`data/models` は作りません。プリセットは DLL 内蔵のものを使います。外部モデル版は、実行時に OBS のデータフォルダへ `models/*.tar.gz` を置いてください。

GitHub Actions の Windows Build には `dll_only` 入力があります。`embed_models` と組み合わせられます。

### モデルの埋め込み

既定は外部モデルです。モデルを DLL / `.plugin` に埋め込む場合は、以下のフィーチャーを使います。

| ビルド | コマンド | バイナリ増加 | 実行時の `models/` |
|---|---|---|---|
| 外部モデル（既定） | `cargo build -p obs-filter --release` | なし | 必要 |
| LL のみ埋め込み | `--features embed-ll` | 約 36 MB | LL は不要 |
| 標準のみ埋め込み | `--features embed-standard` | 約 8 MB | 標準は不要 |
| 両方埋め込み | `--features embed-models` | 約 44 MB | 不要 |

- 埋め込みビルドでも、**コンパイル時**に `models/` の tar.gz が必要です（`include_bytes!` のため）。先に `scripts/fetch-models.*` を実行してください。`-DllOnly` はモデルを自動取得しません。
- モデルは `crates/df-models` に分離されています。モデルが変わらなければ、`df-core` / `obs-filter` のコード変更時にモデルを再コンパイルせず、コンパイル済みの `df-models` を再利用します。
- 埋め込み時はパッケージに `data/models` / `Contents/Resources/models` を含めません。
- 既定の外部モデルビルドは `df-models` をビルドしないため、`cargo build -p obs-filter` は DLL だけをコンパイルします。tar.gz が作られるのは、パッケージ script がモデルを取得・同梱するときだけです。
- パッケージ scripts:
  - Windows: `./build-aux/package-windows.ps1 -Configuration Release -EmbedModels`
  - macOS: `./build-aux/package-macos.sh --embed-models`
- GitHub Actions の手動実行には `embed_models` と `dll_only` 入力があります。

### Windows

```powershell
# 外部モデル（既定）
cargo build -p obs-filter --release --target x86_64-pc-windows-msvc
./build-aux/package-windows.ps1 -Configuration Release -SkipBuild

# 両方埋め込み
./build-aux/package-windows.ps1 -Configuration Release -EmbedModels
```

`dist/obs-plugin_noise-cancelling-windows-x64.zip` が生成されます。

### macOS

```bash
# 外部モデル（既定）
cargo build -p obs-filter --release
./build-aux/package-macos.sh

# 両方埋め込み
./build-aux/package-macos.sh --embed-models
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
├─ crates/df-models/    # 埋め込みモデル専用（オプション）
├─ crates/obs-filter/   # OBS FFI、ワーカー、SPSC、UI
├─ presets/             # 同梱プリセット
├─ models/              # 取得したモデル（git 管理外。埋め込みビルドでもコンパイル時に必要）
├─ scripts/             # モデル取得
├─ build-aux/           # パッケージ
├─ docs/01-noise-cancelling.md
└─ .github/workflows/   # 手動ビルド CI
```

CI 設定・ディレクトリ構成・README の構成は `naruton999/obs-plugin_voice-changer` を参考にしています。コードは新規作成です。

## トラブルシューティング

| 症状 | 確認事項 |
|---|---|
| フィルタ一覧に出ない | DLL / `.plugin` の配置、OBS の再起動、アーキテクチャ（x64 / arm64） |
| 「モデル読込失敗」 | `models/` に 2 つの tar.gz があるか。SHA256 が一致するか。埋め込みビルドならモデル選択が埋め込み対象か |
| 設定が再起動で戻る | 最新ビルドか。`create()` で保存設定を読む修正が入っています |
| 44.1 kHz で動かない | OBS の音声設定が 44.1 kHz か 48 kHz か。UI のレート表示を確認 |
| 音が遅れる | UI の概算遅延を確認。1 hop モードを試す。標準モデルは LL より遅い |
| 音切れする | 2 hop モードを使う。CPU 使用率を確認 |
| ステレオ感が変わる | v0.1 は Mid 処理＋Side 保持。3ch 以上はバイパス |
| macOS で読み込まれない | arm64 ビルドか、署名が ad-hoc か、配置先を確認 |

## ライセンス

GPL-3.0-or-later。詳細は `LICENSE` と `THIRD_PARTY_NOTICES.md` を参照してください。

DeepFilterNet のコードとモデル重みは MIT / Apache-2.0 です。モデルは DeepFilterNet の pinned commit から取得し、`THIRD_PARTY_NOTICES.md` に SHA256 とライセンスを記載しています。

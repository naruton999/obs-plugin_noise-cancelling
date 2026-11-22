# 01. DeepFilterNet ノイズキャンセリング OBS プラグイン 確定仕様

| 項目 | 内容 |
|---|---|
| 文書番号 | 01 |
| 状態 | 確定（v0.1.0 実装用） |
| 対象リポジトリ | `obs-plugin_noise-cancelling` |
| プラグイン名 | `obs-plugin_noise-cancelling` |
| 表示名 | `DeepFilterNet ノイズキャンセリング` |
| 対象 OBS | OBS 31.1.1 API でビルド。OBS 32.2.1 以上で動作確認 |
| 対象 OS | Windows x64（最優先） / macOS arm64（ベストエフォート） |
| ライセンス | GPL-3.0-or-later |
| 元資料 | `docs/proposals/reference/deepfilternet-obs-plugin-handoff.md`（旧 handoff。本仕様が正式） |

---

## 1. 目的とスコープ

### 1.1 目的

OBS Studio の音声フィルタとして、マイクに入る定常ノイズ（PC ファン、エアコン、キーボード、部屋鳴り）を抑え、声を明瞭に残す。

- DSP は **DeepFilterNet3（DFN3）**。
- モデルは **LL（低遅延）** と **標準（高品質）** の 2 種類を UI から選択できる。
- 48 kHz と 44.1 kHz に対応する。44.1 kHz は内部で 48 kHz にリサンプルして処理し、44.1 kHz に戻す。
- OBS の音声スレッドを止めない（ワーカースレッド＋SPSC）。
- Windows x64 を最優先。macOS arm64 はビルド・動作確認をベストエフォートで行う。

### 1.2 v0.1.0 のゴール

| # | ゴール |
|---|---|
| G1 | マイクに追加するとノイズが減り、声が明瞭に残る |
| G2 | 48 kHz / 44.1 kHz で動作する |
| G3 | LL / 標準モデルをプルダウンで切り替えられる |
| G4 | 2 hop（既定）と 1 hop（低遅延モード）を選べる |
| G5 | プリセット、低減量、ミックス、PF、上級者しきい値、遅延維持バイパスを操作できる |
| G6 | 設定がシーンコレクションに保存・復元される |
| G7 | モデル読込中・切替中も OBS の音声スレッドが止まらない |
| G8 | レイテンシを表示し、実測と一致させる |
| G9 | オフライン WAV レンダラで入力 WAV → 出力 WAV を生成できる |

### 1.3 非ゴール

| # | 非ゴール |
|---|---|
| N1 | Linux 対応 |
| N2 | macOS x64（Intel）対応 |
| N3 | モデルの学習・ファインチューニング |
| N4 | 外部 VST / AU ホスト |
| N5 | GPU 推論 |
| N6 | 3 チャンネル以上の音声処理（バイパス＋警告） |
| N7 | モデルの自動ダウンロード（ビルド/パッケージ時に取得する） |

> 「ノイズキャンセリング」は製品名。技術的にはノイズ抑制（noise suppression）であり、逆位相による能動消音ではない。

---

## 2. 名称・識別子

| 対象 | 値 |
|---|---|
| リポジトリ | `obs-plugin_noise-cancelling` |
| Windows DLL | `obs-plugin_noise-cancelling.dll` |
| macOS バンドル | `obs-plugin_noise-cancelling.plugin` |
| macOS 実行ファイル | `obs-plugin_noise-cancelling` |
| データフォルダ | `data/obs-plugins/obs-plugin_noise-cancelling/` |
| フィルタ ID | `obs_plugin_noise_cancelling_filter` |
| 表示名 | `DeepFilterNet ノイズキャンセリング` |
| Cargo パッケージ | `df-core` / `obs-filter` |
| cdylib 出力 | `obs_filter.dll` / `libobs_filter.dylib`（パッケージ時にリネーム） |
| CI artifact（Win） | `obs-plugin_noise-cancelling-windows-x64` |
| CI artifact（mac） | `obs-plugin_noise-cancelling-macos-arm64` |
| macOS Bundle ID | `com.naruton999.obs-plugin-noise-cancelling` |

---

## 3. リポジトリ構成

```text
obs-plugin_noise-cancelling/
├─ Cargo.toml
├─ Cargo.lock
├─ rust-toolchain.toml
├─ .gitignore
├─ LICENSE
├─ THIRD_PARTY_NOTICES.md
├─ README.md
├─ crates/
│  ├─ df-core/
│  │  ├─ Cargo.toml
│  │  ├─ src/
│  │  │  ├─ lib.rs          # DenoiseParams, Engine, Denoiser, tests
│  │  │  ├─ backend.rs      # Backend trait
│  │  │  ├─ dfn.rs          # DeepFilterNet backend（外部/埋め込みモデル）
│  │  │  ├─ resample.rs     # 44.1k <-> 48k（rubato）
│  │  │  ├─ preset.rs       # TOML プリセット
│  │  │  └─ stereo.rs       # Mid/Side ヘルパー
│  │  └─ examples/render.rs # オフライン WAV レンダラ
│  ├─ df-models/           # 埋め込みモデル専用（オプション）
│  │  ├─ Cargo.toml
│  │  ├─ build.rs
│  │  └─ src/lib.rs
│  └─ obs-filter/
│     ├─ Cargo.toml
│     ├─ build.rs
│     └─ src/
│        ├─ lib.rs
│        ├─ ffi.rs          # OBS API 宣言（新規作成）
│        ├─ module.rs
│        ├─ filter.rs       # ワーカー、SPSC、状態管理
│        ├─ layout.rs       # チャンネル/Mid-Side
│        ├─ models.rs       # モデルファイル解決
│        ├─ params.rs       # atomic パラメータ
│        ├─ preset.rs       # df-core::preset の再エクスポート
│        └─ ui.rs
├─ presets/
│  ├─ standard.toml
│  ├─ gentle.toml
│  ├─ strong.toml
│  └─ speech.toml
├─ models/                  # git 管理外。fetch-models で取得
├─ data/obs-plugins/obs-plugin_noise-cancelling/
├─ scripts/
│  ├─ fetch-models.sh
│  └─ fetch-models.ps1
├─ build-aux/
│  ├─ package-macos.sh
│  └─ package-windows.ps1
├─ docs/
│  ├─ 01-noise-cancelling.md
│  └─ proposals/
├─ .github/workflows/
│  ├─ windows.yml
│  └─ macos.yml
└─ third_party/
   └─ DeepFilterNet/
      ├─ LICENSE-MIT
      └─ LICENSE-APACHE
```

### 3.1 実装方針

- **コードは新規作成**する。voice-changer のコードはコピーしない。
- **CI 設定、ディレクトリ構成、README の構成**は voice-changer を参考にする。
- OBS FFI は `libobs` の公開 API 宣言（`obs-source.h` 等）を元に新規作成する。
- `deep_filter` の pre-1.0 API は `dfn.rs` に閉じ込める。
- `df-core` は OBS 非依存。Linux でもテスト可能。
- 埋め込みモデルは `df-models` に分離し、モデルが変わらない限り再コンパイルしない。

---

## 4. DSP 仕様

### 4.1 パラメータ

| パラメータ | 範囲 | 既定 | 備考 |
|---|---|---|---|
| `enabled` | on/off | on | OFF は即時乾音（遅延 0） |
| `attenuation_limit_db` | 0〜100 dB | 100 | 100 で最大低減。0.01 未満は実質バイパス |
| `post_filter_enabled` | on/off | off | OFF は β=0 |
| `post_filter_beta` | 0〜0.05 | 0.0 | 0 で OFF |
| `mix` | 0〜1 | 1.0 | 0 で遅延乾音 |
| `min_db_thresh` | -15〜35 | -10 | 上級者向け |
| `max_db_erb_thresh` | -15〜35 | 30 | 上級者向け |
| `max_db_df_thresh` | -15〜35 | 20 | 上級者向け |
| `bypass` | on/off | off | 遅延維持バイパス |
| `model` | `ll` / `standard` | `ll` | プルダウン |
| `low_latency` | on/off | off | ON で 1 hop、OFF で 2 hop |

- `NaN` / `Inf` はサニタイズする。
- 範囲外はクランプする。
- `mix` と `attenuation_limit_db` は 10〜20 ms でスムージングする。
- `enabled=false` は FIFO を通さず即時乾音。再開時はリセット＋再プライム。
- `bypass=true` は `mix=0` と同じだが、DFN は動かして状態を暖める。

### 4.2 モデル

| モデル | ファイル | サイズ（圧縮） | 遅延の目安 | 特徴 |
|---|---|---|---|---|
| LL（低遅延） | `DeepFilterNet3_ll_onnx.tar.gz` | 36,359,660 bytes | 約 10 ms | 低遅延。パラメータ多め |
| 標準（高品質） | `DeepFilterNet3_onnx.tar.gz` | 7,983,136 bytes | 約 30 ms | 高品質。低 CPU/メモリ |

- 既定は **外部ファイル**として `models/` に置き、配布物に同梱する。
- ビルド時に埋め込みを選択できる:
  - `--features embed-ll`（LL のみ）
  - `--features embed-standard`（標準のみ）
  - `--features embed-models`（両方）
- 埋め込みモデルは `df-models` クレートに分離する。モデルが変わらなければ `df-core` / `obs-filter` の変更時に再コンパイルされない。
- 埋め込みビルドでも **コンパイル時**に `models/` の tar.gz が必要。無い場合は `df-models/build.rs` が分かりやすいエラーを出す。
- 取得元: DeepFilterNet commit `7b14b895fe7fc42cab72b4a4c1e49c982bd6404b` の `models/`。
- SHA256 を `scripts/fetch-models.*` と CI で検証する。
- 埋め込みモデルは UI の「未検出」警告の対象外。外部モデルが無い場合は UI に警告を出し、ステータスを `failed` にする。
- モデル切替はワーカースレッドで行う。切替中は乾音を出し、UI に「モデル切替中」を表示する。
- モデルは MIT / Apache-2.0。`THIRD_PARTY_NOTICES.md` に記載する。

### 4.3 サンプルレート

| 入力レート | 動作 |
|---|---|
| 48 kHz | リサンプルなし。DFN は 48 kHz で処理 |
| 44.1 kHz | `rubato::SincFixedIn` で 44.1k → 48k → 44.1k |
| その他 | バイパス＋UI 警告（v0.1） |

- リサンプラ設定: `sinc_len=64`, `oversampling_factor=128`, `f_cutoff=0.95`, `BlackmanHarris2`, `Linear`。
- リサンプル遅延は実測し、`L_total` に含める。
- 44.1 kHz の UI には「44.1 kHz → 48 kHz リサンプル中」を表示する。

### 4.4 ステレオ

```text
mid = 0.5 * (L + R)
side = 0.5 * (L - R)
mid_out = denoise(mid)
L' = mid_out + side_delayed
R' = mid_out - side_delayed
```

- Mid だけ DFN に通す（CPU 1 チャンネル分）。
- Side は `L_total` 分遅らせて再合成する。ステレオ感を維持する。
- 左右同一なら Side=0 となり、両チャンネルが同一出力になる。
- 3 チャンネル以上はバイパス＋UI 警告。

### 4.5 レイテンシ

| 記号 | 意味 |
|---|---|
| `L_dfn` | DFN 内部遅延（48 kHz）。初期推定 `(lookahead + 1) * hop`、実測で確定 |
| `L_resample` | 44.1 kHz 時のリサンプラ遅延（往復） |
| `L_worker` | ワーカー出力リングのプリフィル。2 hop（既定）または 1 hop（低遅延） |
| `L_total` | プラグイン内部遅延 = `L_dfn + L_resample + L_worker` |

- `L_dfn` は `(df.lookahead + 1) * hop` を初期値とし、インパルス応答で補正する。
- `L_worker` は 48 kHz 換算で 2 hop = 960 samples（20 ms）、1 hop = 480 samples（10 ms）。44.1 kHz では比率換算する。
- `L_total` を UI に ms で表示する。OBS の音声ブロック（約 21 ms）は別途注記する。
- 概算（44.1 kHz、LL、2 hop）: 約 30〜35 ms + OBS 約 21 ms。
- 概算（44.1 kHz、標準、2 hop）: 約 50〜55 ms + OBS 約 21 ms。
- 低遅延モード（1 hop）で約 10 ms 短縮。
- UI 表示と実測の差は ±1 ms 以内を目標とする。

> handoff の `out_fifo` プリフィル＋`dry_fifo`＋ワーカープリフィルの組み合わせは二重遅延になるため採用しない。役割を分離する。

---

## 5. `df-core` 設計

### 5.1 型

```rust
pub struct DenoiseParams { ... }

pub enum ModelKind { LowLatency, Standard }

pub trait Backend {
    fn sample_rate(&self) -> u32;
    fn hop_size(&self) -> usize;
    fn latency_samples(&self) -> usize;
    fn set_params(&mut self, params: &DenoiseParams);
    fn process_hop(&mut self, input: &[f32], output: &mut [f32]) -> anyhow::Result<()>;
    fn reset(&mut self) -> anyhow::Result<()>;
}

pub struct Denoiser { ... }           // 48 kHz
pub struct ResamplingDenoiser { ... } // 44.1 kHz
pub enum Engine {
    Native(Box<Denoiser>),
    Resampled(Box<ResamplingDenoiser>),
}
```

`Engine::new(backend, sample_rate, max_block)` が 48 kHz / 44.1 kHz を選ぶ。モデルと低遅延モードは `obs-filter` 側のワーカー設定。

### 5.2 48 kHz `Denoiser`

- `Backend` を所有し、可変長ブロックを `hop` に分割する。
- `in_fifo` / `dry_fifo` は `create` で容量を確保する。音声スレッドでは確保しない。
- `dry_fifo` は `L_dfn` 分遅らせ、DFN 出力と乾音を整列する。
- `process(input, output)` は処理済みサンプルを `output` に追記する。出力長は入力長と異なってよい（hop の端数は次回に持ち越す）。OBS 側の出力リングが遅延と端数を吸収する。
- `mix` / `attenuation_limit_db` は 10〜20 ms でスムージングする。同一パラメータの再設定は無視する。
- `reset` は FIFO をクリアし、`dry_fifo` を `L_dfn` のゼロで再プライムし、`backend.reset()` を呼ぶ。
- `enabled=false` は入力コピー。再有効化時は `reset`。
- `bypass=true` は `mix=0` として遅延乾音を出す。DFN は動かす。

### 5.3 44.1 kHz `ResamplingDenoiser`

- 入力を `SincFixedIn` で 48 kHz に変換し、`Denoiser` に渡す。
- `Denoiser` の出力を `SincFixedIn` で 44.1 kHz に戻す。
- `to_48k` / `from_48k` / `native_in` の FIFO を持つ。
- `process(input, output)` はネイティブレートの処理済みサンプルを `output` に追記する。出力長は入力長と異なってよい。
- `latency_samples()` は `up.output_delay()` + `L_dfn` + `down.output_delay()` をネイティブレートに換算して返す。
- リサンプラの `process_into_buffer` を使い、出力バッファは事前確保する。
- `reset()` は `Denoiser` と両リサンプラをリセットする。

### 5.4 `DfnBackend`

- `from_model(kind, path)` で、埋め込み対象なら `df_models` のバイト列を `DfParams::from_bytes` に渡し、そうでなければ外部ファイルを読む。
- `DfParams::new(path)` / `DfParams::from_bytes` をサポート。
- `model_embedded(kind)` で埋め込み対象かを公開する。UI の未検出警告はこれを見る。
- `RuntimeParams::default_with_ch(1)` を使用。
- `hop_size = df.hop_size`（通常 480）。
- `latency_samples = (df.lookahead + 1) * hop`。
- `process_hop` は `(1, hop)` の `Array2` を使い、hop ごとのアロケーションを避ける。
- `set_params` は `set_atten_lim`、`set_pf_beta`、しきい値を反映する。
- `reset` は `df.init()`。

### 5.5 プリセット

`schema_version = 1`。未知フィールドは無視する。

```toml
schema_version = 1
name = "standard"
description = "標準のノイズ抑制（最大低減）"
created = "2026-11-21"

[denoise]
enabled = true
attenuation_limit_db = 100.0
post_filter_beta = 0.0
mix = 1.0
min_db_thresh = -10.0
max_db_erb_thresh = 30.0
max_db_df_thresh = 20.0

[ui]
bypass = false
```

| ファイル | 低減量 | PF β | mix | 用途 |
|---|---|---|---|---|
| `standard.toml` | 100 dB | 0.0 | 100% | 標準。最大低減 |
| `gentle.toml` | 12 dB | 0.0 | 100% | 控えめ |
| `strong.toml` | 100 dB | 0.02 | 100% | 強め |
| `speech.toml` | 24 dB | 0.01 | 100% | 音声向け |

- プリセットは DSP パラメータのみ。モデル / 低遅延モードは変更しない。
- 適用時は `enabled=true`、`bypass=false`。
- ファイルが無い場合は `include_str!` のビルトインを使う。

### 5.6 オフライン WAV レンダラ

```bash
cargo run -p df-core --example render -- \
  input.wav output.wav --model ll --preset standard [--low-latency]
```

- 入力 WAV を読み、同じサンプルレート・チャンネル数で出力する。
- 48 kHz / 44.1 kHz、モノラル / ステレオ、PCM 16/24/32-bit / float をサポート。
- ステレオは Mid/Side 処理。
- 出力に `L_total` と処理時間を表示する。
- 実装完了後、ユーザー提供のサンプル WAV で確認する。

---

## 6. `obs-filter` 設計

### 6.1 スレッドモデル

```text
[OBS audio thread]
  mid/side を計算
  mid を入力リングへ push
  mid/side を遅延ラインへ push
  出力リングから mid_out を pop
  不足時は遅延乾音で補完、underruns++
  side_delayed と合成して L/R へ

[worker thread]
  入力リングからネイティブブロックを読む
  Engine.process()
  出力リングへ書く
  パラメータ / モデル / レート変更を反映
  入力が無ければ park
```

- 音声スレッド: SPSC、四則演算、atomic load のみ。
- 音声スレッドで禁止: mutex、ファイル I/O、ヒープ確保、ログ、tract 呼び出し。
- ワーカー: `Engine` を所有。`Thread::unpark()` で起床。
- `rtrb` の SPSC リングを使う。
- リング容量は 16384 samples。出力リングには `L_worker` 分のゼロをプリフィルする。
- 入力リング満杯時はブロック破棄＋overrun。出力リング空時は遅延乾音＋underrun。
- ワーカーは `park_timeout` で停止フラグを確認する。

### 6.2 状態

```rust
struct Shared {
    params: AtomicParams,     // settings（model / low_latency / sample_rate を含む）
    status: AtomicU32,        // 0=loading, 1=ready, 2=failed, 3=unsupported
    latency: AtomicU64,       // L_total (native samples)
    underruns: AtomicU64,
    overruns: AtomicU64,
    errors: AtomicU64,
    prime_request: AtomicU64, // 出力リング再プライムの世代
    prime_done: AtomicU64,    // 音声スレッドがドレイン完了した世代
    primed: AtomicBool,       // プリフィル中/完了。true の間は drain しない
}

struct AudioSide {
    in_tx: Option<rtrb::Producer<f32>>,
    out_rx: Option<rtrb::Consumer<f32>>,
    mid_dry: DelayLine,
    side_delay: DelayLine,
    mid_scratch: Vec<f32>,
    side_scratch: Vec<f32>,
    ready: bool,
    channels: usize,
}
```

- OBS はフィルタのロード時に `get_defaults()` → `create()` を呼び、`update()` は呼ばない。`create()` で保存済み `obs_data_t` を `read_settings()` して `AtomicParams` の初期値にする（設定復元）。
- `update()` はプロパティ変更時の反映に使う。

| 状態 | 音声出力 | UI |
|---|---|---|
| `loading` | 即時乾音 | モデル読込中 / 切替中 |
| `ready` + `enabled` | リング経由（`L_total` 遅延） | 動作中 |
| `ready` + `bypass` | リング経由の遅延乾音 | バイパス中 |
| `ready` + `enabled=false` | 即時乾音 | 無効 |
| `failed` | 即時乾音 | モデル読込失敗 |
| `unsupported` | 即時乾音 | 48 kHz / 44.1 kHz が必要 |

- `loading`/`failed`/`unsupported`/`enabled=false` の間、音声スレッドは出力リングをドレインし、`ready=false` にする。
- 出力リングの再プライムは `prime_request` / `prime_done` のハンドシェイクで行う。音声スレッドがドレインを完了してからワーカーが `L_worker` のゼロをプリフィルするため、プリフィルを誤って捨てない。
- `primed=true` の間（プリフィル中/完了後）は音声スレッドは drain しない。`ready` に戻ったら遅延ラインを `L_total` のゼロで再初期化する。
- v0.1 は遷移時に短い無音/乾音が入る。クリック低減のクロスフェードは v0.2 以降。
- `enabled=false` → `true` は最大 `L_total` の無音が入る（v0.1 許容）。
- `destroy` は `stop=true; unpark(); join()`。

### 6.3 モデル / レート / 低遅延の変更

- `update` は設定を atomic に保存する。
- ワーカーは `model` / `low_latency` / `sample_rate` の変化を検出したら、状態を `loading` にし、入力リングをドレインし、旧 Engine を drop してから新 Engine を作る。
- 新 Engine の `L_total` を計算し、`prime_output` ハンドシェイクで出力リングを `L_worker` のゼロでプリフィルして `ready` にする。
- モデル切替は数秒かかる。切替中は乾音。
- レートが 48 kHz / 44.1 kHz 以外なら `unsupported`。`active_config` をクリアして、対応レートに戻ったら再ビルドする。

### 6.4 OBS FFI

- `obs_module_set_pointer` / `obs_current_module` / `obs_module_ver` / `obs_module_load` / `obs_module_unload`。
- `obs_register_source_s` でフィルタを登録。
- `ObsSourceInfo` のレイアウトテスト（サイズ 408、`filter_audio` offset 136 など）を持つ。
- `LIBOBS_API_VER = 0x1F01_0001`。
- macOS: `-Wl,-undefined,dynamic_lookup`。
- Windows: `raw-dylib` で `obs.dll` にリンク。
- Linux は対象外（テスト時を除く）。

### 6.5 ファイル解決

- プリセット: `obs_find_module_file("presets/standard.toml")` → 親ディレクトリ。失敗時は `obs_get_module_data_path()/presets` → ビルトイン。
- モデル: `obs_find_module_file("models/<file>")` → 失敗時は `obs_get_module_data_path()/models/<file>`。
- 見つからない場合は UI に警告を出し、`failed` にする。

### 6.6 ログ

- `blog` に `noise-cancelling:` プレフィックス。
- 起動時: バージョン、レート、モデル、`L_total`、プリセット数。
- エラー: モデル読込失敗、`process` エラー、underrun/overrun の集計。
- 音声スレッドからはログを出さない。

---

## 7. UI 仕様

```text
┌ DeepFilterNet ノイズキャンセリング ─────────────┐
│ プリセット      [ standard             ▼ ]      │
│                          [プリセットを適用]    │
│ ───────────────────────────────────────────── │
│ モデル          [ LL（低遅延）          ▼ ]    │
│ ☐ 低遅延モード（1 hop、約 -10 ms）             │
│ ☑ ノイズキャンセリングを有効にする              │
│ 低減量        [────────●] 100 dB               │
│ ミックス      [────────●] 100 %                │
│ ☐ ポストフィルタ  β [──●──] 0.00              │
│ ───────────────────────────────────────────── │
│ ☐ 上級者向けしきい値                          │
│   Min [-10.0] dB  ERB [30.0] dB  DF [20.0] dB │
│ ───────────────────────────────────────────── │
│ ☐ 遅延維持バイパス                            │
│ 状態: 動作中                                   │
│ レート: 48 kHz                                 │
│ 概算遅延: 約 20 ms（+ OBS 約 21 ms）          │
│ v0.1.0                                         │
└───────────────────────────────────────────────┘
```

| 項目 | 形式 | 範囲 / 既定 | 備考 |
|---|---|---|---|
| プリセット | ドロップダウン | `standard` 他 | 選択だけでは適用しない |
| プリセットを適用 | ボタン | — | DSP 設定を書き込み `obs_source_update` |
| モデル | ドロップダウン | `ll` / `standard` / `ll` | 切替に数秒かかる |
| 低遅延モード | チェック | OFF / 2 hop | ON で 1 hop。音切れリスクを注記 |
| 有効 | チェック | ON | OFF は即時乾音 |
| 低減量 | スライダー | 0〜100 dB / 100 | 0 dB = 低減なし |
| ミックス | スライダー | 0〜100% / 100% | 0% = 遅延乾音 |
| ポストフィルタ | チェック | OFF | ON で β 有効 |
| β | スライダー | 0〜0.05 / 0.0 | |
| 上級者向けしきい値 | チェック | OFF | 3 スライダーの有効/無効を切替 |
| Min / ERB / DF | スライダー | -15〜35 / -10, 30, 20 | 値は常に適用 |
| 遅延維持バイパス | チェック | OFF | 処理は回したまま乾音 |
| 状態 | ラベル | — | 読込中 / 動作中 / 失敗 / 非対応レート |
| レート | ラベル | — | 48 kHz / 44.1 kHz（リサンプル） |
| 概算遅延 | ラベル | — | `L_total` と OBS ブロック |
| バージョン | ラベル | — | `CARGO_PKG_VERSION` |

- UI は日本語ハードコード。locale ファイルは v0.2 以降。
- 設定は `obs_data_t` に保存。独自ファイルは作らない。
- 状態ラベルはプロパティを開いた時点の値を表示する。

---

## 8. ビルド・パッケージ・CI

### 8.1 Cargo

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"
rust-version = "1.85"

[profile.release]
lto = "thin"
codegen-units = 1
```

`df-core`:

```toml
[features]
default = []
embed-ll = ["dep:df-models", "df-models/embed-ll"]
embed-standard = ["dep:df-models", "df-models/embed-standard"]
embed-models = ["embed-ll", "embed-standard"]

[dependencies]
deep_filter = { git = "https://github.com/Rikorose/DeepFilterNet", rev = "7b14b895fe7fc42cab72b4a4c1e49c982bd6404b", default-features = false, features = ["tract"] }
ndarray = "0.15"
anyhow = "1"
rubato = "0.14"
df-models = { path = "../df-models", optional = true }

# deep_filter 0.5.7-pre は tract 0.21.4 API を前提とする。caret 解決で
# 0.21.19 が選ばれると API 不整合になるため、tract 系を exact pin する。
tract-core = "=0.21.4"
tract-data = "=0.21.4"
tract-hir = "=0.21.4"
tract-linalg = "=0.21.4"
tract-nnef = "=0.21.4"
tract-onnx = "=0.21.4"
tract-pulse = "=0.21.4"

[dev-dependencies]
hound = "3.5"
```

`obs-filter`:

```toml
[lib]
crate-type = ["cdylib"]

[features]
check-only = []  # Linux などでのローカル型チェック専用
embed-ll = ["df-core/embed-ll"]
embed-standard = ["df-core/embed-standard"]
embed-models = ["embed-ll", "embed-standard"]

[dependencies]
df-core = { path = "../df-core" }
rtrb = "0.3"
anyhow = "1"
```

- 既定は外部モデル。`embed-ll` / `embed-standard` / `embed-models` で埋め込みを選択。
- 埋め込みバイト列は `df-models` に分離し、モデルが変わらない限り再コンパイルしない。
- `default-model-ll` などの `deep_filter` 側の埋め込み feature は使わない。
- `Cargo.lock` をコミットする。

### 8.2 モデル取得

`scripts/fetch-models.sh` / `.ps1`:

- `models/` に 2 つの tar.gz をダウンロード。
- SHA256 を検証。
- 既に存在しハッシュが一致すればスキップ。
- CI とパッケージ scripts から呼ぶ。

### 8.3 パッケージ

| OS | 出力 | 内容 |
|---|---|---|
| Windows | `dist/obs-plugin_noise-cancelling-windows-x64.zip` | `obs-plugin_noise-cancelling/obs-plugin_noise-cancelling.dll` + `data/presets/*.toml` + 外部モデル時のみ `data/models/*.tar.gz` |
| macOS | `dist/obs-plugin_noise-cancelling.plugin` | `Contents/MacOS/obs-plugin_noise-cancelling` + `Contents/Resources/presets/*.toml` + 外部モデル時のみ `Contents/Resources/models/*.tar.gz` |

- 外部モデル（既定）: `package-windows.ps1` / `package-macos.sh`。
- 両方埋め込み: Windows `-EmbedModels`、macOS `--embed-models`（または `EMBED_MODELS=1`）。`data/models` / `Contents/Resources/models` は同梱しない。
- 埋め込みビルドでもコンパイル時に `models/` が必要。パッケージ scripts は先に `fetch-models` を実行する。
- **DLL のみ（Windows）**: `package-windows.ps1 -DllOnly`。`dist/windows/obs-plugin_noise-cancelling.dll` だけを出力し、zip・presets・models を作らない。`-DllOnly -EmbedModels` で単一 DLL にモデルを埋め込める。
- `-DllOnly` はモデルを自動取得しない。外部モデル版は実行時に OBS のデータフォルダへ配置する。埋め込み版はコンパイル時に `models/` が必要。
- Windows OBS 33 以降: `C:\ProgramData\obs-studio\plugins\obs-plugin_noise-cancelling\`。
- Windows OBS 32.x / 旧配置: DLL を `obs-plugins\64bit\`、data を `data\obs-plugins\obs-plugin_noise-cancelling\`。
- macOS: `~/Library/Application Support/obs-studio/plugins/` または `OBS.app/Contents/PlugIns/`。
- macOS は ad-hoc 署名。`CODESIGN_IDENTITY` で上書き可能。
- `Info.plist` の Bundle ID は `com.naruton999.obs-plugin-noise-cancelling`。

### 8.4 CI

| ワークフロー | ランナー | 必須度 |
|---|---|---|
| `windows.yml` | `windows-2022` | 必須 |
| `macos.yml` | `macos-15`（arm64） | ベストエフォート |

- トリガーは `workflow_dispatch`。入力 `embed_models`（boolean、既定 false）で両方埋め込み、`dll_only`（boolean、既定 false）で DLL のみ出力を選択。
- fmt / `cargo test -p df-core` / `cargo test -p obs-filter` / clippy / fetch models / release build / package / upload。
- `embed_models=true` のときは `--features embed-models` でビルドし、パッケージにモデルを同梱しない。
- `dll_only=true` かつ `embed_models=false` のときはモデル取得をスキップし、DLL だけを出力する。
- `Swatinem/rust-cache@v2`。
- CI のテストは MockBackend 中心。実モデルはオフラインレンダラで確認する。
- Windows を優先し、macOS の失敗は v0.1.0 のリリースをブロックしない（README に明記）。

---

## 9. テスト・受け入れ基準

### 9.1 `df-core`

| テスト | 内容 |
|---|---|
| `requires_supported_rate` | 48k / 44.1k 以外でエラー |
| `latency_is_stable` | インパルスのピークが `L_dfn` と一致（MockBackend） |
| `block_size_independent` | 任意ブロックで出力が一致 |
| `bypass_is_delayed_dry` | `bypass=true` で遅延乾音 |
| `mix_zero_is_delayed_dry` | `mix=0` で同じ |
| `zero_attenuation_is_near_passthrough` | 低減量 0 で入力に近い |
| `nan_input_is_sanitized` | NaN/Inf で出力が有限 |
| `reset_clears_state` | リセット後に前の音が残らない |
| `enabled_is_immediate_passthrough` | `enabled=false` で遅延 0 |
| `resample_44k_roundtrip` | 44.1k → 48k → 44.1k でサンプル数・波形が破綻しない |
| `model_kind_ids` | `ll` / `standard` の相互変換 |
| `embedded_flags_follow_features` | `embed-*` フィーチャーと `model_embedded()` の一致 |
| `stereo_mid_side` | Mid/Side の往復、左右同一、左のみ |
| `preset_parse_*` | 同梱プリセット、未知フィールド、schema エラー |

### 9.2 `obs-filter`

| テスト | 内容 |
|---|---|
| `source_info_layout` | `ObsSourceInfo` のサイズ/オフセット（408 / filter_audio 136 など） |
| `audio_data_layout` | `ObsAudioData`（80 / frames 64 / timestamp 72） |
| `audio_info_layout` | `ObsAudioInfo`（8） |
| `params_round_trip` | atomic の保存/復元 |
| `model_kind_parse` | `ll` / `standard` の変換 |
| `mid_side_round_trip` | L/R → Mid/Side → L/R |

### 9.3 受け入れ基準

| ID | 基準 |
|---|---|
| AC-01 | マイクに追加するとノイズが減り、声が明瞭に残る |
| AC-02 | 48 kHz / 44.1 kHz で動作する。44.1 kHz はリサンプルされる |
| AC-03 | 8 時間連続で音切れ・アンダーラン 0（ウォームアップ後） |
| AC-04 | 1 コアの 20% 以下（LL、48 kHz、1ch、release）。目標 10% |
| AC-05 | UI 表示の遅延と実測が ±1 ms 以内 |
| AC-06 | フィルタ追加・削除・無効化を 50 回繰り返しても落ちない |
| AC-07 | シーンコレクション切替で設定が保持される |
| AC-08 | OBS 終了後に DLL / `.plugin` がロックされず削除できる |
| AC-09 | モデル読込中・切替中でも OBS の音声スレッドが止まらない |
| AC-10 | プリセットが適用・保存され、ビルトインフォールバックが動く |
| AC-11 | Windows zip が README どおりにインストールできる |
| AC-12 | 音声スレッドで確保・ロック・ログ・tract 呼び出しが無い |
| AC-13 | モデル 2 種をプルダウンで切り替えられる |
| AC-14 | 低遅延モード（1 hop）が動作し、遅延表示が変わる |
| AC-15 | モノラル、同一ステレオ、左のみステレオで破綻しない |
| AC-16 | オフライン WAV レンダラで入出力 WAV を生成できる |
| AC-17 | macOS arm64 はビルド・起動をベストエフォートで確認する |
| AC-18 | 外部モデル / `embed-ll` / `embed-standard` / `embed-models` の各ビルドが成立する |
| AC-19 | 低減量・ミックス・モデル・低遅延モードなどが OBS 再起動後も復元される |

### 9.4 手動確認

1. Windows の OBS 32.2.1 以上、48 kHz でマイクに追加。
2. 有効 / バイパス / 無効を聞き比べる。
3. LL / 標準モデルを切り替える。
4. 2 hop / 1 hop を切り替える。
5. 4 プリセットを適用する。
6. 44.1 kHz に変更し、リサンプル動作を確認。
7. ループバックで遅延を実測し、UI と比較。
8. 8 時間連続、フィルタ追加削除 50 回、シーン切替。
9. OBS 終了後にプラグインファイルを削除できることを確認。
10. CPU / メモリを計測。
11. ユーザー提供のサンプル WAV でオフライン出力を確認。

### 9.5 実装時の実測（2026-11-22）

合成音（トーン＋ノイズ、2 秒）をオフラインレンダラで処理した結果。実機 OBS の最終確認は別途行う。

| 条件 | DSP latency | plugin internal latency | 入力 RMS | 出力 RMS |
|---|---|---|---|---|
| LL / 48 kHz / 2 hop / standard | 10.00 ms | 30.00 ms | 0.150076 | 0.054353 |
| 標準 / 44.1 kHz / 2 hop / standard | 31.36 ms | 51.36 ms | 0.150087 | 0.041440 |

OBS の音声ブロック（約 21 ms）は上記に含まない。いずれも出力が無音にならず、RMS が低下していることを確認した。

---

## 10. ライセンス

| 対象 | ライセンス | 対応 |
|---|---|---|
| 本リポジトリ | GPL-3.0-or-later | `LICENSE` |
| DeepFilterNet コード | MIT / Apache-2.0 | `third_party/DeepFilterNet/` と `THIRD_PARTY_NOTICES.md` |
| DeepFilterNet3 モデル重み | MIT / Apache-2.0 | `THIRD_PARTY_NOTICES.md` |
| `tract` | Apache-2.0 / MIT | `THIRD_PARTY_NOTICES.md` |
| `rubato` | MIT | `THIRD_PARTY_NOTICES.md` |
| `rtrb` | MIT / Apache-2.0 | `THIRD_PARTY_NOTICES.md` |
| `hound`（dev） | Apache-2.0 / MIT | `THIRD_PARTY_NOTICES.md` |
| OBS libobs API | GPL-2.0-or-later | OBS プラグインとしての通常利用 |

- voice-changer のコードはコピーしない。CI / ディレクトリ / README の構成のみ参考にした旨を README に記載する。
- バイナリ配布時は対応するソース（本リポジトリ）を提供する。

---

## 11. リスクと対策

| リスク | 対策 |
|---|---|
| tract の初期化が重い | ワーカーで初期化。準備まで乾音。UI に読込中表示 |
| 44.1k リサンプルで遅延/CPU 増 | 低遅延 sinc、実測、低遅延モード |
| LL モデルの CPU/メモリ | 標準モデルを選択可能。実測して README に記載 |
| モデル 2 種で配布物が大きい | 外部ファイルを既定に。埋め込みは `embed-*` で選択。README にサイズ記載 |
| 埋め込みモデルでバイナリ増 | 外部モデルを既定に。`df-models` 分離で再コンパイルを抑制 |
| 埋め込みビルドで `models/` 必須 | `df-models/build.rs` が明示エラー。README に取得手順 |
| 設定が再起動で戻る | `create()` で保存済み `obs_data_t` を `read_settings()` する（修正済み） |
| モデル切替中の音切れ | 乾音＋短い無音（クロスフェードは v0.2）。UI に切替中表示 |
| アンダーラン | 2 hop 既定＋乾音フォールバック。1 hop は上級者向け |
| pre-1.0 API | commit 固定。`dfn.rs` に隔離 |
| Windows 実機テスト | Windows を最優先。CI と実機で確認 |
| macOS の優先度低下 | ベストエフォート。ビルド手順は README に記載 |
| サンドボックスに C コンパイラが無い | 純 Rust 依存のみ。必要なら `cc` を導入 |
| 複数フィルタでモデル多重ロード | v0.1 は 1 フィルタ想定。README に注意 |

---

## 12. 実装フェーズ

| フェーズ | 内容 | 状態 |
|---|---|---|
| P0 | 仕様、ブランチ、workspace、LICENSE、NOTICES、.gitignore、handoff 移動 | 完了 |
| P1 | `df-core`（Backend、Denoiser、Resampling、Preset、Mock テスト、WAV レンダラ） | 完了。LL 48 kHz / 標準 44.1 kHz の実モデルで WAV 出力を確認 |
| P2 | `obs-filter`（FFI、worker、UI、models、presets） | 完了。Linux `check-only` で型チェック・clippy 済み |
| P3 | パッケージ、CI、README、実機/オフライン確認 | 一部完了。ユーザー環境の OBS でノイズ抑制動作を確認済み。macOS 実機、ユーザー提供 WAV は未 |
| P4 | 設定復元（`create` で保存設定を読む）とモデル埋め込み（`df-models` + `embed-*`） | 実装・ビルド・テスト完了。OBS 再起動での設定復元確認は未 |

- ブランチは `feature/ai-0001-noise-cancelling`。完了までこのブランチで作業する。
- コミットは `AI <ai@example.com>` 名義。
- push / merge は行わない。
- 各フェーズ完了時に報告する。

# Third-party notices

`obs-plugin_noise-cancelling` is licensed under **GPL-3.0-or-later**. See
`LICENSE` for the full text.

This project uses the following third-party components. The notices below are
provided for binary and source distributions.

## DeepFilterNet

- Project: DeepFilterNet
- Source: https://github.com/Rikorose/DeepFilterNet
- Pinned commit: `7b14b895fe7fc42cab72b4a4c1e49c982bd6404b`
- Crate: `deep_filter` 0.5.7-pre (`libDF`)
- License: MIT OR Apache-2.0
- License texts: `third_party/DeepFilterNet/LICENSE-MIT`,
  `third_party/DeepFilterNet/LICENSE-APACHE`
- Copyright: Hendrik Schröter and DeepFilterNet contributors

### Model weights

The following model files are downloaded from the pinned DeepFilterNet commit
and distributed with the plugin. DeepFilterNet states that all code and model
weights in the repository are dual-licensed under MIT OR Apache-2.0.

| File | SHA256 |
|---|---|
| `DeepFilterNet3_ll_onnx.tar.gz` | `5998e58e8ba0e09bb76986ef97b84afa065a571ef282d4a1222f341e3251cf3a` |
| `DeepFilterNet3_onnx.tar.gz` | `c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616` |

Source URL pattern:

```text
https://raw.githubusercontent.com/Rikorose/DeepFilterNet/7b14b895fe7fc42cab72b4a4c1e49c982bd6404b/models/<file>
```

## tract

- Project: tract
- Source: https://github.com/sonos/tract
- Version: 0.21.4 (pinned for `deep_filter` compatibility)
- License: Apache-2.0 OR MIT
- Copyright: Sonos, Inc. and tract contributors

## rubato

- Project: rubato
- Source: https://github.com/HEnquist/rubato
- Version: 0.14.x
- License: MIT
- Copyright: Henrik Enquist

## rtrb

- Project: rtrb
- Source: https://github.com/mgeier/rtrb
- Version: 0.3.x
- License: MIT OR Apache-2.0
- Copyright: rtrb contributors

## hound (development tool only)

- Project: hound
- Source: https://github.com/ruuda/hound
- Version: 3.5.x
- License: Apache-2.0 OR MIT
- Copyright: Ruud van Asseldonk

`hound` is used by the offline WAV renderer and tests. It is not part of the
OBS plugin binary.

## OBS Studio / libobs API

- Project: OBS Studio
- Source: https://github.com/obsproject/obs-studio
- License: GPL-2.0-or-later
- The plugin uses the public libobs API declarations and links against the
  OBS process at load time. No libobs binary is bundled.

## Structure reference

The CI workflow shape, directory layout, and README section structure were
inspired by `naruton999/obs-plugin_voice-changer`. No source code from that
project is copied into this repository.

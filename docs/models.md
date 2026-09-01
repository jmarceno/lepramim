# Model artifacts

Lexaloud uses the **Kokoro-82M** neural TTS model by [hexgrad on Hugging
Face](https://huggingface.co/hexgrad/Kokoro-82M), accessed via the
[`kokoro-onnx`](https://github.com/thewh1teagle/kokoro-onnx) ONNX
wrapper by thewh1teagle.

## Artifacts

| File | Size | SHA256 (pinned) | Source |
|------|------|-----------------|--------|
| `kokoro-v1.0.onnx` | ~310 MB | `7d5df8ec...` (full hash in source) | https://github.com/thewh1teagle/kokoro-onnx/releases |
| `voices-v1.0.bin` | ~28 MB | `bca610b8...` (full hash in source) | same release |

Full pins are in [`src/lexaloud/models.py`](../src/lexaloud/models.py).
Both files are verified SHA256 on every daemon startup; a mismatch
refuses to load the model.

## Download location

```
~/.cache/lexaloud/models/
├── kokoro-v1.0.onnx
└── voices-v1.0.bin
```

Override with `XDG_CACHE_HOME`:

```bash
XDG_CACHE_HOME=/mnt/big-drive/.cache lexaloud download-models
```

## Licensing

- **Kokoro-82M model weights**: Apache-2.0 per the Hugging Face model
  card. If this ever changes upstream, update this document and
  `THIRD_PARTY_LICENSES.md` accordingly.
- **`kokoro-onnx` wrapper package**: MIT per the wheel's LICENSE file.
- **Voices**: bundled with the `voices-v1.0.bin` file under the same
  Apache-2.0 license as the weights.

Lexaloud does not modify or repackage the weights. Users redistributing
a Lexaloud installation in bulk (e.g., AppImage, Docker) should ensure
their distribution respects the upstream Apache-2.0 and MIT terms.

## Voices

Kokoro v1.0 ships with 54 voices. Lexaloud's control window exposes the
complete bundled catalog:

| ID | Description |
|----|-------------|
| American English | `af_heart`, `af_alloy`, `af_aoede`, `af_bella`, `af_jessica`, `af_kore`, `af_nicole`, `af_nova`, `af_river`, `af_sarah`, `af_sky`, `am_adam`, `am_echo`, `am_eric`, `am_fenrir`, `am_liam`, `am_michael`, `am_onyx`, `am_puck`, `am_santa` |
| British English | `bf_alice`, `bf_emma`, `bf_isabella`, `bf_lily`, `bm_daniel`, `bm_fable`, `bm_george`, `bm_lewis` |
| Spanish | `ef_dora`, `em_alex`, `em_santa` |
| French | `ff_siwis` |
| Hindi | `hf_alpha`, `hf_beta`, `hm_omega`, `hm_psi` |
| Italian | `if_sara`, `im_nicola` |
| Japanese | `jf_alpha`, `jf_gongitsune`, `jf_nezumi`, `jf_tebukuro`, `jm_kumo` |
| Brazilian Portuguese | `pf_dora`, `pm_alex`, `pm_santa` |
| Mandarin Chinese | `zf_xiaobei`, `zf_xiaoni`, `zf_xiaoxiao`, `zf_xiaoyi`, `zm_yunjian`, `zm_yunxi`, `zm_yunxia`, `zm_yunyang` |

Any voice string the installed voices pack recognizes works in
`config.toml`.

## Languages

The control window includes `en-us`, `en-gb`, `es`, `fr-fr`, `hi`, `it`,
`ja`, `pt-br`, and `zh`, matching the bundled Kokoro voices.

## Why 310 MB?

Neural TTS tradeoffs. Kokoro is intentionally small as neural models
go — the closest comparable open-weights models (XTTS-v2, Mars-5) are
hundreds of MB to multiple GB. 310 MB is a one-time download that
lives in `~/.cache` and persists across Lexaloud reinstalls.

## Recovering from a corrupt download

```bash
rm -rf ~/.cache/lexaloud/models
lexaloud download-models
```

The installer checks SHA256 and refuses to start with a corrupt file,
so "it just feels slow lately" is never a corrupt model — the daemon
would hard-fail at startup instead.

# Research delta

What was checked on top of `01_RESEARCH_REPORT.md`, and what is still open.
Checked on **12 November 2026**. "Measured" means run on the owner's laptop; "web" means read
from the linked page on that date; "open" means not yet verified.

## 1. This laptop (measured)

| Item | Finding |
|---|---|
| Machine | HP Victus 15-fb3xxx, Windows 11 Home Single Language, build 26300, 64-bit |
| Processor | AMD Ryzen 7 7445HS, 6 cores / 12 threads |
| Memory | 15.2 GB total. Only about 1.3 GB was free during the check, so other apps were using most of it |
| Graphics | NVIDIA GeForce RTX 4050 Laptop GPU (dedicated, Vulkan 1.4) and AMD Radeon 740M (integrated, Vulkan 1.4) |
| Disk | C: 48.5 GB free of 230.8 GB. D: 244 GB free (project and Rust toolchain live on D:) |
| Audio | Microphone Array (AMD), Speaker (Realtek), Bluetooth headset "Slainte" (headset + hands-free endpoints) |
| WebView2 | Installed, version 157.0.4341.46 |
| Already installed | Git 2.56, winget 1.29, Python 3.14.7 (Microsoft Store build; not used, not modified) |
| `node` on PATH | Was only the Microsoft Store placeholder, not real Node.js. Real Node.js LTS was installed |
| Not present | Ollama, Java, Android SDK |

Installed on 12 Nov 2026 with the owner's approval (all free, via `winget`): GitHub CLI,
Node.js LTS, CMake, Rust (rustup, MSVC toolchain, kept in `D:\dev\rust`), Visual Studio 2022
Build Tools with the C++ workload, LLVM. Rust is on D: because the Windows user name contains a
space, which breaks some C/C++ build scripts.

## 2. Keyboard shortcuts (measured)

Each shortcut was registered with `RegisterHotKey` for an instant and released again.
Error 1409 means another program already owns it.

| Shortcut | Planned use | Result |
|---|---|---|
| Ctrl+Win+H | Hands-free toggle | **Free** |
| Alt+Shift+Z | Paste last transcript | **Free** |
| Alt+Shift+X | Copy last transcript | **Free** |
| Alt+Shift+F | Fix last dictation | **Free** |
| Ctrl+Win+N | Scratchpad | **Taken** (error 1409) |
| Ctrl+Win+Space | (control) | Taken, as the report said |
| Win+H | (control) | Taken (Windows voice typing) |

**Consequence:** the build prompt's default scratchpad shortcut (Ctrl+Win+N) is already used
by Windows on this laptop. A different default must be chosen in Stage 7, from candidates
probed the same way. Limit of this check: it only sees shortcuts registered through
`RegisterHotKey`; a program that uses a low-level keyboard hook would not show up.

Hold-Ctrl+Win (push-to-talk) cannot be probed this way because it has no non-modifier key. It
is tested for real in Stage 1.

## 3. Items the report asked to verify

| # | Item | Status | Finding |
|---|---|---|---|
| 1 | Wispr Flow Windows release date (context only) | web | 12 March 2025, per Wispr's own changelog. |
| 2 | IndicConformer ONNX export | web, partly open | The official `ai4bharat/indic-conformer-600m-multilingual` repo is gated (needs a Hugging Face login and agreement). Third-party ONNX repackages exist. One describes the CTC head as 22 x 256 language tokens + 1 blank (5,633 outputs) and reports matching the original on a Hindi clip. **No official sherpa-onnx pre-trained IndicConformer was found**, so the plan of record is onnxruntime with our own decoding; sherpa-onnx stays optional. |
| 3 | IndicConformer Sindhi output script | **open** | Not stated on any page found. Must be settled in Stage 6 by inspecting the Sindhi vocabulary and decoding a real Sindhi clip. Until then the app must not promise a Sindhi script. |
| 4 | Sarvam: Sindhi support | web | Saaras v3 lists 23 languages (22 Indian + English) including Sindhi, code `sd-IN`. Modes: transcribe, translate, verbatim, translit, codemix. |
| 5 | Sarvam: Sindhi output script | **open** | Sarvam's language table shows Sindhi with its Perso-Arabic name (سنڌي), which suggests Perso-Arabic output, but no page says so directly. Test on real audio in Stage 6. |
| 6 | Sarvam pricing | web | Official pricing page: speech-to-text ₹30/hour, ₹45/hour with diarisation, billed per second. Matches the report. Some listings mention a newer `saaras:v4`; not verified. |
| 7 | Groq free limits and model names | web | Official rate-limits page: `whisper-large-v3-turbo` 20 requests/min, 2,000 requests/day, **7,200 audio-seconds/hour**, 28,800 audio-seconds/day. `openai/gpt-oss-20b`: 30 requests/min, 1,000 requests/day, 8,000 tokens/min, 200,000 tokens/day. New versus the report: the hourly audio cap (2 hours of audio per hour) and the 8K tokens/min cap. |
| 8 | Ctrl+Win+H and Alt+Shift+Z free on this laptop | measured | Both free (section 2). |
| 9 | Web Speech API languages in Chrome/Safari | **open** | Can only be checked in a running browser. Stage 4. |
| 10 | `getDisplayMedia` tab audio with Google Meet | **open** | Needs a real Meet call in Chrome on this laptop. Stage 4. |
| 11 | Google Play policy on AccessibilityService | web | Confirmed restrictive. `isAccessibilityTool` is only for apps that help people with disabilities. Other apps need a Play Console declaration plus prominent disclosure and consent. An October 2025 update prohibits using the API to autonomously plan and carry out actions. The plan stands: keyboard (IME) in the "play" flavour, bubble/accessibility only in the "sideload" flavour. |
| 12 | SignPath Foundation eligibility | web | Free for open-source projects. Needs: an OSI-approved licence, a public repository, multi-factor authentication for every team member (GitHub and SignPath), a published code-signing policy page with the required attribution line, builds verified as coming from a trusted build system (GitHub Actions), and an approver for each signing request. The certificate names **SignPath Foundation** as publisher. Guides also mention needing a prior release. Apply in Stage 8, after the first unsigned release. |
| 13 | CPU-only local LLM latency | **not run** | Ollama is not installed on this laptop. |
| 14 | Groq latency | **not run** | Needs the owner's own Groq API key. |
| 15 | India recording law | open | Not legal advice and not verified. The app always shows a consent reminder and never records automatically. |

## 4. New findings that change the plan

1. **whisper.cpp with Vulkan needs the Vulkan SDK at build time** (headers and the `VULKAN_SDK`
   variable). The laptop had Vulkan drivers but not the SDK. Vulkan SDK 1.4.363.0 was installed
   on 12 Nov 2026 with the owner's approval (`C:\VulkanSDK\1.4.363.0`). A GPU build has not been
   attempted yet.
2. **Possible build conflict on Windows.** Another open-source dictation app reported that
   `whisper-rs-sys` and the `tokenizers` crate use incompatible C runtime linking on Windows and
   that Vulkan builds crashed on machines without a GPU. Stage 1 must read Handy's real build
   setup before choosing crates, and must fall back to the processor when Vulkan is missing.
3. **Scratchpad shortcut conflict** (section 2).
4. **Accent colour on the light theme.** The suggested teal `#0FA3A3` has a contrast ratio of
   about 5.8:1 on the dark background but only about 3:1 on the light background `#FAFAF7`.
   The light theme needs a darker teal for text. To be fixed and tested with the design tokens
   in Stage 1.

## 5. Sources

- Wispr changelog, Flow for Windows: https://roadmap.wisprflow.ai/changelog/flow-for-windows-is-live
- AI4Bharat IndicConformer 600M: https://huggingface.co/ai4bharat/indic-conformer-600m-multilingual
- AI4Bharat IndicConformerASR: https://github.com/AI4Bharat/IndicConformerASR
- Sarvam Saaras model page: https://docs.sarvam.ai/api/getting-started/models/saaras
- Sarvam language codes: https://docs.sarvam.ai/api/api-guides-tutorials/speech-to-text/how-to/specify-language-codes
- Sarvam pricing: https://docs.sarvam.ai/api/getting-started/pricing
- Groq rate limits: https://console.groq.com/docs/rate-limits
- Google Play AccessibilityService policy: https://support.google.com/googleplay/android-developer/answer/10964491
- SignPath Foundation terms: https://signpath.org/terms
- SignPath origin verification: https://docs.signpath.io/origin-verification
- Epicenter v7.11.0 release notes (Windows whisper.cpp issue): https://newreleases.io/project/github/EpicenterHQ/epicenter/release/v7.11.0

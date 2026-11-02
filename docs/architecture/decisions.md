# Retained engineering decisions

These decisions consolidate the lasting findings of the earlier architecture
reviews. Reproduction lives in source tests and validation scripts; run-specific
reports are not build dependencies.

| Decision | Reason and consequence |
| --- | --- |
| Keep the existing Rust engine across frontend work | Musical timing, part lifecycle and DSP have one authority. Tauri/egui are clients, not competing engines. |
| Keep GUI, driver ownership and callback work separate | File/asset preparation and device calls cannot run on audio. Tauri's worker owns those operations; egui retains its existing ownership path. |
| Use coherent publication instead of combining diagnostics | Independent atomics can describe different generations. Readers reject overlapping publication; audio does not lock/retry for the GUI. |
| Bound command, receipt, parameter and MIDI work | Floods must not grow queues without limit or crowd Stop out. Parameters retain latest intent; stale holds are invalidated by release epochs. |
| Admit musical actions at the engine frontier | UI timestamps cannot rewrite audio already rendered. Exact-boundary inclusion is defined by rounded sample frames, including inverse-tick rounding edge cases. |
| Require pristine live-engine startup | An advanced scheduler without matching audio cursors previously allowed successful-looking silence. Prepare a fresh clock/bank/session instead. |
| Preserve short sources and interruption smoothing | Overlapping automatic fades must not erase short samples. Filtered EOF/stop/retrigger use bounded held-output release, not extra unbounded voices. |
| Retire live tempo history without shifting anchors | Repeated BPM edits must not exhaust old history forever; excessive simultaneous future edits still reject at capacity. Frozen offline maps remain separate. |
| Distinguish source playback from musical rules | One-shot EOF and spent Once are different states. Manual Stop suppresses automation, and cancel does not silently re-enable it. |
| Treat stream closure as terminal admission | Failed/stopped/dropped streams resolve pending tickets Closed without needing another render. Restart creates a new session instead of pretending to resume. |
| Keep stereo cue limitations explicit | Logical four-channel routing and software tests cannot prove private physical headphone output. |
| Pin tested toolchain/framework versions | Rust 1.93.0 supports the locked Tauri graph. Do not repeat the earlier incompatible toolchain pairing or upgrade dependencies as a guessed latency fix. |
| Retain egui during Tauri validation | Both use the shared Coordinator/MIDI/timeline contract. Development-default and live-show readiness are separate decisions. |
| Use portable restricted skin data and native recovery | CSS customization must not grant executable/native access or permanently remove reset controls. No plugin or arbitrary scripting framework is implied. |
| Do not pretend bounded admission cancels a driver | An OS call already in progress can delay its worker/shutdown join. This is a hardware/recovery gate, not a reason to move driver calls onto audio. |

Read [engine contracts](engine.md), [state semantics](performance-state.md) and
the [latency study](../M3_WEB4_1_LATENCY.md) for precise boundaries and remaining
validation. The [original product specification](../design/product-specification.md)
retains exploratory alternatives, including the historical JUCE fallback proposal;
it is not approval to replace the implemented engine or add unimplemented features.

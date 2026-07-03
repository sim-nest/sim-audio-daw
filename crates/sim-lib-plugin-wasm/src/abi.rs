//! Formal audio plugin ABI records shared by the host and test fixtures.

pub use sim_wasm_abi::audio::{
    EXPORT_MANIFEST_PTR, EXPORT_PREPARE, EXPORT_PROCESS, EXPORT_RESET, IMPORT_AUDIO_READ,
    IMPORT_AUDIO_WRITE, IMPORT_FRAME_COUNT, IMPORT_MODULE, IMPORT_PARAM_GET, WasmAudioManifest,
};

use sim_kernel::{
    AbiVersion, Cx, Export, Lib, LibManifest, LibTarget, Linker, Result, Symbol, Version,
};

use crate::{
    lan_buffered_audio_preview_profile, lan_render_return_profile, live_clock_symbol,
    realtime_local_audio_profile,
};

const AUDIO_GRAPH_LIVE_LIB_ID: &str = "audio-graph-live";

/// Loadable library that registers the live audio graph surface with a runtime.
pub struct AudioGraphLiveLib;

impl Lib for AudioGraphLiveLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(AUDIO_GRAPH_LIVE_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: live_symbols()
                .into_iter()
                .map(|symbol| Export::Value { symbol })
                .collect(),
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for symbol in live_symbols() {
            linker.value(symbol.clone(), live_value(cx, symbol)?)?;
        }
        Ok(())
    }
}

/// Installs [`AudioGraphLiveLib`] into the context once, idempotently.
pub fn install_audio_graph_live_lib(cx: &mut Cx) -> Result<()> {
    sim_lib_core::install_once(cx, &AudioGraphLiveLib).map(|_| ())
}

fn live_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "LiveGraphRunner"),
        Symbol::qualified("stream", "LiveControlToAudioQueue"),
        Symbol::qualified("stream", "LiveAudioToControlQueue"),
        Symbol::qualified("stream", "LivePlacementSite"),
        Symbol::qualified("stream", "LivePlacedNode"),
        Symbol::qualified("stream", "LiveTransportClock"),
        Symbol::qualified("stream", "RealtimeLocalAudioProfile"),
        Symbol::qualified("stream", "LanBufferedAudioPreviewProfile"),
        Symbol::qualified("stream", "LanRenderReturnProfile"),
    ]
}

fn live_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let role = match symbol.name.as_ref() {
        "LiveGraphRunner" => "live audio graph runner card",
        "LiveControlToAudioQueue" => "bounded control-to-audio queue card",
        "LiveAudioToControlQueue" => "bounded audio-to-control queue card",
        "LivePlacementSite" => "local live placement site card",
        "LivePlacedNode" => "placed live audio node card",
        "LiveTransportClock" => "live stream-clock transport card",
        "RealtimeLocalAudioProfile" => "realtime local audio transport profile card",
        "LanBufferedAudioPreviewProfile" => "LAN buffered audio preview profile card",
        "LanRenderReturnProfile" => "LAN render return profile card",
        _ => "live audio graph card",
    };
    let profile = match symbol.name.as_ref() {
        "LanBufferedAudioPreviewProfile" => lan_buffered_audio_preview_profile(),
        "LanRenderReturnProfile" => lan_render_return_profile(),
        _ => realtime_local_audio_profile(),
    };
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string("audio-graph-live".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (Symbol::new("role"), cx.factory().string(role.to_owned())?),
        (
            Symbol::new("clock"),
            cx.factory().symbol(live_clock_symbol())?,
        ),
        (
            Symbol::new("bounded"),
            cx.factory().string("control/audio queues".to_owned())?,
        ),
        (
            Symbol::new("profile"),
            cx.factory().symbol(profile.name().clone())?,
        ),
    ])
}

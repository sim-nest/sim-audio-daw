#![forbid(unsafe_code)]

use sim_kernel::Symbol;
use sim_lib_stream_host::{
    AudioDeviceCard, AudioPlacementRequest, AudioRouter, AudioSiteKey, FakeBackend,
    ModeledAudioSite,
};

use crate::{CpalModeledSite, cpal_modeled_site_symbol, default_modeled_cpal_site};

#[cfg(feature = "cpal-hardware")]
use std::{
    sync::{
        Arc,
        mpsc::{SyncSender, sync_channel},
    },
    time::Duration,
};

#[cfg(feature = "cpal-hardware")]
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, PcmPacket, PushResult, StreamMedia, StreamPacket,
};
#[cfg(feature = "cpal-hardware")]
use sim_lib_stream_host::{AudioSite, HostDirection, HostStreamConfigRequest};

#[test]
fn modeled_cpal_site_registers_and_opens_stream() {
    let mut router = AudioRouter::new();
    router.register(default_modeled_cpal_site());

    let key = AudioSiteKey(cpal_modeled_site_symbol());
    assert!(router.site(&key).is_some());
    assert_eq!(router.sites_by_capability(2, &[48_000]), vec![key.clone()]);

    let opened = router
        .open_placement(AudioPlacementRequest {
            site_key: key,
            stream_request: CpalModeledSite::playback_request(8).unwrap(),
        })
        .unwrap();

    assert_eq!(opened.config().device().to_string(), "fake/pcm");
    opened.close().unwrap();
}

#[test]
fn router_with_cpal_real_sites_has_modeled_fallback() {
    let mut router = AudioRouter::new();
    router.register(CpalModeledSite::default_stereo());

    #[cfg(feature = "cpal-hardware")]
    for site in crate::enumerate_cpal_sites().unwrap_or_default() {
        router.register(std::sync::Arc::new(site));
    }

    let key = AudioSiteKey(cpal_modeled_site_symbol());
    let capable = router.sites_by_capability(2, &[48_000]);
    assert!(capable.contains(&key));
}

#[test]
fn absent_jack_provider_degrades_to_modeled_site() {
    let mut router = AudioRouter::new();
    router.register(CpalModeledSite::default_stereo());
    let modeled = AudioSiteKey(cpal_modeled_site_symbol());
    let jack = AudioSiteKey(Symbol::qualified("audio/site", "jack-real-system"));

    let resolved = router.resolve_or_modeled(&jack, &modeled).unwrap();
    assert_eq!(resolved, modeled);

    let opened = open_same_graph(&router, resolved);
    assert_eq!(
        opened.config().media(),
        sim_lib_stream_core::StreamMedia::Pcm
    );
    opened.close().unwrap();
}

#[test]
fn same_graph_opens_against_modeled_or_provider_site() {
    let mut router = AudioRouter::new();
    router.register(CpalModeledSite::default_stereo());
    let modeled = AudioSiteKey(cpal_modeled_site_symbol());
    let provider = AudioSiteKey(Symbol::qualified("audio/site", "jack-modeled"));
    router.register(std::sync::Arc::new(ModeledAudioSite::new(
        AudioDeviceCard::modeled(provider.clone(), "JACK Provider Modeled"),
        std::sync::Arc::new(FakeBackend::new()),
    )));

    for requested in [modeled.clone(), provider] {
        let resolved = router.resolve_or_modeled(&requested, &modeled).unwrap();
        let opened = open_same_graph(&router, resolved);
        assert_eq!(
            opened.config().media(),
            sim_lib_stream_core::StreamMedia::Pcm
        );
        opened.close().unwrap();
    }
}

fn open_same_graph(
    router: &AudioRouter,
    site_key: AudioSiteKey,
) -> sim_lib_stream_host::HostOpenStream {
    router
        .open_placement(AudioPlacementRequest {
            site_key,
            stream_request: CpalModeledSite::playback_request(8).unwrap(),
        })
        .unwrap()
}

#[cfg(feature = "cpal-hardware")]
#[test]
fn cpal_real_site_smoke() {
    if std::env::var("SIM_CPAL_HARDWARE_SMOKE").as_deref() != Ok("1") {
        eprintln!("set SIM_CPAL_HARDWARE_SMOKE=1 to open a real cpal device");
        return;
    }

    let mut sites = crate::enumerate_cpal_sites().expect("cpal enumeration failed");
    assert!(!sites.is_empty(), "no cpal output devices found");

    let site = sites.remove(0);
    let key = site.key().clone();
    let channels = usize::from(site.card().channels_out.max(1));
    let request = HostStreamConfigRequest::new(
        crate::cpal_hardware_backend_symbol(),
        key.0.clone(),
        StreamMedia::Pcm,
        HostDirection::Output,
        BufferPolicy::bounded(64).unwrap(),
    )
    .with_clock(ClockDomain::Sample.symbol());

    let mut router = AudioRouter::new();
    router.register(Arc::new(site));
    let opened = router
        .open_placement(AudioPlacementRequest {
            site_key: key,
            stream_request: request,
        })
        .expect("open_placement failed");

    let silence = PcmPacket::f32(channels, 1, vec![0.0; channels]).unwrap();
    assert_eq!(
        opened
            .stream()
            .push_packet(sim_lib_stream_core::StreamItem::new(StreamPacket::Pcm(
                silence
            )))
            .unwrap(),
        PushResult::Accepted
    );

    opened.close().expect("close failed");
}

#[cfg(feature = "cpal-hardware")]
#[test]
fn config_from_cpal_uses_default_frames_with_bounds() {
    let supported = cpal::SupportedStreamConfig::new(
        2,
        cpal::SampleRate(96_000),
        cpal::SupportedBufferSize::Range { min: 128, max: 256 },
        cpal::SampleFormat::F32,
    );
    let config = crate::config_from_cpal(Symbol::new("cpal/test-output"), &supported).unwrap();

    assert_eq!(config.backend(), &crate::cpal_hardware_backend_symbol());
    assert_eq!(config.device(), &Symbol::new("cpal/test-output"));
    assert_eq!(config.media(), StreamMedia::Pcm);
    assert_eq!(config.direction(), HostDirection::Output);
    assert_eq!(config.buffer().capacity(), 256);
    assert_eq!(config.clock().clock(), &ClockDomain::Sample.symbol());
    assert_eq!(config.clock().sample_rate_hz(), Some(96_000));
    assert_eq!(config.latency().output_frames(), 256);
}

#[cfg(feature = "cpal-hardware")]
#[test]
fn cpal_driver_drop_releases_stored_stream_handle_without_sleep() {
    let (sender, receiver) = sync_channel(1);
    let driver = crate::CpalDriver::from_drop_probe(DropProbe(sender));

    drop(driver);

    receiver.recv_timeout(Duration::from_secs(1)).unwrap();
}

#[cfg(feature = "cpal-hardware")]
struct DropProbe(SyncSender<()>);

#[cfg(feature = "cpal-hardware")]
impl Drop for DropProbe {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

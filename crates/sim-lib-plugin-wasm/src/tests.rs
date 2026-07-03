#[cfg(feature = "wasm-plugin")]
mod wasm_plugin {
    use sim_lib_audio_graph_core::{
        BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Transport,
    };
    use sim_lib_plugin_core::{AudioPluginCapability, CapabilitySet, PluginInstance};

    use crate::{WasmPluginProcessor, WasmResourceLimits, load_wasm_plugin};

    fn round4(values: &[f32]) -> Vec<f32> {
        values
            .iter()
            .map(|value| (value * 10_000.0).round() / 10_000.0)
            .collect()
    }

    #[test]
    fn wasm_gain_plugin_loads_and_processes_stereo() {
        let wasm_bytes =
            wat::parse_str(include_str!("fixtures/gain.wat")).expect("valid gain fixture");
        let mut processor =
            WasmPluginProcessor::from_bytes(&wasm_bytes).expect("gain plugin loads");

        assert_eq!(processor.descriptor().id.stable_id, "sim.gain");
        assert_eq!(processor.descriptor().id.format.as_str(), "wasm");
        assert_eq!(processor.descriptor().ports.len(), 2);
        assert_eq!(processor.descriptor().parameters.len(), 1);

        processor.prepare(PrepareConfig::new(48_000, 64, 2, 2));
        processor.set_param(0, 0.5).expect("gain parameter exists");

        let input_l: Vec<f32> = (0..64).map(|frame| frame as f32 / 64.0).collect();
        let input_r: Vec<f32> = input_l.iter().map(|value| -value).collect();
        let mut out_l = vec![0.0; 64];
        let mut out_r = vec![0.0; 64];
        let mut outputs: Vec<&mut [f32]> = vec![out_l.as_mut_slice(), out_r.as_mut_slice()];
        let mut sink = NullEventSink;
        let mut scratch = BlockArena::with_f32_capacity(128);
        let mut block = ProcessBlock {
            frames: 64,
            in_audio: &[&input_l, &input_r],
            out_audio: &mut outputs,
            in_events: &[],
            out_events: &mut sink,
            transport: Transport::default(),
            scratch: &mut scratch,
        };

        processor.process(&mut block);

        let expected_l: Vec<f32> = input_l.iter().map(|value| value * 0.5).collect();
        let expected_r: Vec<f32> = input_r.iter().map(|value| value * 0.5).collect();
        assert_eq!(round4(&out_l), round4(&expected_l));
        assert_eq!(round4(&out_r), round4(&expected_r));
    }

    #[test]
    fn wasm_plugin_load_denied_without_capability() {
        let wasm_bytes =
            wat::parse_str(include_str!("fixtures/gain.wat")).expect("valid gain fixture");
        let result = load_wasm_plugin(
            &CapabilitySet::empty(),
            &wasm_bytes,
            WasmResourceLimits::default(),
        );
        let Err(err) = result else {
            panic!("load unexpectedly succeeded without capability");
        };

        assert!(format!("{err}").contains("plugin.audio.wasm"));
    }

    #[test]
    fn wasm_plugin_load_allowed_with_capability() {
        let wasm_bytes =
            wat::parse_str(include_str!("fixtures/gain.wat")).expect("valid gain fixture");
        let caps = CapabilitySet::with(AudioPluginCapability::WasmPlugin);
        let processor = load_wasm_plugin(&caps, &wasm_bytes, WasmResourceLimits::default())
            .expect("capability allows wasm load");

        assert_eq!(processor.descriptor().id.stable_id, "sim.gain");
    }

    #[test]
    fn wasm_plugin_memory_cap_is_enforced() {
        let wasm_bytes =
            wat::parse_str(include_str!("fixtures/gain.wat")).expect("valid gain fixture");
        let caps = CapabilitySet::with(AudioPluginCapability::WasmPlugin);
        let limits = WasmResourceLimits {
            fuel_per_process: 10_000_000,
            max_memory_pages: 0,
        };

        assert!(load_wasm_plugin(&caps, &wasm_bytes, limits).is_err());
    }

    #[test]
    fn wasm_plugin_fuel_exhaustion_returns_error_and_silences() {
        let wasm_bytes =
            wat::parse_str(include_str!("fixtures/gain.wat")).expect("valid gain fixture");
        let caps = CapabilitySet::with(AudioPluginCapability::WasmPlugin);
        let limits = WasmResourceLimits {
            fuel_per_process: 1,
            max_memory_pages: 64,
        };
        let mut processor = load_wasm_plugin(&caps, &wasm_bytes, limits).expect("plugin loads");
        processor.prepare(PrepareConfig::new(48_000, 64, 2, 2));

        let input_l = vec![1.0f32; 64];
        let input_r = vec![-1.0f32; 64];
        let mut out_l = vec![0.5f32; 64];
        let mut out_r = vec![0.5f32; 64];
        let mut outputs: Vec<&mut [f32]> = vec![out_l.as_mut_slice(), out_r.as_mut_slice()];
        let mut sink = NullEventSink;
        let mut scratch = BlockArena::with_f32_capacity(128);
        let mut block = ProcessBlock {
            frames: 64,
            in_audio: &[&input_l, &input_r],
            out_audio: &mut outputs,
            in_events: &[],
            out_events: &mut sink,
            transport: Transport::default(),
            scratch: &mut scratch,
        };

        let result = processor.process_checked(&mut block);
        let Err(err) = result else {
            panic!("low fuel did not trap");
        };

        assert!(format!("{err}").contains("trapped"));
        assert!(out_l.iter().all(|sample| *sample == 0.0));
        assert!(out_r.iter().all(|sample| *sample == 0.0));
    }
}

mod router_tests {
    use std::path::Path;

    use sim_lib_plugin_core::{AudioPluginCapability, CapabilitySet};

    use crate::{PluginRouter, WasmResourceLimits};

    #[test]
    fn router_unknown_extension_errors() {
        let router = PluginRouter::new(WasmResourceLimits::default());
        let caps = CapabilitySet::with(AudioPluginCapability::WasmPlugin);
        let result = router.load(Path::new("plugin.vst3"), &caps);
        let Err(err) = result else {
            panic!("unknown extension unexpectedly loaded");
        };

        assert!(format!("{err}").contains("unrecognised plugin extension"));
    }

    #[cfg(feature = "wasm-plugin")]
    #[test]
    fn router_wasm_route_uses_load_wasm_plugin() {
        let wasm_bytes =
            wat::parse_str(include_str!("fixtures/gain.wat")).expect("valid gain fixture");
        let path =
            std::env::temp_dir().join(format!("sim_gain_router_{}.wasm", std::process::id()));
        std::fs::write(&path, &wasm_bytes).expect("write wasm fixture");

        let router = PluginRouter::new(WasmResourceLimits::default());
        let caps = CapabilitySet::with(AudioPluginCapability::WasmPlugin);
        let plugin = router.load(&path, &caps).expect("wasm route loads");

        assert_eq!(plugin.descriptor().id.stable_id, "sim.gain");

        let _ = std::fs::remove_file(path);
    }

    #[cfg(feature = "clap-host")]
    #[test]
    fn router_clap_route_uses_provider_override() {
        use sim_lib_plugin_clap::native::FixtureClapHostProvider;
        use sim_lib_plugin_core::PluginFormat;

        let path = Path::new("fixture://gain.clap");
        let provider =
            FixtureClapHostProvider::gain().with_location(path.to_string_lossy().into_owned());
        let router = PluginRouter::builder(WasmResourceLimits::default())
            .with_clap_provider(provider)
            .build();
        let caps = CapabilitySet::with(AudioPluginCapability::NativePlugin);
        let plugin = router.load(path, &caps).expect("clap route loads");

        assert_eq!(plugin.descriptor().id.format, PluginFormat::Clap);
        assert_eq!(plugin.descriptor().id.stable_id, "org.sim.gain");
    }

    #[cfg(all(feature = "lv2-host", target_os = "linux"))]
    #[test]
    fn router_lv2_route_uses_provider_override() {
        use sim_lib_plugin_core::PluginFormat;
        use sim_lib_plugin_lv2::native::FixtureLv2HostProvider;

        let path = Path::new("https://sim.dev/lv2/gain.lv2");
        let provider = FixtureLv2HostProvider::gain().with_uri(path.to_string_lossy().into_owned());
        let router = PluginRouter::builder(WasmResourceLimits::default())
            .with_lv2_provider(provider)
            .build();
        let caps = CapabilitySet::with(AudioPluginCapability::NativePlugin);
        let plugin = router.load(path, &caps).expect("lv2 route loads");

        assert_eq!(plugin.descriptor().id.format, PluginFormat::Lv2);
        assert_eq!(plugin.descriptor().id.stable_id, "https://sim.dev/lv2/gain");
    }
}

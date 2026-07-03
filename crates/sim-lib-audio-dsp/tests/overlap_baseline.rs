//! Baseline test: freeze the exact set of exported card symbols so the
//! registration surface stays byte-identical.

#[test]
fn overlap_baseline_audio_dsp_symbols_are_frozen() {
    let mut got: Vec<String> = sim_lib_audio_dsp::audio_dsp_symbols()
        .iter()
        .map(|symbol| symbol.to_string())
        .collect();
    got.sort();
    let expected: Vec<&str> = vec![
        "audio-dsp/AllPassFilter",
        "audio-dsp/BiquadFilter",
        "audio-dsp/Chorus",
        "audio-dsp/CombFilter",
        "audio-dsp/Compressor",
        "audio-dsp/DcBlocker",
        "audio-dsp/DelayProcessor",
        "audio-dsp/Flanger",
        "audio-dsp/FractionalDelay",
        "audio-dsp/Gain",
        "audio-dsp/Gate",
        "audio-dsp/Limiter",
        "audio-dsp/OnePoleFilter",
        "audio-dsp/OversamplingWrapper",
        "audio-dsp/Pan",
        "audio-dsp/SmoothValue",
        "audio-dsp/SmoothedGain",
        "audio-dsp/SoftClipper",
        "audio-dsp/StateVariableFilter",
        "audio-dsp/Vibrato",
        "audio-dsp/Waveshaper",
    ];
    assert_eq!(got, expected);
}

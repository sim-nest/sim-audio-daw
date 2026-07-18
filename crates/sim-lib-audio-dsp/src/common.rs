use sim_lib_audio_graph_core::ProcessBlock;

pub(crate) const MIN_FILTER_HZ: f32 = 1.0;

pub(crate) fn input_sample(block: &ProcessBlock<'_>, channel: usize, frame: usize) -> f32 {
    block
        .in_audio
        .get(channel)
        .or_else(|| block.in_audio.first())
        .and_then(|lane| lane.get(frame))
        .copied()
        .unwrap_or(0.0)
}

pub(crate) fn output_channels(block: &ProcessBlock<'_>) -> usize {
    block.out_audio.len()
}

pub(crate) fn prepared_output_channels(
    block: &ProcessBlock<'_>,
    prepared: usize,
    processor: &str,
) -> usize {
    debug_assert!(
        output_channels(block) <= prepared,
        "{processor}::process received more channels than prepare configured"
    );
    output_channels(block).min(prepared)
}

pub(crate) fn clamp_cutoff(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
    cutoff_hz.clamp(MIN_FILTER_HZ, (sample_rate_hz * 0.49).max(MIN_FILTER_HZ))
}

pub(crate) fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

pub(crate) fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.max(1.0e-8).log10()
}

pub(crate) fn prepare_channels<T: Clone>(target: &mut Vec<T>, channels: usize, value: T) {
    target.clear();
    target.resize(channels, value);
}

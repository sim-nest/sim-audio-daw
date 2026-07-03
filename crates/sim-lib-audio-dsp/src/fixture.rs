use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

/// A deterministic input/expected-output pair for regression-testing a
/// [`Processor`] offline.
#[derive(Clone, Debug, PartialEq)]
pub struct GoldenFixture {
    /// Fixture name.
    pub name: &'static str,
    /// Sample rate used when preparing the processor.
    pub sample_rate_hz: u32,
    /// Input audio lanes, one per channel.
    pub input: Vec<Vec<f32>>,
    /// Expected output audio lanes, one per channel.
    pub expected: Vec<Vec<f32>>,
}

impl GoldenFixture {
    /// Returns the fixture frame count (length of the first input lane).
    pub fn frames(&self) -> u32 {
        self.input.first().map_or(0, Vec::len) as u32
    }
}

/// Returns the R30 gain golden fixture (a 0.25x gain reference).
pub fn r30_gain_golden_fixture() -> GoldenFixture {
    GoldenFixture {
        name: "r30-gain",
        sample_rate_hz: 48_000,
        input: vec![vec![1.0, -0.5, 0.25, 0.0, -0.25, 0.5, -1.0, 0.75]],
        expected: vec![vec![
            0.25, -0.125, 0.0625, 0.0, -0.0625, 0.125, -0.25, 0.1875,
        ]],
    }
}

/// Returns the R30 delay golden fixture (a two-sample impulse delay).
pub fn r30_delay_golden_fixture() -> GoldenFixture {
    GoldenFixture {
        name: "r30-delay",
        sample_rate_hz: 1_000,
        input: vec![vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0]],
        expected: vec![vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0]],
    }
}

/// Prepares and runs `processor` over a fixture's input, returning the rendered
/// output lanes for comparison against [`GoldenFixture::expected`].
pub fn run_offline<P: Processor>(
    processor: &mut P,
    fixture: &GoldenFixture,
    out_channels: usize,
) -> Vec<Vec<f32>> {
    let frames = fixture.frames() as usize;
    processor.prepare(PrepareConfig::new(
        fixture.sample_rate_hz,
        fixture.frames(),
        fixture.input.len() as u16,
        out_channels as u16,
    ));
    let mut output = vec![vec![0.0; frames]; out_channels];
    let input_refs: Vec<&[f32]> = fixture.input.iter().map(Vec::as_slice).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * out_channels.max(1));
    let mut block = ProcessBlock {
        frames: fixture.frames(),
        in_audio: &input_refs,
        out_audio: &mut output_refs,
        in_events: &[],
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    processor.process(&mut block);
    output
}

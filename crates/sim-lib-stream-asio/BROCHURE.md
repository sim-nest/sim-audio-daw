# sim-lib-stream-asio

In one line: A stand-in for the Windows ASIO low-latency sound system that lets SIM audio run and be tested without real drivers.

## What it gives you

This crate models ASIO, the low-latency driver interface Windows studios rely on, entirely in Rust. It links to no SDK and touches no real driver; instead it serves steady, made-up driver enumeration that behaves the same every run. It reports the ASIO drivers a host would see, hands back a plan for opening a stream, and bridges ASIO-style process callbacks into the audio graph. Because the same graph code can run against this model under Linux testing and against a native driver on Windows, an audio project keeps one code path across very different machines. A native adapter would target Windows and fill the same model from real driver enumeration.

## Why you will be glad

- Your audio path can be exercised without the Steinberg SDK or an ASIO driver installed.
- The same graph code runs against the model in testing and a real driver on Windows.
- Made-up driver enumeration stays steady, so results do not drift between runs.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. It covers the Windows low-latency path, sitting beside the ALSA, JACK, PipeWire, and PortAudio adapters. It gives SIM an ASIO-shaped surface to build and validate against long before any Windows hardware is in reach.

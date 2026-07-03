//! Baseline test: freeze the exact set of exported card symbols so the
//! registration surface stays byte-identical.

#[test]
fn overlap_baseline_clap_plugin_symbols_are_frozen() {
    let mut got: Vec<String> = sim_lib_plugin_clap::clap_plugin_symbols()
        .iter()
        .map(|symbol| symbol.to_string())
        .collect();
    got.sort();
    let expected: Vec<&str> = vec![
        "plugin-clap/ClapEvent",
        "plugin-clap/ClapExportedProcessor",
        "plugin-clap/ClapGainFixture",
        "plugin-clap/ClapHostProcessor",
        "plugin-clap/ClapParamMap",
        "plugin-clap/ClapSynthFixture",
    ];
    assert_eq!(got, expected);
}

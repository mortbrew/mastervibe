//! Public-API walkthrough of one breach on a shared seed.

use mastervibe_core::{
    count_consistent, embedded_catalog, generate_secret, is_consistent, score_pegs, score_probe,
    Amp, Breach, BreachStatus, Code, Observation, RunSeed, ScoreHook, ScoreLineKind, ToolMod,
};

#[test]
fn shared_seed_plays_a_breach_with_a_tool_hook() {
    let seed = RunSeed::parse("MV-7F3A-9C21-L1").unwrap();
    let generated = generate_secret(seed);
    assert!(generated.fair);
    assert_eq!(generated.code.len(), 4);
    assert!(generated.code.max_frequency() <= 2);

    let catalog = embedded_catalog();
    let mut breach = Breach::new(generated.spec, generated.code).unwrap();
    assert_eq!(breach.status(), BreachStatus::Open);
    assert_eq!(breach.consistent_count().unwrap(), generated.spec.space());

    let opener = Code::parse(generated.spec.charset(), "0123").unwrap();
    let spike = ToolMod {
        id: "spike".to_string(),
        bits: 0,
        amp: Amp::from_milli(500),
    };
    let hooks: [&dyn ScoreHook; 1] = [&spike];
    let turn = breach.probe(&opener, catalog, &hooks).unwrap().clone();

    let pegs = score_pegs(&generated.code, &opener).unwrap();
    assert_eq!(turn.pegs, pegs);
    let expected = score_probe(&opener, pegs, catalog, Some(generated.spec), &hooks);
    assert_eq!(turn.score, expected);
    assert!(turn.score.lines.iter().any(|line| matches!(
        &line.kind,
        ScoreLineKind::Tool { id } if id == "spike"
    )));

    let feedback = [Observation {
        probe: opener,
        pegs,
    }];
    assert_eq!(
        breach.consistent_count().unwrap(),
        count_consistent(4, 6, &feedback).unwrap()
    );
    assert!(is_consistent(&generated.code, &feedback).unwrap());
    assert!(breach.is_consistent(&generated.code).unwrap());

    if pegs.all_hits(4) {
        assert_eq!(breach.status(), BreachStatus::Cracked);
    } else {
        assert!(breach.is_open());
        let rest = breach.probes_left();
        assert!(rest < usize::from(generated.spec.probe_budget()));
        let _ = breach.probe(&generated.code, catalog, &[]).unwrap();
        assert_eq!(breach.status(), BreachStatus::Cracked);
        assert_eq!(breach.consistent_count().unwrap(), 1);
    }
}

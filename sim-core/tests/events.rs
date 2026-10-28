//! Kingdom events (design, Architecture → typed events; R4; D11): the sim
//! records what its kingdoms did, and every event points at something real.

use sim_core::events::{self, EventKind};
use sim_core::world::World;

#[test]
fn a_growing_world_reports_its_kingdoms() {
    let mut world = World::new(1);
    world.step(1_500);
    let (events, dropped) = world.drain_events();
    assert_eq!(dropped, 0, "1,500 years fit in the queue");

    let kinds: Vec<&str> = events.iter().map(|e| events::label(e.kind)).collect();
    for wanted in ["tier_up", "building", "kingdom", "government"] {
        assert!(
            kinds.contains(&wanted),
            "expected a {wanted} event in {kinds:?}"
        );
    }
    assert!(
        events.windows(2).all(|pair| pair[0].tick <= pair[1].tick),
        "events arrive in the order they happened"
    );
    assert!(
        events.iter().all(|e| e.tick % 25 == 0),
        "kingdom changes happen on the 25-year check"
    );
}

#[test]
fn every_event_points_at_a_real_place_and_people() {
    let mut world = World::new(1);
    world.step(1_500);
    let (events, _) = world.drain_events();
    for event in &events {
        assert!(
            world.cultures().get(event.culture_id as usize).is_some(),
            "{event:?} names a people that exists"
        );
        let text = events::describe(&world, event);
        assert!(!text.is_empty(), "{event:?} reads as words");
        if let Some(polity) = event.polity_id {
            assert!(
                world.polities().iter().any(|p| p.id == polity),
                "{event:?} names a kingdom that exists"
            );
        }
    }
}

#[test]
fn the_latest_tier_event_matches_the_settlement_now() {
    let mut world = World::new(1);
    world.step(1_500);
    let (events, _) = world.drain_events();
    for settlement in world.settlements() {
        let last = events.iter().rev().find(|e| {
            e.settlement_id == Some(settlement.id)
                && matches!(
                    e.kind,
                    EventKind::TierUp { .. } | EventKind::TierDown { .. }
                )
        });
        if let Some(event) = last {
            let (EventKind::TierUp { tier } | EventKind::TierDown { tier }) = event.kind else {
                unreachable!("filtered to tier events")
            };
            assert_eq!(
                tier, settlement.tier,
                "the last tier event is the tier it holds"
            );
        } else {
            assert_eq!(
                settlement.tier, 0,
                "a settlement that never changed tier is a camp"
            );
        }
    }
}

#[test]
fn a_drain_hands_over_each_event_once() {
    let mut world = World::new(1);
    world.step(1_000);
    let (first, _) = world.drain_events();
    assert!(!first.is_empty());
    let (again, _) = world.drain_events();
    assert!(again.is_empty(), "drained events are gone");
    world.step(250);
    let (later, _) = world.drain_events();
    assert!(
        later.iter().all(|e| e.tick > 1_000),
        "only new events follow"
    );
}

#[test]
fn events_do_not_change_history() {
    // The queue is for the UI: draining it, or never draining it, leaves the
    // world the same.
    let mut drained = World::new(3);
    let mut ignored = World::new(3);
    for _ in 0..8 {
        drained.step(125);
        drained.drain_events();
        ignored.step(125);
    }
    assert_eq!(drained.population, ignored.population);
    assert_eq!(drained.culture_id, ignored.culture_id);
    assert_eq!(drained.settlements().len(), ignored.settlements().len());
}

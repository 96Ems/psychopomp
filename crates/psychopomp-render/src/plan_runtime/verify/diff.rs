//! A compact account of how two plan or reel files differ, keyed by the IDs
//! authors use (segments, actors, channels, cues, media) instead of array
//! indices: one line per changed thing.
use std::collections::BTreeSet;

use serde_json::Value;

pub(super) fn summarize(before: &Value, after: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if before.get("segments").is_some() || after.get("segments").is_some() {
        reel(before, after, &mut lines);
    } else {
        plan("", before, after, &mut lines);
    }
    lines
}

fn reel(before: &Value, after: &Value, lines: &mut Vec<String>) {
    fields("", before, after, &["segments"], lines);
    fn id(segment: &Value) -> Option<&str> {
        segment.pointer("/plan/id").and_then(Value::as_str)
    }
    let order = |reel: &Value| {
        array(reel, "segments")
            .iter()
            .filter_map(|segment| id(segment).map(str::to_owned))
            .collect::<Vec<_>>()
    };
    for (key, before, after) in keyed(
        array(before, "segments"),
        array(after, "segments"),
        |segment| id(segment).map(str::to_owned),
    ) {
        match (before, after) {
            (None, Some(_)) => lines.push(format!("segment {key} added")),
            (Some(_), None) => lines.push(format!("segment {key} removed")),
            (Some(before), Some(after)) => {
                let prefix = format!("{key}: ");
                fields(&prefix, before, after, &["plan"], lines);
                plan(&prefix, &before["plan"], &after["plan"], lines);
            }
            (None, None) => {}
        }
    }
    let (before, after) = (order(before), order(after));
    let common = |ids: &[String], other: &[String]| {
        ids.iter()
            .filter(|id| other.contains(id))
            .cloned()
            .collect::<Vec<_>>()
    };
    if common(&before, &after) != common(&after, &before) {
        lines.push("segments reordered".to_owned());
    }
}

fn plan(prefix: &str, before: &Value, after: &Value, lines: &mut Vec<String>) {
    const LISTS: [(&str, &str); 7] = [
        ("actors", "actor"),
        ("continuousChannels", "channel"),
        ("stateChannels", "state"),
        ("cues", "cue"),
        ("media", "media"),
        ("semanticTargets", "target"),
        ("presentationSteps", "step"),
    ];
    if before.get("durationNanos") != after.get("durationNanos") {
        lines.push(format!(
            "{prefix}duration {} → {}",
            seconds(before.get("durationNanos")),
            seconds(after.get("durationNanos"))
        ));
    }
    let lists = LISTS.map(|(key, _)| key);
    fields(
        prefix,
        before,
        after,
        &[&lists[..], &["durationNanos"]].concat(),
        lines,
    );
    for (list, noun) in LISTS {
        for (id, before, after) in keyed(array(before, list), array(after, list), |item| {
            item.get("id").and_then(Value::as_str).map(str::to_owned)
        }) {
            let head = format!("{prefix}{noun} {id}");
            match (before, after) {
                (None, Some(after)) => lines.push(format!("{head} added{}", shape(after))),
                (Some(_), None) => lines.push(format!("{head} removed")),
                (Some(before), Some(after)) if before != after => {
                    lines.push(format!("{head} {}", item(list, before, after)));
                }
                _ => {}
            }
        }
    }
}

/// What changed inside one keyed item.
fn item(list: &str, before: &Value, after: &Value) -> String {
    let mut parts = Vec::new();
    match list {
        "continuousChannels" | "stateChannels" => {
            if before.get("initial") != after.get("initial") {
                parts.push(format!(
                    "initial {} → {}",
                    compact(before.get("initial")),
                    compact(after.get("initial"))
                ));
            }
            let (old, new) = (array(before, "events"), array(after, "events"));
            if old != new {
                let first = old.iter().zip(new).position(|(a, b)| a != b);
                let first = first.unwrap_or(old.len().min(new.len()));
                let at = new
                    .get(first)
                    .or_else(|| old.get(first))
                    .and_then(|event| event.get("atNanos"));
                let count = if old.len() == new.len() {
                    String::new()
                } else {
                    format!(" ({} → {} events)", old.len(), new.len())
                };
                parts.push(format!("events changed from {}{count}", seconds(at)));
            }
            let mut rest = Vec::new();
            paths(
                "",
                Some(before),
                Some(after),
                &["initial", "events"],
                &mut rest,
            );
            if !rest.is_empty() {
                parts.push(format!("changed {}", list_paths(&rest)));
            }
        }
        "cues" => parts.push(format!(
            "{}..{} → {}..{}",
            seconds(before.get("startNanos")),
            seconds(before.get("endNanos")),
            seconds(after.get("startNanos")),
            seconds(after.get("endNanos"))
        )),
        _ => {
            let mut changed = Vec::new();
            paths("", Some(before), Some(after), &[], &mut changed);
            parts.push(format!("changed {}", list_paths(&changed)));
        }
    }
    parts.join("; ")
}

/// Scalar fields of two objects other than `skip`, reported by name.
fn fields(prefix: &str, before: &Value, after: &Value, skip: &[&str], lines: &mut Vec<String>) {
    let keys = |value: &Value| {
        value
            .as_object()
            .map(|object| object.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let keys = keys(before)
        .into_iter()
        .chain(keys(after))
        .filter(|key| !skip.contains(&key.as_str()))
        .collect::<BTreeSet<_>>();
    for key in keys {
        let (old, new) = (before.get(&key), after.get(&key));
        if old != new {
            lines.push(format!("{prefix}{key} {} → {}", compact(old), compact(new)));
        }
    }
}

/// Changed leaf paths under two values: objects by key, arrays of identified
/// objects by ID, and anything else (scalars, numeric arrays) as one leaf.
fn paths(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    skip: &[&str],
    out: &mut Vec<String>,
) {
    if before == after {
        return;
    }
    let join = |key: &str| {
        if path.is_empty() {
            key.to_owned()
        } else {
            format!("{path}.{key}")
        }
    };
    match (before, after) {
        (Some(Value::Object(old)), Some(Value::Object(new))) => {
            let keys = old.keys().chain(new.keys()).collect::<BTreeSet<_>>();
            for key in keys.into_iter().filter(|key| !skip.contains(&key.as_str())) {
                paths(&join(key), old.get(key), new.get(key), &[], out);
            }
        }
        (Some(Value::Array(old)), Some(Value::Array(new)))
            if old.iter().chain(new).all(|item| item.get("id").is_some()) =>
        {
            let id = |item: &Value| item.get("id").and_then(Value::as_str).map(str::to_owned);
            for (key, old, new) in keyed(old, new, id) {
                paths(&format!("{path}[{key}]"), old, new, &[], out);
            }
        }
        _ => out.push(if path.is_empty() { "value" } else { path }.to_owned()),
    }
}

fn list_paths(paths: &[String]) -> String {
    const SHOWN: usize = 4;
    let mut text = paths
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > SHOWN {
        text.push_str(&format!(" +{} more", paths.len() - SHOWN));
    }
    text
}

/// Items of two lists matched by key (an item without one by its index): the
/// new list's order, then removed items.
fn keyed<'a>(
    before: &'a [Value],
    after: &'a [Value],
    key: impl Fn(&Value) -> Option<String>,
) -> Vec<(String, Option<&'a Value>, Option<&'a Value>)> {
    let keys = |items: &'a [Value]| {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| (key(item).unwrap_or_else(|| format!("#{index}")), item))
            .collect::<Vec<_>>()
    };
    let (before, after) = (keys(before), keys(after));
    let find = |items: &[(String, &'a Value)], wanted: &str| {
        items
            .iter()
            .find(|(key, _)| key == wanted)
            .map(|&(_, item)| item)
    };
    let mut matched = after
        .iter()
        .map(|(key, item)| (key.clone(), find(&before, key), Some(*item)))
        .collect::<Vec<_>>();
    for (key, item) in &before {
        if find(&after, key).is_none() {
            matched.push((key.clone(), Some(*item), None));
        }
    }
    matched
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A brief description of an added channel or item.
fn shape(item: &Value) -> String {
    match (
        item.get("initial"),
        item.get("events").and_then(Value::as_array),
    ) {
        (Some(initial), Some(events)) => format!(
            " (initial {}, {} events)",
            compact(Some(initial)),
            events.len()
        ),
        _ => String::new(),
    }
}

fn seconds(nanos: Option<&Value>) -> String {
    match nanos.and_then(Value::as_u64) {
        Some(nanos) => format!("{:.3}s", nanos as f64 / 1e9),
        None => compact(nanos),
    }
}

fn compact(value: Option<&Value>) -> String {
    const LIMIT: usize = 40;
    let Some(value) = value else {
        return "∅".to_owned();
    };
    let text = value.to_string();
    if text.chars().count() <= LIMIT {
        return text;
    }
    text.chars().take(LIMIT - 1).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::summarize;

    fn plan() -> Value {
        json!({
            "version": 2,
            "id": "hello",
            "durationNanos": 4_000_000_000u64,
            "actors": [{ "id": "stage", "recipe": "stage", "data": {
                "elements": [
                    { "id": "client", "kind": "card", "at": [560.0, 540.0, 0.0] },
                    { "id": "server", "kind": "orb", "radius": 140.0 }
                ],
                "post": { "bloom": 0.55 }
            } }],
            "continuousChannels": [
                { "id": "stage.client.opacity", "actorId": "stage", "property": "client.opacity",
                  "initial": 1.0, "events": [] },
                { "id": "stage.link.draw", "actorId": "stage", "property": "link.draw",
                  "initial": 0.0, "events": [
                      { "operation": "set", "atNanos": 0, "value": 0.0 },
                      { "operation": "ease", "atNanos": 500_000_000u64, "target": 1.0 }
                  ] }
            ],
            "stateChannels": [],
            "cues": [{ "id": "intro", "startNanos": 0, "endNanos": 1_000_000_000u64 }],
            "media": []
        })
    }

    #[test]
    fn identical_plans_summarize_to_nothing() {
        assert!(summarize(&plan(), &plan()).is_empty());
    }

    #[test]
    fn channels_report_initial_values_and_the_first_changed_event() {
        let mut after = plan();
        after["continuousChannels"][0]["initial"] = json!(0.0);
        after["continuousChannels"][1]["events"][1]["atNanos"] = json!(750_000_000u64);
        after["continuousChannels"][1]["events"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "operation": "set", "atNanos": 2_000_000_000u64, "value": 0.0 }));
        assert_eq!(
            summarize(&plan(), &after),
            [
                "channel stage.client.opacity initial 1.0 → 0.0",
                "channel stage.link.draw events changed from 0.750s (2 → 3 events)",
            ]
        );
    }

    #[test]
    fn items_are_matched_by_id_not_position() {
        let mut after = plan();
        let channels = after["continuousChannels"].as_array_mut().unwrap();
        channels.reverse();
        channels.remove(0);
        channels.push(json!({ "id": "stage.server.pulse", "actorId": "stage",
            "property": "server.pulse", "initial": 0.0, "events": [{}, {}] }));
        after["cues"][0]["endNanos"] = json!(1_500_000_000u64);
        after["durationNanos"] = json!(4_500_000_000u64);
        assert_eq!(
            summarize(&plan(), &after),
            [
                "duration 4.000s → 4.500s",
                "channel stage.server.pulse added (initial 0.0, 2 events)",
                "channel stage.link.draw removed",
                "cue intro 0.000s..1.000s → 0.000s..1.500s",
            ]
        );
    }

    #[test]
    fn actor_data_reports_paths_through_identified_elements() {
        let mut after = plan();
        after["actors"][0]["data"]["elements"][0]["at"][0] = json!(600.0);
        after["actors"][0]["data"]["post"]["bloom"] = json!(0.6);
        after["actors"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "id": "caption", "recipe": "caption", "data": {} }));
        assert_eq!(
            summarize(&plan(), &after),
            [
                "actor stage changed data.elements[client].at, data.post.bloom",
                "actor caption added",
            ]
        );
    }

    #[test]
    fn reels_prefix_changes_with_their_segment() {
        let reel = |plans: Vec<Value>| {
            json!({ "version": 1, "id": "film", "segments": plans.into_iter().map(|plan| json!({
                "transitionNanos": 0, "transitionStyle": "dip", "plan": plan
            })).collect::<Vec<_>>() })
        };
        let mut outro = plan();
        outro["id"] = json!("outro");
        let before = reel(vec![plan(), outro.clone()]);
        let mut changed = plan();
        changed["continuousChannels"][0]["initial"] = json!(0.5);
        let mut after = reel(vec![changed]);
        after["segments"][0]["transitionStyle"] = json!("cut");
        assert_eq!(
            summarize(&before, &after),
            [
                "hello: transitionStyle \"dip\" → \"cut\"",
                "hello: channel stage.client.opacity initial 1.0 → 0.5",
                "segment outro removed",
            ]
        );
        let swapped = reel(vec![outro, plan()]);
        assert_eq!(summarize(&before, &swapped), ["segments reordered"]);
    }
}

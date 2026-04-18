//! Command picker: search box + filtered list.
//!
//! Groups by `category` when the search box is empty; shows a flat
//! score-ordered list when the user types. Badges highlight OOB,
//! deprecated, unstable, and removed commands so the user doesn't
//! waste time filling in a form for a command QEMU already rejected.

use std::collections::BTreeMap;

use dioxus::prelude::*;

use bmc::types::CommandDto;

use super::search::score;

#[component]
pub fn CommandPicker(
    commands: Signal<Vec<CommandDto>>,
    selected: Signal<Option<usize>>,
    query: Signal<String>,
) -> Element {
    let query_value = query.read().clone();
    let commands_value = commands.read().clone();

    let filtered: Vec<(usize, i32)> = if query_value.is_empty() {
        (0..commands_value.len()).map(|i| (i, 0)).collect()
    } else {
        let mut scored: Vec<(usize, i32)> = commands_value
            .iter()
            .enumerate()
            .filter_map(|(i, c)| score(&query_value, &c.wire_name).map(|s| (i, s)))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        scored
    };

    // Grouped by category when there's no query.
    let grouped: BTreeMap<String, Vec<usize>> = if query_value.is_empty() {
        let mut m: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, c) in commands_value.iter().enumerate() {
            m.entry(c.category.clone()).or_default().push(i);
        }
        m
    } else {
        BTreeMap::new()
    };

    rsx! {
        div { class: "command-picker",
            input {
                class: "picker-search",
                r#type: "search",
                placeholder: "Search {commands_value.len()} commands…",
                value: "{query_value}",
                oninput: move |e| query.set(e.value()),
            }

            if query_value.is_empty() {
                div { class: "picker-list grouped",
                    for (cat, ids) in grouped.iter() {
                        details { class: "picker-group",
                            summary { "{cat}" span { class: "group-count", " ({ids.len()})" } }
                            ul { class: "picker-items",
                                for i in ids.iter().copied() {
                                    CommandItem {
                                        cmd: commands_value[i].clone(),
                                        index: i,
                                        selected: selected,
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "picker-list flat",
                    ul { class: "picker-items",
                        for (i, _score) in filtered.iter().copied() {
                            CommandItem {
                                cmd: commands_value[i].clone(),
                                index: i,
                                selected: selected,
                            }
                        }
                    }
                }
                if filtered.is_empty() {
                    div { class: "picker-empty", "No commands match." }
                }
            }
        }
    }
}

#[component]
fn CommandItem(cmd: CommandDto, index: usize, selected: Signal<Option<usize>>) -> Element {
    let is_selected = selected.read().as_ref().copied() == Some(index);
    let li_class = if is_selected {
        "picker-item selected"
    } else {
        "picker-item"
    };
    rsx! {
        li {
            class: li_class,
            onclick: move |_| selected.set(Some(index)),
            span { class: "cmd-name", "{cmd.wire_name}" }
            CommandBadges { cmd: cmd.clone() }
        }
    }
}

#[component]
fn CommandBadges(cmd: CommandDto) -> Element {
    rsx! {
        span { class: "cmd-badges",
            if cmd.allow_oob {
                span { class: "badge badge-oob", "OOB" }
            }
            if cmd.deprecated {
                span { class: "badge badge-deprecated", "deprecated" }
            }
            if cmd.unstable {
                span { class: "badge badge-unstable", "unstable" }
            }
            if cmd.until.is_some() {
                span { class: "badge badge-removed", "removed" }
            }
        }
    }
}

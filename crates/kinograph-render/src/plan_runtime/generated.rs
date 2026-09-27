//! Generated-channel ownership is checked in both ID and actor/property space.
//! Recipes keep their namespaces and values; Task position is the sole explicit
//! authored override, never a generic last-writer-wins rule.
use anyhow::{Result, bail};
use kinograph::plan::{ContinuousChannelPlan, ScenePlan};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
pub(super) enum Owner {
    Editor,
    Task,
    Grid,
    Component,
    Header,
    Attachment,
}

impl Owner {
    fn conflict(self, id: &str) -> anyhow::Error {
        anyhow::anyhow!(match self {
            Self::Editor => format!("authored channel collides with generated line channel '{id}'"),
            Self::Task => format!("authored channel collides with generated task channel '{id}'"),
            Self::Grid => format!("authored channel collides with generated grid channel '{id}'"),
            Self::Component => format!("generated component channel collision: {id}"),
            Self::Header => format!("generated header channel collision: {id}"),
            Self::Attachment => format!(
                "authored channel collides with reserved semantic attachment channel '{id}'"
            ),
        })
    }
}

pub(super) struct Reservations {
    ids: HashMap<String, (String, String)>,
    properties: HashSet<(String, String)>,
}

impl Reservations {
    pub(super) fn new(channels: &[ContinuousChannelPlan]) -> Self {
        Self {
            ids: channels
                .iter()
                .map(|c| (c.id.clone(), (c.actor_id.clone(), c.property.clone())))
                .collect(),
            properties: channels
                .iter()
                .map(|c| (c.actor_id.clone(), c.property.clone()))
                .collect(),
        }
    }

    /// False means keep an explicitly authored Task x/y channel.
    pub(super) fn reserve(
        &mut self,
        id: &str,
        actor: &str,
        property: &str,
        owner: Owner,
    ) -> Result<bool> {
        let key = (actor.to_owned(), property.to_owned());
        if self.ids.get(id).is_some_and(|existing| existing != &key) {
            return Err(owner.conflict(id));
        }
        if self.properties.contains(&key) {
            if matches!(owner, Owner::Task) && matches!(property, "x" | "y") {
                return Ok(false);
            }
            return Err(owner.conflict(id));
        }
        if self.ids.contains_key(id) {
            bail!(owner.conflict(id));
        }
        self.ids.insert(id.into(), key.clone());
        self.properties.insert(key);
        Ok(true)
    }
}

pub(super) fn extend(
    plan: &mut ScenePlan,
    channels: impl IntoIterator<Item = ContinuousChannelPlan>,
    owner: Owner,
) -> Result<()> {
    let mut reserved = Reservations::new(&plan.continuous_channels);
    for channel in channels {
        if reserved.reserve(&channel.id, &channel.actor_id, &channel.property, owner)? {
            plan.continuous_channels.push(channel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(id: &str, actor: &str, property: &str) -> ContinuousChannelPlan {
        ContinuousChannelPlan {
            id: id.into(),
            actor_id: actor.into(),
            property: property.into(),
            initial: 0_f32.into(),
            events: vec![],
        }
    }
    #[test]
    fn collisions_check_both_keys_and_only_allow_task_position() {
        let existing = [
            channel("authored", "a", "x"),
            channel("a.y", "other", "opacity"),
        ];
        let mut slots = Reservations::new(&existing);
        assert!(!slots.reserve("a.x", "a", "x", Owner::Task).unwrap());
        assert!(slots.reserve("a.x", "a", "x", Owner::Grid).is_err());
        assert!(slots.reserve("a.y", "a", "y", Owner::Task).is_err());
        assert!(slots.reserve("a.width", "a", "width", Owner::Task).unwrap());
        assert!(
            slots
                .reserve("alternate", "a", "width", Owner::Task)
                .is_err()
        );
    }
}
